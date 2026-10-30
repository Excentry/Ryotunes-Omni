import { describe, expect, it, vi } from 'vitest';
import { auth, initApp, playback, toast, ui } from './player.svelte';
import type { QueueState } from './api';

const host = vi.hoisted(() => ({
	listeners: new Map<string, (payload: never) => void>(),
	unlisten: vi.fn(),
	releaseLateListener: undefined as (() => void) | undefined,
	getQueue: vi.fn(() => new Promise(() => {})),
	getPlayback: vi.fn(() => new Promise(() => {}))
}));

vi.mock('./api', async (importOriginal) => {
	const original = await importOriginal<typeof import('./api')>();
	const listeners = Object.fromEntries(
		Object.keys(original)
			.filter((name) => /^on[A-Z]/.test(name))
			.map((name) => [name, (handler: (payload: never) => void) => {
				if (name === 'onVolume' || name === 'onStopAfterCurrent') {
					return Promise.reject(new Error('Listener unavailable'));
				}
				host.listeners.set(name, handler);
				const unlisten = () => {
					host.listeners.delete(name);
					host.unlisten(name);
				};
				if (name === 'onLtNotice') {
					return new Promise<() => void>((resolve) => {
						host.releaseLateListener = () => resolve(unlisten);
					});
				}
				return Promise.resolve(unlisten);
			}])
	);
	return { ...original, ...listeners, getQueue: host.getQueue, getPlayback: host.getPlayback };
});

describe('player state with a fixture host', () => {
	it('applies host events and unsubscribes when the mini window closes', async () => {
		const toastError = vi.spyOn(toast, 'error');
		const teardown = initApp(true);
		await vi.waitFor(() => {
			expect(ui.toast?.msg).toBe('Live updates are unavailable. Restart the app to try again.');
		});
		expect(toastError).toHaveBeenCalledTimes(1);
		const queue: QueueState = {
			items: [{ video_id: 'track-1', title: 'Track', artists: 'Artist' }],
			currentIndex: 0
		};
		host.listeners.get('onQueueChanged')?.(queue as never);
		expect(playback.queue).toEqual(queue);
		host.listeners.get('onDuration')?.(120 as never);
		host.listeners.get('onPosition')?.(15 as never);
		expect(playback.position).toBe(15);
		host.listeners.get('onPlaybackState')?.('paused' as never);
		expect(playback.paused).toBe(true);
		playback.pendingVideoId = 'track-1';
		host.listeners.get('onPlaybackError')?.('Playback unavailable' as never);
		expect(playback.lastError).toBe('Playback unavailable');
		expect(playback.pendingVideoId).toBeNull();
		ui.channelPickerOpen = true;
		ui.channelPickerRequired = true;
		const epoch = auth.epoch;
		host.listeners.get('onAuthChanged')?.({ signedIn: false } as never);
		expect(auth.account?.signedIn).toBe(false);
		expect(auth.epoch).toBe(epoch + 1);
		expect(ui.channelPickerOpen).toBe(false);
		expect(ui.channelPickerRequired).toBe(false);

		const listenerCount = host.listeners.size;
		expect(listenerCount).toBeGreaterThan(0);
		teardown();
		await vi.waitFor(() => expect(host.listeners.size).toBe(1));
		expect(host.listeners.has('onLtNotice')).toBe(true);
		host.releaseLateListener!();
		await vi.waitFor(() => expect(host.listeners.size).toBe(0));
		expect(host.unlisten).toHaveBeenCalledTimes(listenerCount);
		host.listeners.get('onPosition')?.(45 as never);
		expect(playback.position).toBe(15);
		toastError.mockRestore();
	});
});
