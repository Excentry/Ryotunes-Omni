import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Page from './+page.svelte';
import { clearCached } from '$lib/pagecache';
import type { HomePage } from '$lib/api';

const host = vi.hoisted(() => ({ getHome: vi.fn(), getHomeMore: vi.fn(), toastError: vi.fn() }));

vi.mock('$lib/api', () => host);
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$lib/player.svelte', () => ({
	auth: { account: { signedIn: true } },
	library: { items: [] },
	personal: { picks: [], saved: [], pins: [], recent: {}, artists: {}, dismissedSeeds: [], home: { order: [], hidden: [], seen: [] } },
	playback: { now: null },
	noteHomeSections: vi.fn(),
	seedOnRepeatPick: vi.fn(),
	toast: { error: host.toastError },
	startGoogleLogin: vi.fn()
}));
vi.mock('$lib/theme.svelte', () => ({ appearance: { lowResourceMode: true } }));
vi.mock('$lib/components/HomeHero.svelte', async () => ({ default: (await import('$lib/test/Empty.svelte')).default }));
vi.mock('$lib/components/Shortcuts.svelte', async () => ({ default: (await import('$lib/test/Empty.svelte')).default }));
vi.mock('$lib/components/RecentRail.svelte', async () => ({ default: (await import('$lib/test/Empty.svelte')).default }));
vi.mock('$lib/components/Shelf.svelte', async () => ({ default: (await import('$lib/test/Empty.svelte')).default }));
vi.mock('$lib/components/ForgottenFavourites.svelte', async () => ({ default: (await import('$lib/test/Empty.svelte')).default }));
vi.mock('$lib/components/ArtistIndex.svelte', async () => ({ default: (await import('$lib/test/Empty.svelte')).default }));
vi.mock('$lib/components/HomeLayoutDialog.svelte', async () => ({ default: (await import('$lib/test/Empty.svelte')).default }));

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: Error) => void;
	const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
	return { promise, resolve, reject };
}

describe('Home with a fixture host', () => {
	let component: ReturnType<typeof mount>;
	let target: HTMLElement;

	beforeEach(() => {
		vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
		clearCached();
		host.getHome.mockReset();
		host.getHomeMore.mockReset();
		host.toastError.mockReset();
		target = document.createElement('main');
		document.body.append(target);
	});

	afterEach(async () => {
		if (component) await unmount(component);
		target.remove();
		vi.unstubAllGlobals();
	});

	it('shows loading placeholders, then an empty signed-in feed', async () => {
		const request = deferred<HomePage>();
		host.getHome.mockReturnValue(request.promise);
		component = mount(Page, { target });
		flushSync();
		expect(target.querySelectorAll('section[aria-hidden="true"]')).toHaveLength(3);
		expect(target.textContent).not.toContain('Your home feed came back empty this time.');
		request.resolve({ chips: [], sections: [] });
		await vi.waitFor(() => {
			flushSync();
			expect(target.textContent).toContain('Your home feed came back empty this time.');
		});
		expect(target.querySelectorAll('section[aria-hidden="true"]')).toHaveLength(0);
	});

	it('shows a host error and retries into a successful empty feed', async () => {
		host.getHome.mockRejectedValueOnce(new Error('Home unavailable'))
			.mockResolvedValueOnce({ chips: [], sections: [] });
		component = mount(Page, { target });
		await vi.waitFor(() => {
			flushSync();
			expect(target.textContent).toContain('Could not load Home. Check your connection and try again.');
		});
		expect(target.textContent).not.toContain('Home unavailable');
		const retry = Array.from(target.querySelectorAll('button')).find((button) => button.textContent?.includes('Try again'));
		expect(retry).toBeDefined();
		retry!.click();
		await vi.waitFor(() => {
			flushSync();
			expect(target.textContent).toContain('Your home feed came back empty this time.');
		});
		expect(target.textContent).not.toContain('Home unavailable');
		expect(host.getHome).toHaveBeenCalledTimes(2);
	});

	it('offers a retry when loading another page fails', async () => {
		host.getHome.mockResolvedValue({
			chips: [], sections: [{ title: 'First shelf', items: [] }], continuation: 'next-token'
		});
		host.getHomeMore.mockRejectedValueOnce(new Error('Page unavailable'))
			.mockResolvedValueOnce({ chips: [], sections: [{ title: 'Second shelf', items: [] }] });
		component = mount(Page, { target });
		await vi.waitFor(() => {
			flushSync();
			expect(target.textContent).toContain('Load more');
		});
		Array.from(target.querySelectorAll('button')).find((button) => button.textContent?.includes('Load more'))!.click();
		await vi.waitFor(() => {
			flushSync();
			expect(target.textContent).toContain('Try again');
		});
		expect(host.toastError).toHaveBeenCalledWith('Could not load more');
		Array.from(target.querySelectorAll('button')).find((button) => button.textContent?.includes('Try again'))!.click();
		await vi.waitFor(() => {
			flushSync();
			expect(target.textContent).not.toContain('Try again');
		});
		expect(host.getHomeMore).toHaveBeenCalledTimes(2);
		expect(host.getHomeMore).toHaveBeenCalledWith('next-token');
	});
});
