import {
	prepareSpotifyImportCommit,
	summarizeSpotifyImport,
	type SpotifyImportDecision,
	type SpotifyImportReview
} from './spotify-import.ts';

function ok(condition: boolean, message: string): void {
	if (!condition) throw new Error(`FAIL: ${message}`);
}

function rejects(action: () => unknown, message: string): void {
	let rejected = false;
	try {
		action();
	} catch {
		rejected = true;
	}
	ok(rejected, message);
}

const review = (): SpotifyImportReview => ({
	jobId: 'job-1',
	revision: 4,
	phase: 'review',
	source: {
		accountKey: 'account-a',
		playlistId: 'source-playlist',
		snapshotId: 'source-snapshot',
		total: 3
	},
	target: { playlistId: 'local-import', revision: 8 },
	occurrences: [
		{
			occurrenceId: 'entry-a',
			ordinal: 0,
			outcome: 'matched',
			candidates: [
				{
					candidateId: 'candidate-a',
					videoId: 'aaaaaaaaaaa',
					title: 'Example',
					artists: ['Artist'],
					durationMs: 180000
				}
			]
		},
		{ occurrenceId: 'entry-b', ordinal: 1, outcome: 'source_error', candidates: [] },
		{
			occurrenceId: 'entry-c',
			ordinal: 2,
			outcome: 'low_confidence',
			candidates: [
				{
					candidateId: 'candidate-c',
					videoId: 'aaaaaaaaaaa',
					title: 'Example',
					artists: ['Artist'],
					durationMs: 180000
				}
			]
		}
	]
});

const decisions: readonly SpotifyImportDecision[] = [
	{ occurrenceId: 'entry-c', kind: 'accept', candidateId: 'candidate-c' },
	{ occurrenceId: 'entry-b', kind: 'skip' },
	{ occurrenceId: 'entry-a', kind: 'accept', candidateId: 'candidate-a' }
];

const initial = review();
const serialized = JSON.stringify(initial);
const plan = prepareSpotifyImportCommit(initial, decisions, 4);
ok(plan.preview.length === 2, 'both appearances of the same video are retained');
ok(
	plan.preview.map((entry) => entry.occurrenceId).join(',') === 'entry-a,entry-c',
	'commit preserves source order, not decision order'
);
ok(
	plan.preview.map((entry) => entry.sourceOrdinal).join(',') === '0,2',
	'skipped source position remains represented in the ledger'
);
ok(JSON.stringify(initial) === serialized, 'planning does not mutate the source review');
ok(!JSON.stringify(plan.request).includes('videoId'), 'host resolves candidate metadata itself');
ok(plan.request.expectedPlaylistRevision === 8, 'target revision is carried to commit');
ok(plan.request.decisions.length === 3, 'all source occurrences are represented');
ok(plan.summary.accepted === 2 && plan.summary.skipped === 1, 'selection counts reconcile');
ok(plan.summary.outcomes.source_error === 1, 'source errors are not search misses');
ok(summarizeSpotifyImport(initial, []).pending === 3, 'matches require explicit decisions too');

rejects(() => prepareSpotifyImportCommit(initial, decisions, 3), 'stale review cannot commit');
for (const phase of ['cancelled', 'failed', 'committing', 'committed', 'loading', 'matching'] as const) {
	rejects(
		() => prepareSpotifyImportCommit({ ...initial, phase }, decisions, 4),
		`${phase} job cannot commit`
	);
}
rejects(() => prepareSpotifyImportCommit(initial, decisions.slice(1), 4), 'pending review cannot commit');
rejects(
	() =>
		prepareSpotifyImportCommit(
			initial,
			initial.occurrences.map((entry) => ({ occurrenceId: entry.occurrenceId, kind: 'skip' })),
			4
		),
	'empty selection cannot erase an existing playlist'
);
rejects(
	() => prepareSpotifyImportCommit(initial, [...decisions, decisions[0]], 4),
	'duplicate decisions cannot inflate accepted counts'
);
rejects(
	() =>
		prepareSpotifyImportCommit(
			initial,
			[{ occurrenceId: 'foreign-entry', kind: 'skip' }, ...decisions],
			4
		),
	'decisions cannot reference another job occurrence'
);
rejects(
	() =>
		prepareSpotifyImportCommit(
			initial,
			[
				{ occurrenceId: 'entry-a', kind: 'accept', candidateId: 'candidate-c' },
				...decisions.slice(0, 2)
			],
			4
		),
	'candidates are scoped to each source occurrence'
);
rejects(
	() =>
		prepareSpotifyImportCommit(
			{ ...initial, source: { ...initial.source, total: 4 } },
			decisions,
			4
		),
	'incomplete source cannot commit'
);
rejects(
	() =>
		prepareSpotifyImportCommit(
			{
				...initial,
				occurrences: initial.occurrences.map((entry, index) =>
					index === 2 ? { ...entry, ordinal: 3 } : entry
				)
			},
			decisions,
			4
		),
	'ordinal gaps cannot silently discard occurrences'
);
rejects(
	() =>
		prepareSpotifyImportCommit(
			{
				...initial,
				occurrences: initial.occurrences.map((entry, index) =>
					index === 2 ? { ...entry, occurrenceId: 'entry-a' } : entry
				)
			},
			decisions,
			4
		),
	'repeated occurrence IDs are rejected'
);
const invalidVideo: SpotifyImportReview = {
	...initial,
	occurrences: initial.occurrences.map((entry, index) =>
		index === 0
			? { ...entry, candidates: [{ ...entry.candidates[0], videoId: 'LOCAL:private-path' }] }
			: entry
	)
};
rejects(
	() => prepareSpotifyImportCommit(invalidVideo, decisions, 4),
	'local paths cannot be submitted as matched video IDs'
);
const unavailable: SpotifyImportReview = {
	...initial,
	occurrences: initial.occurrences.map((entry, index) =>
		index === 0 ? { ...entry, outcome: 'source_error' } : entry
	)
};
const manuallyResolved = prepareSpotifyImportCommit(unavailable, decisions, 4);
ok(
	manuallyResolved.preview.some((entry) => entry.occurrenceId === 'entry-a'),
	'manual candidates can resolve source errors when the host offers them'
);
rejects(
	() => prepareSpotifyImportCommit({ ...initial, revision: 4.5 }, decisions, 4.5),
	'fractional revisions cannot be committed'
);

const newPlaylist = prepareSpotifyImportCommit(
	{ ...initial, target: { playlistId: null, revision: null } },
	decisions,
	4
);
ok(newPlaylist.request.expectedPlaylistRevision === null, 'new target is distinct from replacement');

const large: SpotifyImportReview = {
	...initial,
	source: { ...initial.source, total: 1251 },
	occurrences: Array.from({ length: 1251 }, (_, ordinal) => ({
		...initial.occurrences[0],
		occurrenceId: `entry-${ordinal}`,
		ordinal
	}))
};
const largePlan = prepareSpotifyImportCommit(
	large,
	large.occurrences.map((entry) => ({
		occurrenceId: entry.occurrenceId,
		kind: 'accept',
		candidateId: 'candidate-a'
	})),
	4
);
ok(largePlan.preview.length === 1251, '1251 repeated video IDs remain 1251 playlist occurrences');
ok(largePlan.preview[1250].sourceOrdinal === 1250, 'last large-list position is preserved');

console.log('ok: Spotify import review contract, scoped decisions, revisions and repeated occurrences');
