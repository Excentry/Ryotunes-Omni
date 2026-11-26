import { convertFileSrc } from '@tauri-apps/api/core';

// Rewrite a Google image URL to (about) the pixel size a slot actually renders, so WebKitGTK
// doesn't decode a 544px (or 1080p) image for a 40px row. Only lh3/yt3 googleusercontent-style
// URLs carry the size in the URL (`=w544-h544` / `=s576` suffixes); anything else (notably
// i.ytimg.com path-variant thumbs, where other sizes can 404) is returned unchanged.
export function thumb(url: string | undefined | null, px: number): string | undefined {
	if (!url) return undefined;
	// Local library artwork is a path on this machine, not a URL. The webview can't open a bare
	// path, so hand it through Tauri's asset protocol. Kept here rather than at the command
	// boundary so what gets stored (queue, Shortcuts) stays the real path — which is also what
	// MPRIS needs.
	if (url.startsWith('/') || /^[A-Za-z]:[\\/]/.test(url)) return convertFileSrc(url);
	if (/=w\d+-h\d+/.test(url)) return url.replace(/=w\d+-h\d+/, `=w${px}-h${px}`);
	if (/=s\d+/.test(url)) return url.replace(/=s\d+/, `=s${px}`);
	if (url.startsWith(SPOTIFY_CDN)) return spotifySized(url, px);
	return url;
}

// Spotify serves each cover at fixed widths, and the width is the 16-character prefix of the image
// id; the rest of the id is shared. The catalogue hands over the smallest one, so pick the smallest
// width that still fills the slot. Ids with any other prefix (playlist mosaics, uploads) stay as
// they are.
const SPOTIFY_CDN = 'https://i.scdn.co/image/';
const SPOTIFY_WIDTHS: [number, string][][] = [
	[[64, 'ab67616d00004851'], [300, 'ab67616d00001e02'], [640, 'ab67616d0000b273']],
	[[160, 'ab6761610000f178'], [320, 'ab67616100005174'], [640, 'ab6761610000e5eb']]
];

function spotifyFamily(url: string): [number, string][] | undefined {
	if (!url.startsWith(SPOTIFY_CDN)) return undefined;
	const id = url.slice(SPOTIFY_CDN.length);
	return SPOTIFY_WIDTHS.find((widths) => widths.some(([, prefix]) => id.startsWith(prefix)));
}

/** Whether every size thumb() derives from this URL is known to exist, so the sized URL can be
 * tried before the original. */
export function sizesReliably(url: string | undefined | null): boolean {
	return !!url && !!spotifyFamily(url);
}

function spotifySized(url: string, px: number): string {
	const id = url.slice(SPOTIFY_CDN.length);
	const family = spotifyFamily(url);
	if (!family) return url;
	const [, prefix] = family.find(([width]) => width >= px) ?? family[family.length - 1];
	return SPOTIFY_CDN + prefix + id.slice(prefix.length);
}
