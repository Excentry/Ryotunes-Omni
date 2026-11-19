export type SpotifyImportOutcome =
	| 'matched'
	| 'low_confidence'
	| 'not_found'
	| 'unsupported'
	| 'source_error'
	| 'search_error';

export type SpotifyImportPhase =
	| 'loading'
	| 'matching'
	| 'review'
	| 'committing'
	| 'committed'
	| 'cancelled'
	| 'failed';

export interface SpotifyImportCandidate {
	readonly candidateId: string;
	readonly videoId: string;
	readonly title: string;
	readonly artists: readonly string[];
	readonly durationMs: number | null;
}

export interface SpotifyImportOccurrence {
	readonly occurrenceId: string;
	readonly ordinal: number;
	readonly outcome: SpotifyImportOutcome;
	readonly candidates: readonly SpotifyImportCandidate[];
}

export interface SpotifyImportReview {
	readonly jobId: string;
	readonly revision: number;
	readonly phase: SpotifyImportPhase;
	readonly source: {
		readonly accountKey: string;
		readonly playlistId: string;
		readonly snapshotId: string;
		readonly total: number;
	};
	readonly target:
		| { readonly playlistId: null; readonly revision: null }
		| { readonly playlistId: string; readonly revision: number };
	readonly occurrences: readonly SpotifyImportOccurrence[];
}

export type SpotifyImportDecision =
	| { readonly occurrenceId: string; readonly kind: 'skip' }
	| { readonly occurrenceId: string; readonly kind: 'accept'; readonly candidateId: string };

export interface SpotifyImportSummary {
	readonly total: number;
	readonly accepted: number;
	readonly skipped: number;
	readonly pending: number;
	readonly outcomes: Readonly<Record<SpotifyImportOutcome, number>>;
}

export interface SpotifyImportCommitRequest {
	readonly jobId: string;
	readonly expectedReviewRevision: number;
	readonly expectedPlaylistRevision: number | null;
	readonly decisions: readonly SpotifyImportDecision[];
}

export interface SpotifyImportCommitPlan {
	readonly request: SpotifyImportCommitRequest;
	readonly summary: SpotifyImportSummary;
	readonly preview: readonly {
		readonly occurrenceId: string;
		readonly sourceOrdinal: number;
		readonly candidateId: string;
		readonly videoId: string;
	}[];
}

const outcomes: readonly SpotifyImportOutcome[] = [
	'matched',
	'low_confidence',
	'not_found',
	'unsupported',
	'source_error',
	'search_error'
];

function requireText(value: string, label: string): void {
	if (typeof value !== 'string' || value.trim().length === 0) {
		throw new Error(`Missing ${label}.`);
	}
}

function requireRevision(value: number, label: string): void {
	if (!Number.isSafeInteger(value) || value < 0) throw new Error(`Invalid ${label}.`);
}

function reviewEntries(review: SpotifyImportReview): readonly SpotifyImportOccurrence[] {
	requireText(review.jobId, 'job ID');
	requireRevision(review.revision, 'review revision');
	requireText(review.source.accountKey, 'source account key');
	requireText(review.source.playlistId, 'source playlist ID');
	requireText(review.source.snapshotId, 'source snapshot ID');
	requireRevision(review.source.total, 'source total');
	if (review.target.playlistId !== null) {
		requireText(review.target.playlistId, 'target playlist ID');
		requireRevision(review.target.revision, 'target playlist revision');
	} else if (review.target.revision !== null) {
		throw new Error('A new target cannot have an existing revision.');
	}
	if (review.occurrences.length !== review.source.total) {
		throw new Error('Source occurrences are incomplete.');
	}
	const ids = new Set<string>();
	const ordered = [...review.occurrences].sort((left, right) => left.ordinal - right.ordinal);
	for (const [index, occurrence] of ordered.entries()) {
		requireText(occurrence.occurrenceId, 'occurrence ID');
		if (ids.has(occurrence.occurrenceId)) throw new Error('Repeated occurrence ID.');
		ids.add(occurrence.occurrenceId);
		if (occurrence.ordinal !== index) throw new Error('Source ordinals must be contiguous.');
		if (!outcomes.includes(occurrence.outcome)) throw new Error('Unknown matching outcome.');
		const candidates = new Set<string>();
		for (const candidate of occurrence.candidates) {
			requireText(candidate.candidateId, 'candidate ID');
			if (candidates.has(candidate.candidateId)) throw new Error('Repeated candidate ID.');
			candidates.add(candidate.candidateId);
			if (!/^[A-Za-z0-9_-]{11}$/.test(candidate.videoId)) {
				throw new Error('Candidate must reference a YouTube video ID.');
			}
			requireText(candidate.title, 'candidate title');
			if (candidate.artists.length === 0) throw new Error('Candidate has no artists.');
			for (const artist of candidate.artists) requireText(artist, 'candidate artist');
			if (
				candidate.durationMs !== null &&
				(!Number.isSafeInteger(candidate.durationMs) || candidate.durationMs <= 0)
			) {
				throw new Error('Invalid candidate duration.');
			}
		}
	}
	return ordered;
}

