import { emit } from '@tauri-apps/api/event';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import * as api from './api.ts';

function equal(actual: unknown, expected: unknown, message: string): void {
	if (JSON.stringify(actual) !== JSON.stringify(expected)) {
		throw new Error(`${message}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
	}
}

Object.defineProperty(globalThis, 'window', {
	value: { crypto: globalThis.crypto },
	configurable: true
});

const calls: Array<{ command: string; args: unknown }> = [];
const home: api.HomePage = {
	chips: [{ title: 'Focus', params: 'focus-token' }],
	sections: [{ title: 'For you', items: [{ kind: 'song', id: 'song-1', title: 'First' }] }],
	continuation: 'next-page'
};
const nextPage: api.HomePage = {
	chips: [],
	sections: [{ title: 'More', items: [{ kind: 'album', id: 'album-1', title: 'Second' }] }]
};
let resolveHome!: (page: api.HomePage) => void;
const pendingHome = new Promise<api.HomePage>((resolve) => { resolveHome = resolve; });

mockIPC((command, args) => {
	calls.push({ command, args });
	switch (command) {
		case 'get_home': return pendingHome;
		case 'get_home_more': return nextPage;
		case 'search_page': throw new Error('Search unavailable');
		case 'get_playback': return { now: null, paused: true, position: 0, duration: 0, volume: 70 };
		default: throw new Error(`Unexpected command: ${command}`);
	}
}, { shouldMockEvents: true });

try {
	let settled = false;
	const request = api.getHome('focus-token').then((page) => { settled = true; return page; });
	await Promise.resolve();
	equal(settled, false, 'home request remains pending until host responds');
	equal(calls[0], { command: 'get_home', args: { params: 'focus-token' } }, 'home command');
	resolveHome(home);
	equal(await request, home, 'home response');
	equal(await api.getHomeMore('next-page'), nextPage, 'home continuation');
	equal(calls[1], { command: 'get_home_more', args: { token: 'next-page' } }, 'continuation command');
	let searchError: unknown;
	try { await api.searchPage('query'); } catch (error) { searchError = error; }
	equal(String(searchError), 'Error: Search unavailable', 'search rejection reaches UI');
	equal(calls[2], { command: 'search_page', args: { query: 'query' } }, 'search command');
	equal(await api.getPlayback(), { now: null, paused: true, position: 0, duration: 0, volume: 70 }, 'playback snapshot');

	const received: unknown[] = [];
	const unlistenQueue = await api.onQueueChanged((queue) => received.push(queue));
	const unlistenPosition = await api.onPosition((position) => received.push(position));
	const unlistenError = await api.onPlaybackError((message) => received.push(message));
	const unlistenAuth = await api.onAuthChanged((account) => received.push(account));
	const queue: api.QueueState = {
		items: [{ video_id: 'song-1', title: 'First', artists: 'Artist' }],
		currentIndex: 0
	};
	await emit('queue-changed', queue);
	await emit('position', { position: 12.5 });
	await emit('playback-error', { message: 'Playback unavailable' });
	await emit('auth-changed', { signedIn: false });
	equal(received, [queue, 12.5, 'Playback unavailable', { signedIn: false }], 'event payloads');
	unlistenQueue();
	unlistenPosition();
	unlistenError();
	unlistenAuth();
	console.log('UI-host fixture checks: ok');
} finally {
	clearMocks();
}
