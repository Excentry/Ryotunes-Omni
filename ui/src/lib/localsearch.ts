// Shared local search/ranking for every in-app list. Pure and allocation-conscious so it can be
// exercised by localsearch.check.ts without a DOM and reused by playlist/library/queue surfaces.
//
// Search rules:
// - Unicode/diacritic insensitive ("Beyoncé" matches "beyonce").
// - whitespace/punctuation normalized and token order independent.
// - exact/prefix title matches outrank artist/album/subtitle matches.
// - a single longer token gets mild typo tolerance, never expensive fuzzy work for 1–3 chars.
// - stable ranking: equal scores keep the collection's own order.
import type { BrowseItem, SongItem } from './api';

export interface Indexed<T> {
	items: T[];
	/** Normalized title. */
	name: string[];
	/** Normalized non-title searchable fields. */
	secondary: string[];
	/** Title + secondary text. */
	hay: string[];
	/** Pre-split normalized words, used only by the guarded typo fallback. */
	words: string[][];
}

/** Lowercase, strip combining marks, and make punctuation/extra whitespace equivalent. */
export function normalizeSearchText(value: string | null | undefined): string {
	return (value ?? '')
		.normalize('NFKD')
		.replace(/\p{M}+/gu, '')
		.toLocaleLowerCase()
		.replace(/[^\p{L}\p{N}]+/gu, ' ')
		.trim()
		.replace(/\s+/g, ' ');
}

function makeIndex<T>(items: T[], titleOf: (item: T) => string, secondaryOf: (item: T) => string): Indexed<T> {
	const copy = items.slice();
	const name: string[] = [];
	const secondary: string[] = [];
	const hay: string[] = [];
	const words: string[][] = [];
	for (const item of copy) {
		const title = normalizeSearchText(titleOf(item));
		const extra = normalizeSearchText(secondaryOf(item));
		const all = `${title} ${extra}`.trim();
		name.push(title);
		secondary.push(extra);
		hay.push(all);
		words.push(all ? all.split(' ') : []);
	}
	return { items: copy, name, secondary, hay, words };
}

export function indexSongs(songs: SongItem[]): Indexed<SongItem> {
	return makeIndex(
		songs,
		(s) => s.title ?? '',
		(s) => `${s.artists ?? ''} ${s.album ?? ''} ${s.play_count ?? ''}`
	);
}

export function indexCards(cards: BrowseItem[]): Indexed<BrowseItem> {
	return makeIndex(cards, (c) => c.title ?? '', (c) => c.subtitle ?? '');
}

/** Generic indexed collection for queue/setting surfaces that need to preserve wrapper metadata. */
export function indexCustom<T>(items: T[], titleOf: (item: T) => string, secondaryOf: (item: T) => string): Indexed<T> {
	return makeIndex(items, titleOf, secondaryOf);
}

/** Bounded Levenshtein. Returns >limit as soon as the word cannot qualify. */
function editDistanceWithin(a: string, b: string, limit: number): number {
	if (Math.abs(a.length - b.length) > limit) return limit + 1;
	if (a === b) return 0;
	let prev = Array.from({ length: b.length + 1 }, (_, i) => i);
	for (let i = 1; i <= a.length; i++) {
		const cur = new Array<number>(b.length + 1);
		cur[0] = i;
		let rowMin = cur[0];
		for (let j = 1; j <= b.length; j++) {
			cur[j] = Math.min(
				prev[j] + 1,
				cur[j - 1] + 1,
				prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1)
			);
			rowMin = Math.min(rowMin, cur[j]);
		}
		if (rowMin > limit) return limit + 1;
		prev = cur;
	}
	return prev[b.length];
}

function fuzzyWordScore(words: string[], token: string): number | null {
	if (token.length < 4) return null;
	const limit = token.length >= 8 ? 2 : 1;
	let best = limit + 1;
	for (const word of words) {
		if (!word || Math.abs(word.length - token.length) > limit) continue;
		// Cheap guard before dynamic programming: typo matches usually retain first or last letter.
		if (word[0] !== token[0] && word.at(-1) !== token.at(-1)) continue;
		const d = editDistanceWithin(word, token, limit);
		if (d < best) best = d;
		if (best === 1) break;
	}
	return best <= limit ? best : null;
}