function decisionMap(
	entries: readonly SpotifyImportOccurrence[],
	decisions: readonly SpotifyImportDecision[]
): ReadonlyMap<string, SpotifyImportDecision> {
	const occurrences = new Map(entries.map((entry) => [entry.occurrenceId, entry]));
	const selected = new Map<string, SpotifyImportDecision>();
	for (const decision of decisions) {
		const occurrence = occurrences.get(decision.occurrenceId);
		if (!occurrence) throw new Error('Decision references another occurrence.');
		if (selected.has(decision.occurrenceId)) throw new Error('Repeated decision.');
		if (decision.kind === 'accept') {
			if (!occurrence.candidates.some((candidate) => candidate.candidateId === decision.candidateId)) {
				throw new Error('Decision references an unoffered candidate.');
			}
		} else if (decision.kind !== 'skip') {
			throw new Error('Unknown review decision.');
		}
		selected.set(decision.occurrenceId, decision);
	}
	return selected;
}

export function summarizeSpotifyImport(
	review: SpotifyImportReview,
	decisions: readonly SpotifyImportDecision[]
): SpotifyImportSummary {
	const entries = reviewEntries(review);
	const selected = decisionMap(entries, decisions);
	const counts: Record<SpotifyImportOutcome, number> = {
		matched: 0,
		low_confidence: 0,
		not_found: 0,
		unsupported: 0,
		source_error: 0,
		search_error: 0
	};
	let accepted = 0;
	let skipped = 0;
	for (const entry of entries) {
		counts[entry.outcome]++;
		const decision = selected.get(entry.occurrenceId);
		if (decision?.kind === 'accept') accepted++;
		if (decision?.kind === 'skip') skipped++;
	}
	return {
		total: entries.length,
		accepted,
		skipped,
		pending: entries.length - accepted - skipped,
		outcomes: counts
	};
}

export function prepareSpotifyImportCommit(
	review: SpotifyImportReview,
	decisions: readonly SpotifyImportDecision[],
	expectedReviewRevision: number
): SpotifyImportCommitPlan {
	if (review.phase !== 'review') throw new Error('Import job is not open for review.');
	requireRevision(expectedReviewRevision, 'expected review revision');
	if (expectedReviewRevision !== review.revision) throw new Error('Review revision changed.');
	const entries = reviewEntries(review);
	const selected = decisionMap(entries, decisions);
	const summary = summarizeSpotifyImport(review, decisions);
	if (summary.pending !== 0) throw new Error('Every occurrence needs an explicit decision.');
	if (summary.accepted === 0) throw new Error('An empty selection cannot replace a playlist.');
	const ordered: SpotifyImportDecision[] = [];
	const preview: SpotifyImportCommitPlan['preview'][number][] = [];
	for (const entry of entries) {
		const decision = selected.get(entry.occurrenceId);
		if (!decision) throw new Error('Missing occurrence decision.');
		if (decision.kind === 'skip') {
			ordered.push({ occurrenceId: entry.occurrenceId, kind: 'skip' });
			continue;
		}
		const candidate = entry.candidates.find((item) => item.candidateId === decision.candidateId);
		if (!candidate) throw new Error('Selected candidate disappeared.');
		ordered.push({
			occurrenceId: entry.occurrenceId,
			kind: 'accept',
			candidateId: candidate.candidateId
		});
		preview.push({
			occurrenceId: entry.occurrenceId,
			sourceOrdinal: entry.ordinal,
			candidateId: candidate.candidateId,
			videoId: candidate.videoId
		});
	}
	return {
		request: {
			jobId: review.jobId,
			expectedReviewRevision,
			expectedPlaylistRevision: review.target.revision,
			decisions: ordered
		},
		summary,
		preview
	};
}
