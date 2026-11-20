import * as api from './api';
import { safeErrorMessage } from './safe-error';
import {
	initialSpotifyView,
	reduceSpotifyAuth,
	type SpotifyView
} from './spotify-view';

export const spotify = $state<SpotifyView>({ ...initialSpotifyView });

function assign(next: SpotifyView) {
	Object.assign(spotify, next);
}

export async function refreshSpotify() {
	const [status, selected] = await Promise.all([api.spotifyStatus(), api.getProvider()]);
	assign({
		...spotify,
		provider: selected.provider,
		signedIn: status.signedIn,
		name: status.name
	});
}

export async function selectProvider(provider: api.Provider) {
	if (spotify.provider === provider) return;
	const result = await api.setProvider(provider);
	spotify.provider = result.provider;
}

export async function signInToSpotify() {
	spotify.error = '';
	spotify.flow = 'browser';
	try {
		await api.spotifySignIn();
	} catch (error) {
		assign({ ...spotify, flow: 'idle', error: safeErrorMessage(error) });
	}
}

export async function signOutOfSpotify() {
	await api.spotifySignOut();
}

/** Seeds the state and follows the host's events. Returns the cleanup. */
export function initSpotify(): () => void {
	let stopped = false;
	const unlisteners: (() => void)[] = [];
	const keep = (pending: Promise<() => void>) =>
		void pending.then((off) => (stopped ? off() : unlisteners.push(off))).catch(() => {});
	keep(api.onSpotifyAuth((event) => assign(reduceSpotifyAuth(spotify, event))));
	keep(api.onProviderChanged((provider) => (spotify.provider = provider)));
	void refreshSpotify().catch(() => {});
	return () => {
		stopped = true;
		for (const off of unlisteners.splice(0)) off();
	};
}
