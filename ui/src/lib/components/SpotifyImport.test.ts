import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import SpotifyImport from './SpotifyImport.svelte';
import type { SpotifyLabReview } from '$lib/api';

const host = vi.hoisted(() => ({
	spotifyImportStatus: vi.fn(),
	spotifyImportConnect: vi.fn(),
	spotifyImportDisconnect: vi.fn(),
	spotifyImportStart: vi.fn(),
	spotifyImportSearch: vi.fn(),
	spotifyImportCommit: vi.fn(),
	spotifyImportSnapshot: vi.fn(),
	spotifyImportReadback: vi.fn()
}));
vi.mock('$lib/api', () => host);

const fixture = (): SpotifyLabReview => ({
	review: {
		jobId: 'job', revision: 2, phase: 'review',
		source: { accountKey: 'account', playlistId: 'source', snapshotId: 'snapshot', total: 2 },
		target: { playlistId: null, revision: null },
		occurrences: [0, 1].map((ordinal) => ({
			occurrenceId: `entry-${ordinal}`, ordinal, outcome: 'low_confidence',
			candidates: [{
				candidateId: `candidate-${ordinal}`, videoId: 'aaaaaaaaaaa',
				title: 'Repeated song', artists: ['Artist'], durationMs: 180000
			}]
		}))
	},
	sources: [0, 1].map(() => ({
		title: 'Source song', artists: ['Artist'], durationMs: 180000, outcome: 'low_confidence'
	})),
	receipt: null
});

function button(text: string): HTMLButtonElement {
	const result = [...document.querySelectorAll('button')].find((node) => node.textContent?.includes(text));
	if (!result) throw new Error(`Missing button: ${text}`);
	return result;
}

async function settled(text: string) {
	await vi.waitFor(() => {
		flushSync();
		expect(document.body.textContent).toContain(text);
	});
}

