// Self-check for the artwork sizing in `thumb.ts`:
//
//     node --experimental-strip-types ui/src/lib/thumb.check.ts
//
// Prints "ok" and exits 0, or throws on the first broken invariant. Not imported by the app.
import { sizesReliably, thumb } from './thumb.ts';

function ok(cond: boolean, what: string): void {
	if (!cond) throw new Error(`FAIL: ${what}`);
}

const cdn = 'https://i.scdn.co/image/';
const hash = '51646e66471c2f3f29f87a03';
const album = (prefix: string) => `${cdn}${prefix}${hash}`;

ok(thumb(album('ab67616d00004851'), 48) === album('ab67616d00004851'), 'a small slot keeps the 64px cover');
ok(thumb(album('ab67616d00004851'), 120) === album('ab67616d00001e02'), 'the player bar gets the 300px cover');
ok(thumb(album('ab67616d00004851'), 300) === album('ab67616d00001e02'), 'an exact width is enough');
ok(thumb(album('ab67616d00004851'), 960) === album('ab67616d0000b273'), 'a large view gets the largest cover');
ok(thumb(album('ab67616d0000b273'), 96) === album('ab67616d00001e02'), 'a large cover is sized down for a row');
ok(thumb(`${cdn}ab6761610000f178${hash}`, 512) === `${cdn}ab6761610000e5eb${hash}`, 'artist portraits size up');
ok(thumb(`${cdn}ab6761610000e5eb${hash}`, 128) === `${cdn}ab6761610000f178${hash}`, 'artist portraits size down');
ok(thumb(`${cdn}ab67706c0000da84${hash}`, 960) === `${cdn}ab67706c0000da84${hash}`, 'an unknown prefix is untouched');
ok(thumb('https://mosaic.scdn.co/640/abc', 96) === 'https://mosaic.scdn.co/640/abc', 'mosaics are untouched');
ok(thumb('https://lh3.googleusercontent.com/x=w60-h60-l90-rj', 544) === 'https://lh3.googleusercontent.com/x=w544-h544-l90-rj', 'Google images still resize');
ok(thumb('https://i.ytimg.com/vi/abc/hqdefault.jpg', 544) === 'https://i.ytimg.com/vi/abc/hqdefault.jpg', 'ytimg is untouched');

ok(sizesReliably(album('ab67616d00004851')), 'Spotify album covers size reliably');
ok(sizesReliably(`${cdn}ab6761610000e5eb${hash}`), 'Spotify portraits size reliably');
ok(!sizesReliably(`${cdn}ab67706c0000da84${hash}`), 'an unknown Spotify prefix does not');
ok(!sizesReliably('https://lh3.googleusercontent.com/x=w60-h60'), 'Google images keep the original first');
ok(!sizesReliably(undefined), 'no URL is not reliable');

console.log('ok');