function scoreAt<T>(ix: Indexed<T>, i: number, q: string, terms: string[]): number | null {
	const name = ix.name[i];
	const extra = ix.secondary[i];
	const hay = ix.hay[i];

	if (name === q) return 0;
	if (name.startsWith(q)) return 10 + Math.min(0.99, Math.max(0, name.length - q.length) / 100); // shorter prefixes first
	if (extra === q) return 20;
	if (name.includes(q)) return 30;
	if (extra.includes(q)) return 40;

	// Every token must be represented somewhere. Token order is intentionally irrelevant.
	if (terms.every((term) => hay.includes(term))) {
		let score = 50;
		for (const term of terms) {
			if (name.split(' ').some((w) => w === term)) score -= 3;
			else if (name.split(' ').some((w) => w.startsWith(term))) score -= 1;
		}
		return score;
	}

	// Typo tolerance is deliberately only for a single meaningful token. It keeps "weknd" useful
	// without making a two-word query scan every word through edit distance on a 10k-song playlist.
	if (terms.length === 1) {
		const distance = fuzzyWordScore(ix.words[i], terms[0]);
		if (distance !== null) return 80 + distance;
	}
	return null;
}

/** Everything matching `query`, best first. Empty query matches nothing. */
export function match<T>(ix: Indexed<T>, query: string): T[] {
	const q = normalizeSearchText(query);
	if (!q) return [];
	const terms = q.split(' ');
	const hits: { i: number; rank: number }[] = [];
	for (let i = 0; i < ix.hay.length; i++) {
		const rank = scoreAt(ix, i, q, terms);
		if (rank !== null) hits.push({ i, rank });
	}
	hits.sort((a, b) => a.rank - b.rank || a.i - b.i);
	return hits.map((h) => ix.items[h.i]);
}

// Track lists often re-filter the exact same array on every keystroke. Keep their flattened text in
// a WeakMap: when the page drops the array, the index disappears with it and cannot become a leak.
const songIndexCache = new WeakMap<SongItem[], Indexed<SongItem>>();
let lastSongIndex: Indexed<SongItem> | null = null;

/** True when `items` is `prev.items` followed by more rows, so only the tail needs indexing. */
function appendsTo(prev: Indexed<SongItem>, items: SongItem[]): boolean {
	if (items.length < prev.items.length) return false;
	for (let i = 0; i < prev.items.length; i++) if (items[i] !== prev.items[i]) return false;
	return true;
}

/** The index of `prev` plus `tail`, normalizing only the new rows. */
export function extendSongIndex(prev: Indexed<SongItem>, tail: SongItem[]): Indexed<SongItem> {
	const next = indexSongs(tail);
	return {
		items: prev.items.concat(next.items),
		name: prev.name.concat(next.name),
		secondary: prev.secondary.concat(next.secondary),
		hay: prev.hay.concat(next.hay),
		words: prev.words.concat(next.words)
	};
}

/** Shared ranked track filter. Empty query preserves the original array reference. */
export function filterSongItems<T extends SongItem>(items: T[], query: string): T[] {
	if (!normalizeSearchText(query)) return items;
	let ix = songIndexCache.get(items as SongItem[]);
	if (!ix) {
		// Pagination hands over a new array per page that starts with the previous one. Reusing the
		// last index keeps a filter walk linear instead of re-normalizing every row on every page.
		ix =
			lastSongIndex && appendsTo(lastSongIndex, items)
				? extendSongIndex(lastSongIndex, items.slice(lastSongIndex.items.length))
				: indexSongs(items);
		songIndexCache.set(items as SongItem[], ix);
		lastSongIndex = ix;
	}
	return match(ix, query) as T[];
}