describe('Spotify import review with a fixture host', () => {
	let component: ReturnType<typeof mount> | undefined;
	let target: HTMLElement;
	let saved: ReturnType<typeof vi.fn<() => void>>;

	beforeEach(() => {
		vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} });
		for (const method of Object.values(host)) method.mockReset();
		host.spotifyImportStatus.mockResolvedValue({
			enabled: true, connected: false, maxOccurrences: 1000, reviewJobs: []
		});
		host.spotifyImportConnect.mockResolvedValue(undefined);
		host.spotifyImportDisconnect.mockResolvedValue(undefined);
		host.spotifyImportStart.mockResolvedValue(fixture());
		const receipt = { jobId: 'job', playlistId: 'local', playlistRevision: 1, total: 2, accepted: 2, skipped: 0 };
		host.spotifyImportCommit.mockResolvedValue(receipt);
		host.spotifyImportReadback.mockResolvedValue([0, 1].map((position) => ({
			entryId: `stored-${position}`, position, sourceOrdinal: position, videoId: 'aaaaaaaaaaa', songJson: '{}'
		})));
		saved = vi.fn();
		target = document.createElement('div');
		document.body.append(target);
		component = mount(SpotifyImport, { target, props: { onclose: vi.fn(), onsaved: saved } });
		flushSync();
	});

	afterEach(async () => {
		if (component) await unmount(component);
		component = undefined;
		target.remove();
		vi.unstubAllGlobals();
	});

	async function loadReview() {
		await settled('Connect Spotify');
		button('Connect Spotify').click();
		await settled('Spotify playlist URL');
		const input = document.querySelector<HTMLInputElement>('#spotify-playlist-url')!;
		input.value = 'https://open.spotify.com/playlist/3zlBonH1l7BOTkoKITprOt';
		input.dispatchEvent(new Event('input', { bubbles: true }));
		flushSync();
		button('Read playlist').click();
		await settled('2 undecided');
	}

	it('resumes prepared reviews from the active session without reconnecting or discarding them', async () => {
		await unmount(component!);
		host.spotifyImportStatus.mockResolvedValue({
			enabled: true, connected: true, maxOccurrences: 1000, reviewJobs: ['job']
		});
		host.spotifyImportSnapshot.mockResolvedValue(fixture());
		component = mount(SpotifyImport, { target, props: { onclose: vi.fn(), onsaved: saved } });
		flushSync();
		await settled('Continue a prepared review');
		button('source').click();
		await settled('2 undecided');
		expect(host.spotifyImportSnapshot).toHaveBeenCalledWith('job');
		expect(host.spotifyImportConnect).not.toHaveBeenCalled();
		expect(host.spotifyImportDisconnect).not.toHaveBeenCalled();
	});

	it('requires both decisions and verifies repeated videos as distinct stored occurrences', async () => {
		await loadReview();
		expect(button('Save 0 occurrences locally').disabled).toBe(true);
		button('Accept this candidate').click();
		flushSync();
		expect(button('Save 1 occurrences locally').disabled).toBe(true);
		button('Next occurrence').click();
		flushSync();
		button('Accept this candidate').click();
		flushSync();
		button('Save 2 occurrences locally').click();
		await settled('Saved and read back');
		expect(host.spotifyImportCommit).toHaveBeenCalledWith({
			jobId: 'job', expectedReviewRevision: 2, expectedPlaylistRevision: null,
			decisions: [
				{ occurrenceId: 'entry-0', kind: 'accept', candidateId: 'candidate-0' },
				{ occurrenceId: 'entry-1', kind: 'accept', candidateId: 'candidate-1' }
			]
		});
		expect(saved).toHaveBeenCalledOnce();
	});

	it('does not claim readback verification when storage silently deduplicates a repeated video', async () => {
		host.spotifyImportReadback.mockResolvedValue([
			{ entryId: 'stored-0', position: 0, sourceOrdinal: 0, videoId: 'aaaaaaaaaaa', songJson: '{}' }
		]);
		await loadReview();
		button('Accept this candidate').click();
		button('Next occurrence').click();
		flushSync();
		button('Accept this candidate').click();
		flushSync();
		button('Save 2 occurrences locally').click();
		await settled('Saved entries differ from the reviewed selection.');
		expect(document.body.textContent).not.toContain('Saved and read back');
		expect(saved).not.toHaveBeenCalled();
		expect(host.spotifyImportCommit).toHaveBeenCalledOnce();
	});

	it('recovers a lost commit response through status without committing twice', async () => {
		host.spotifyImportCommit.mockRejectedValue(new Error('Response lost'));
		const snapshot = fixture();
		snapshot.review = { ...snapshot.review, phase: 'committed', revision: 3 };
		snapshot.receipt = { jobId: 'job', playlistId: 'local', playlistRevision: 1, total: 2, accepted: 2, skipped: 0 };
		host.spotifyImportSnapshot.mockResolvedValue(snapshot);
		await loadReview();
		button('Accept this candidate').click();
		button('Next occurrence').click();
		flushSync();
		button('Accept this candidate').click();
		flushSync();
		button('Save 2 occurrences locally').click();
		await settled('Response lost');
		button('Reload status').click();
		await settled('Saved and read back');
		expect(host.spotifyImportCommit).toHaveBeenCalledOnce();
		expect(saved).toHaveBeenCalledOnce();
	});

	it('disconnects pending authorization when the component is destroyed', async () => {
		host.spotifyImportConnect.mockReturnValue(new Promise(() => {}));
		await settled('Connect Spotify');
		button('Connect Spotify').click();
		await settled('Waiting for Spotify authorization');
		await unmount(component!);
		component = undefined;
		expect(host.spotifyImportDisconnect).toHaveBeenCalledOnce();
	});

	it('pairs selected row and candidate backgrounds with readable foreground tokens', async () => {
		await loadReview();
		const activeRow = document.querySelector('nav[aria-label="Source occurrences"] button[aria-current="step"]')!;
		expect(activeRow.classList.contains('text-accent-foreground')).toBe(true);
		expect(activeRow.querySelector('span.text-muted-foreground')).toBeNull();
		button('Accept this candidate').click();
		flushSync();
		const accepted = document.querySelector('section[aria-label="Review selected occurrence"] button[aria-pressed="true"]')!;
		expect(accepted.classList.contains('text-accent-foreground')).toBe(true);
		expect(accepted.querySelector('span.text-muted-foreground')).toBeNull();
		button('Next occurrence').click();
		flushSync();
		expect(activeRow.classList.contains('text-accent-foreground')).toBe(false);
	});

	it('searches every unsearched occurrence using current revisions without accepting anything', async () => {
		const initial = fixture();
		initial.review = { ...initial.review, occurrences: initial.review.occurrences.map((entry) => ({ ...entry, candidates: [] })) };
		host.spotifyImportStart.mockResolvedValue(initial);
		const first = fixture().review;
		host.spotifyImportSearch
			.mockResolvedValueOnce({ ...first, revision: 3, occurrences: [first.occurrences[0], { ...first.occurrences[1], candidates: [] }] })
			.mockResolvedValueOnce({ ...first, revision: 4 });
		await loadReview();
		button('Search 2 unsearched occurrences').click();
		await vi.waitFor(() => {
			flushSync();
			expect(host.spotifyImportSearch).toHaveBeenCalledTimes(2);
			expect(button('Search 0 unsearched occurrences').disabled).toBe(true);
		});
		expect(host.spotifyImportSearch.mock.calls.map((call) => call.slice(0, 3))).toEqual([
			['job', 'entry-0', 2], ['job', 'entry-1', 3]
		]);
		expect(document.body.textContent).toContain('2 undecided');
		expect(host.spotifyImportCommit).not.toHaveBeenCalled();
	});

	it('stops batch search after the in-flight result when requested', async () => {
		const initial = fixture();
		initial.review = { ...initial.review, occurrences: initial.review.occurrences.map((entry) => ({ ...entry, candidates: [] })) };
		host.spotifyImportStart.mockResolvedValue(initial);
		let resolve!: (value: SpotifyLabReview['review']) => void;
		host.spotifyImportSearch.mockReturnValue(new Promise((yes) => { resolve = yes; }));
		await loadReview();
		button('Search 2 unsearched occurrences').click();
		await settled('Searching candidates 1 of 2');
		button('Stop after current search').click();
		const result = fixture().review;
		resolve({ ...result, revision: 3, occurrences: [result.occurrences[0], { ...result.occurrences[1], candidates: [] }] });
		await vi.waitFor(() => {
			flushSync();
			expect(button('Search 1 unsearched occurrences').disabled).toBe(false);
		});
		expect(host.spotifyImportSearch).toHaveBeenCalledOnce();
	});

	it('stops at the first search failure without retrying or discarding undecided entries', async () => {
		const initial = fixture();
		initial.review = { ...initial.review, occurrences: initial.review.occurrences.map((entry) => ({ ...entry, candidates: [] })) };
		host.spotifyImportStart.mockResolvedValue(initial);
		host.spotifyImportSearch.mockRejectedValue(new Error('Search unavailable'));
		await loadReview();
		button('Search 2 unsearched occurrences').click();
		await settled('Search unavailable');
		expect(host.spotifyImportSearch).toHaveBeenCalledOnce();
		expect(document.body.textContent).toContain('2 undecided');
		expect(host.spotifyImportCommit).not.toHaveBeenCalled();
	});

	it.each(['source_error', 'unsupported'] as const)(
		'lets the user manually search and accept a candidate for %s source entries',
		async (outcome) => {
			const initial = fixture();
			initial.review = {
				...initial.review,
				occurrences: initial.review.occurrences.map((entry, index) =>
					index === 0 ? { ...entry, outcome, candidates: [] } : entry
				)
			};
			initial.sources[0].outcome = outcome;
			host.spotifyImportStart.mockResolvedValue(initial);
			const updated = {
				...initial.review,
				revision: 3,
				occurrences: initial.review.occurrences.map((entry, index) =>
					index === 0 ? {
						...entry,
						candidates: [{
							candidateId: 'manual-candidate',
							videoId: 'bbbbbbbbbbb',
							title: 'Manually resolved song',
							artists: ['Chosen artist'],
							durationMs: null
						}]
					} : entry
				)
			};
			host.spotifyImportSearch.mockResolvedValue(updated);
			await loadReview();

			expect(document.body.textContent).toContain('Enter a search manually');
			const input = document.querySelector<HTMLInputElement>('#spotify-candidate-query')!;
			input.value = 'artist title manually entered';
			input.dispatchEvent(new Event('input', { bubbles: true }));
			flushSync();
			button('Search candidates').click();
			await settled('Manually resolved song');
			expect(host.spotifyImportSearch).toHaveBeenCalledWith(
				'job', 'entry-0', 2, 'artist title manually entered'
			);

			button('Accept this candidate').click();
			flushSync();
			expect(document.body.textContent).toContain('1 accepted · 0 skipped · 1 undecided');
			expect(host.spotifyImportCommit).not.toHaveBeenCalled();
		}
	);

	it('can clear an explicit omission and leave the occurrence pending', async () => {
		await loadReview();
		button('Omit this occurrence').click();
		flushSync();
		expect(document.body.textContent).toContain('0 accepted · 1 skipped · 1 undecided');
		button('Leave pending').click();
		flushSync();
		expect(document.body.textContent).toContain('0 accepted · 0 skipped · 2 undecided');
		expect(button('Save 0 occurrences locally').disabled).toBe(true);
		expect(document.body.textContent).not.toContain('Skip 2 undecided occurrences');
		expect(host.spotifyImportCommit).not.toHaveBeenCalled();
	});

	it('keeps authorization while starting another playlist after verified save', async () => {
		await loadReview();
		button('Accept this candidate').click();
		button('Next occurrence').click();
		flushSync();
		button('Accept this candidate').click();
		flushSync();
		button('Save 2 occurrences locally').click();
		await settled('Saved and read back');
		button('Import another playlist').click();
		await settled('Spotify playlist URL');
		expect(host.spotifyImportConnect).toHaveBeenCalledOnce();
		expect(host.spotifyImportDisconnect).not.toHaveBeenCalled();
		expect(document.querySelector<HTMLInputElement>('#spotify-playlist-url')?.value).toBe('');
	});
});
