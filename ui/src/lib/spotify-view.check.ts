import {
	initialSpotifyView,
	needsSpotifyGate,
	reduceSpotifyAuth,
	spotifyBrowseKey,
	type SpotifyView
} from './spotify-view.ts';

function ok(condition: boolean, message: string): void {
	if (!condition) throw new Error(message);
}

const spotify: SpotifyView = { ...initialSpotifyView, provider: 'spotify' };
ok(needsSpotifyGate(spotify), 'Spotify selected without a session shows the gate');
ok(!needsSpotifyGate(initialSpotifyView), 'YouTube Music never shows the gate');

const url = 'https://accounts.spotify.com/authorize?client_id=x';
const waiting = reduceSpotifyAuth(spotify, { state: 'url', url });
ok(waiting.flow === 'browser' && waiting.authUrl === url, 'the url event starts the browser wait');

const unopened = reduceSpotifyAuth(waiting, { state: 'open_failed', message: 'no browser' });
ok(unopened.flow === 'browser', 'a failed browser launch keeps waiting for the redirect');
ok(unopened.authUrl === url && unopened.error === 'no browser', 'and keeps the link to open by hand');

const signedIn = reduceSpotifyAuth(unopened, { state: 'signed_in', name: 'Ada' });
ok(signedIn.signedIn && signedIn.name === 'Ada', 'signed_in records the session');
ok(signedIn.flow === 'idle' && !signedIn.error && !signedIn.authUrl, 'signed_in clears the flow');
ok(!needsSpotifyGate(signedIn), 'a session lifts the gate');
ok(spotifyBrowseKey(signedIn) !== spotifyBrowseKey(spotify), 'signing in re-keys the pages');

const failed = reduceSpotifyAuth(waiting, { state: 'error', message: 'no Premium' });
ok(failed.flow === 'idle' && failed.error === 'no Premium' && !failed.authUrl, 'errors end the wait');

const out = reduceSpotifyAuth(signedIn, { state: 'signed_out' });
ok(!out.signedIn && out.name === null && needsSpotifyGate(out), 'signing out brings the gate back');

ok(
	spotifyBrowseKey({ ...initialSpotifyView, signedIn: true }) === spotifyBrowseKey(initialSpotifyView),
	'a Spotify session does not re-key pages while YouTube Music is selected'
);

console.log('ok');
