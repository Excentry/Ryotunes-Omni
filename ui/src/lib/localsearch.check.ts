import { extendSongIndex, indexCards, indexSongs, match, normalizeSearchText, filterSongItems } from './localsearch.ts';
import type { BrowseItem, SongItem } from './api.ts';

function ok(value: unknown, message: string) {
	if (!value) throw new Error(message);
}
function ids(items: SongItem[]) { return items.map((i) => i.video_id).join(''); }
const song = (video_id: string, title: string, artists = '', album = ''): SongItem => ({
	video_id, title, artists, album
} as SongItem);
const card = (id: string, title: string, subtitle = ''): BrowseItem => ({
	id, title, subtitle, kind: 'playlist'
} as BrowseItem);

const songs = [
	song('a', 'Yellow Submarine', 'The Beatles', 'Revolver'),
	song('b', 'Submarine Dreams', 'Someone Else', 'Abbey'),
	song('c', 'Something', 'The Beatles', 'Abbey Road'),
	song('d', 'Yellow', 'Coldplay', 'Parachutes'),
	song('e', 'Starboy', 'The Weeknd', 'Starboy'),
	song('f', 'STAY', 'The Kid LAROI & Justin Bieber', 'F*CK LOVE'),
	song('g', 'Beyoncé Live', 'Beyoncé', 'Homecoming')
];
const ix = indexSongs(songs);
ok(match(ix, '').length === 0, 'an empty query matches nothing');
ok(match(ix, '   ').length === 0, 'whitespace is an empty query');
ok(ids(match(ix, 'SUBMARINE')) === 'ba', 'matching ignores case and ranks title prefix');
ok(ids(match(ix, 'abbey')) === 'bc', 'album metadata is searched');
ok(ids(match(ix, 'coldplay')) === 'd', 'artist is searched');
ok(ids(match(ix, 'beatles yellow')) === 'a', 'tokens can match fields in any order');
ok(match(ix, 'beatles coldplay').length === 0, 'all query tokens are required');
ok(ids(match(ix, 'yellow')) === 'da', 'exact title outranks title prefix');
ok(ids(match(ix, 'st')) === 'fe', 'short queries are predictable prefix matches without fuzzy noise');
ok(ids(match(ix, 'weknd')) === 'e', 'single longer token tolerates a small typo');
ok(ids(match(ix, 'beyonce')) === 'g', 'diacritics are normalized');
ok(normalizeSearchText('  Juice—WRLD  ') === 'juice wrld', 'punctuation and whitespace normalize');
ok(filterSongItems(songs, 'weeknd')[0]?.video_id === 'e', 'shared track filter uses ranked engine');

const firstPage = songs.slice(0, 4);
const secondPage = songs.slice(0, 7);
filterSongItems(firstPage, 'yellow');
ok(ids(filterSongItems(secondPage, 'beyonce')) === 'g', 'a longer page is searchable after extending the previous index');
ok(ids(filterSongItems(secondPage, 'yellow')) === 'da', 'extended index keeps the original ranking');
const reordered = [songs[3], songs[0], ...songs.slice(4)];
ok(ids(filterSongItems(reordered, 'yellow')) === 'da', 'a list that is not an append is reindexed from scratch');
ok(extendSongIndex(indexSongs(firstPage), songs.slice(4)).hay.length === 7, 'extending adds only the new rows');

const cards = indexCards([card('1', 'Revolver', 'The Beatles'), card('2', 'Parachutes', 'Coldplay')]);
ok(match(cards, 'beatles').map((c) => c.id).join('') === '1', 'card subtitle is searchable');
ok(match(indexSongs([]), 'anything').length === 0, 'empty library searches fine');
console.log('Local search: OK');
