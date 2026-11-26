import type { Provider, SpotifyAuthEvent } from './api.ts';

export interface SpotifyView {
	provider: Provider;
	signedIn: boolean;
	name: string | null;
	/** `browser` while the authorization page is open and the host waits for its redirect. */
	flow: 'idle' | 'browser';
	error: string;
	/** Kept while waiting, so the gate can offer the link when the browser did not open. */
	authUrl: string;
}

export const initialSpotifyView: SpotifyView = {
	provider: 'youtube',
	signedIn: false,
	name: null,
	flow: 'idle',
	error: '',
	authUrl: ''
};

export function reduceSpotifyAuth(view: SpotifyView, event: SpotifyAuthEvent): SpotifyView {
	switch (event.state) {
		case 'url':
			return { ...view, flow: 'browser', authUrl: event.url, error: '' };
		case 'open_failed':
			return { ...view, error: event.message };
		case 'signed_in':
			return { ...view, signedIn: true, name: event.name, flow: 'idle', error: '', authUrl: '' };
		case 'signed_out':
			return { ...view, signedIn: false, name: null, flow: 'idle', error: '', authUrl: '' };
		case 'error':
			return { ...view, flow: 'idle', error: event.message, authUrl: '' };
	}
}

/** Spotify selected without a session: pages show the sign-in gate instead of their content. */
export const needsSpotifyGate = (view: SpotifyView) =>
	view.provider === 'spotify' && !view.signedIn;

/** Changes whenever what the catalogue pages would load changes, so they can be re-keyed. */
export const spotifyBrowseKey = (view: SpotifyView) =>
	`${view.provider}:${view.provider === 'spotify' && view.signedIn ? 'in' : 'out'}`;
