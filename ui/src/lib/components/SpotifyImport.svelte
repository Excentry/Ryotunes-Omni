<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as api from '$lib/api';
	import {
		prepareSpotifyImportCommit,
		summarizeSpotifyImport,
		type SpotifyImportDecision,
		type SpotifyImportCommitPlan
	} from '$lib/spotify-import';

	let { onclose, onsaved, maxOccurrences = 100 }: {
		onclose: () => void; onsaved: () => void; maxOccurrences?: number;
	} = $props();
	let connected = $state(false);
	let statusReady = $state(false);
	let busy = $state('');
	let error = $state('');
	let playlistUrl = $state('');
	let staged = $state<api.SpotifyLabReview | null>(null);
	let preparedReviews = $state<api.SpotifyLabReview[]>([]);
	let selections = $state<Record<string, string>>({});
	let index = $state(0);
	let query = $state('');
	let verified = $state(false);
	let expectedPlan: SpotifyImportCommitPlan | null = null;
	let disposed = false;
	let stopBatch = $state(false);
	let batchSearching = $state(false);

	const decisions = $derived<SpotifyImportDecision[]>(
		(staged?.review.occurrences ?? []).flatMap<SpotifyImportDecision>((entry) => {
			const value = selections[entry.occurrenceId];
			if (!value) return [];
			return value === 'skip'
				? [{ occurrenceId: entry.occurrenceId, kind: 'skip' as const }]
				: [{ occurrenceId: entry.occurrenceId, kind: 'accept' as const, candidateId: value }];
		})
	);
	const summary = $derived(staged ? summarizeSpotifyImport(staged.review, decisions) : null);
	const current = $derived(staged?.review.occurrences[index]);
	const original = $derived(staged?.sources[index]);
	const unsearched = $derived((staged?.review.occurrences ?? []).filter((entry) =>
		entry.outcome !== 'unsupported' && entry.outcome !== 'source_error' &&
		entry.candidates.length === 0 && !selections[entry.occurrenceId]
	));

	onDestroy(() => {
		disposed = true;
		if (busy.startsWith('Waiting for Spotify authorization')) {
			void api.spotifyImportDisconnect().catch(() => {});
		}
	});

	onMount(() => {
		void (async () => {
			try {
				const status = await api.spotifyImportStatus();
				if (disposed) return;
				connected = status.connected;
				if (status.connected && status.reviewJobs.length > 0) {
					busy = 'Loading prepared reviews…';
					preparedReviews = await Promise.all(
						status.reviewJobs.map((jobId) => api.spotifyImportSnapshot(jobId))
					);
				}
			} catch (cause) {
				if (!disposed) error = String(cause);
			} finally {
				if (!disposed) {
					statusReady = true;
					busy = '';
				}
			}
		})();
	});

	function selectIndex(next: number) {
		index = next;
		const source = staged?.sources[next];
		query = source ? [source.artists.join(' '), source.title].filter(Boolean).join(' ') : '';
	}

	function nextUndecided() {
		const entries = staged?.review.occurrences ?? [];
		if (!entries.length) return;
		for (let offset = 1; offset <= entries.length; offset += 1) {
			const next = (index + offset) % entries.length;
			if (!selections[entries[next].occurrenceId]) {
				selectIndex(next);
				return;
			}
		}
	}

	function leavePending() {
		if (current) delete selections[current.occurrenceId];
	}

	function resume(review: api.SpotifyLabReview) {
		staged = review;
		selections = {};
		selectIndex(0);
	}

	async function run(label: string, action: () => Promise<void>) {
		if (busy) return;
		busy = label;
		error = '';
		try {
			await action();
		} catch (cause) {
			if (!disposed) error = String(cause);
		} finally {
			if (!disposed) busy = '';
		}
	}

	function connect() {
		void run('Waiting for Spotify authorization in your browser…', async () => {
			await api.spotifyImportConnect();
			if (!disposed) connected = true;
		});
	}

	function load() {
		void run('Reading the complete Spotify playlist…', async () => {
			const result = await api.spotifyImportStart(playlistUrl);
			if (disposed) return;
			staged = result;
			selections = {};
			selectIndex(0);
		});
	}

	function search() {
		const entry = current;
		const snapshot = staged;
		if (!entry || !snapshot) return;
		void run('Searching YouTube Music candidates…', async () => {
			const result = await api.spotifyImportSearch(
				snapshot.review.jobId, entry.occurrenceId, snapshot.review.revision, query
			);
			if (disposed) return;
			staged = { ...snapshot, review: result };
			delete selections[entry.occurrenceId];
		});
	}

	function searchRemaining() {
		const snapshot = staged;
		if (!snapshot) return;
		void run('Preparing candidate searches…', async () => {
			batchSearching = true;
			stopBatch = false;
			try {
				const entries = [...unsearched];
				for (const [position, entry] of entries.entries()) {
					if (disposed || stopBatch) break;
					const source = snapshot.sources[entry.ordinal];
					if (!source || !staged) throw new Error('Source metadata is no longer available.');
					selectIndex(entry.ordinal);
					busy = `Searching candidates ${position + 1} of ${entries.length}…`;
					const result = await api.spotifyImportSearch(
						staged.review.jobId, entry.occurrenceId, staged.review.revision,
						`${source.artists.join(' ')} ${source.title}`
					);
					if (disposed) return;
					staged = { ...staged, review: result };
				}
			} finally {
				if (!disposed) batchSearching = false;
			}
		});
	}

	function anotherPlaylist() {
		if (busy || !verified) return;
		const previousJobId = staged?.review.jobId;
		staged = null;
		selections = {};
		preparedReviews = preparedReviews.filter((review) => review.review.jobId !== previousJobId);
		playlistUrl = '';
		query = '';
		index = 0;
		verified = false;
		expectedPlan = null;
		error = '';
	}

	async function readback() {
		if (!staged) return;
		const rows = await api.spotifyImportReadback(staged.review.jobId);
		if (disposed) return;
		if (!expectedPlan) throw new Error('Saved import found. Reopen its playlist to inspect the stored entries.');
		if (
			rows.length !== expectedPlan.preview.length ||
			rows.some((row, position) =>
				row.position !== position ||
				row.videoId !== expectedPlan!.preview[position].videoId ||
				row.sourceOrdinal !== expectedPlan!.preview[position].sourceOrdinal
			) ||
			new Set(rows.map((row) => row.entryId)).size !== rows.length
		) throw new Error('Saved entries differ from the reviewed selection. Do not retry the import.');
		verified = true;
		onsaved();
	}

	function save() {
		if (!staged) return;
		const snapshot = staged;
		void run('Saving to the isolated local database…', async () => {
			expectedPlan = prepareSpotifyImportCommit(snapshot.review, decisions, snapshot.review.revision);
			const receipt = await api.spotifyImportCommit(expectedPlan.request);
			if (disposed) return;
			staged = { ...snapshot, review: { ...snapshot.review, phase: 'committed' }, receipt };
			await readback();
		});
	}

	function recover() {
		if (!staged) return;
		const id = staged.review.jobId;
		void run('Checking authoritative import status…', async () => {
			const result = await api.spotifyImportSnapshot(id);
			if (disposed) return;
			staged = result;
			if (result.receipt) await readback();
		});
	}

	async function close() {
		onclose();
	}
</script>

<Dialog.Root open onOpenChange={(open) => { if (!open) void close(); }}>
	<Dialog.Content class="ryo-overlay-sheet sm:max-w-5xl max-h-[85vh] overflow-y-auto" showCloseButton={false}>
		<Dialog.Header>
			<Dialog.Title>Import from Spotify</Dialog.Title>
			<Dialog.Description>
				Experimental session in an isolated profile. Review every occurrence before saving locally.
				Spotify and YouTube playlists will not be changed.
			</Dialog.Description>
		</Dialog.Header>

		{#if !statusReady}
			<p role="status" class="rounded-md border p-4 text-sm">Checking the isolated import session…</p>
		{:else if !connected}
			<div class="space-y-4">
				<p class="text-sm text-muted-foreground">
					Connect your Spotify developer test account. Authorization opens in your browser.
					Tokens stay in memory and are discarded when you close this window.
				</p>
				<Button onclick={connect} disabled={!!busy}>Connect Spotify</Button>
			</div>
		{:else if !staged}
			<div class="space-y-5">
				{#if preparedReviews.length}
					<section class="space-y-2" aria-label="Prepared imports">
						<h3 class="font-semibold">Continue a prepared review</h3>
						{#each preparedReviews as review (review.review.jobId)}
							<Button variant="outline" class="h-auto w-full justify-between gap-3 whitespace-normal text-left"
								disabled={!!busy} onclick={() => resume(review)}>
								<span class="min-w-0 break-all">{review.review.source.playlistId}</span>
								<span class="shrink-0 text-muted-foreground">{review.review.source.total} occurrences</span>
							</Button>
						{/each}
					</section>
				{/if}
				<form class="space-y-4" onsubmit={(event) => { event.preventDefault(); load(); }}>
				<div class="space-y-2">
					<label for="spotify-playlist-url" class="text-sm font-semibold">Spotify playlist URL</label>
					<Input id="spotify-playlist-url" bind:value={playlistUrl} type="url" required
						placeholder="https://open.spotify.com/playlist/…" disabled={!!busy} />
					<p class="text-sm text-muted-foreground">
						Choose an owned or collaborative playlist with 1 to {maxOccurrences} entries. The whole playlist is read.
					</p>
				</div>
				<Button type="submit" disabled={!!busy || !playlistUrl.trim()}>Read playlist</Button>
				</form>
			</div>
		{:else if staged.receipt}
			<div class="space-y-4" role="status">
				<h3 class="text-xl font-semibold">{verified ? 'Saved and read back' : 'Saved locally'}</h3>
				<p class="text-sm">
					{staged.receipt.accepted} saved occurrences, {staged.receipt.skipped} skipped.
					{#if verified}Stored order, video IDs and independent occurrence IDs match your review.{/if}
				</p>
				<p class="text-sm text-muted-foreground break-all">{staged.receipt.playlistId}</p>
				<p class="text-sm text-muted-foreground">
					This verifies storage, not recording accuracy. Playlist editing remains experimental.
				</p>
				{#if !verified}<Button variant="outline" onclick={recover} disabled={!!busy}>Verify saved entries</Button>{/if}
				{#if verified}<Button variant="outline" onclick={anotherPlaylist} disabled={!!busy}>Import another playlist</Button>{/if}
			</div>
		{:else if current && original && summary}
			<div class="flex flex-wrap items-center justify-between gap-4 text-sm">
				<p aria-live="polite">{summary.accepted} accepted · {summary.skipped} skipped · {summary.pending} undecided</p>
				<p class="text-muted-foreground">
					{staged.review.target.playlistId ? 'Replaces the prior import in this test profile' : 'New local playlist in this test profile'}
				</p>
			</div>
			<div class="flex flex-wrap items-center gap-2">
				<Button variant="outline" onclick={searchRemaining} disabled={!!busy || unsearched.length === 0}>
					Search {unsearched.length} unsearched occurrences
				</Button>
				{#if batchSearching}
					<Button variant="outline" onclick={() => (stopBatch = true)} disabled={stopBatch}>
						{stopBatch ? 'Stopping after current search…' : 'Stop after current search'}
					</Button>
				{/if}
				<p class="text-sm text-muted-foreground">Search proposes candidates only. Every acceptance remains your choice.</p>
			</div>
			<div class="grid gap-6 md:grid-cols-[16rem_minmax(0,1fr)]">
				<nav aria-label="Source occurrences" class="max-h-96 overflow-y-auto rounded-md border p-2">
					{#each staged.review.occurrences as entry, position (entry.occurrenceId)}
						<button type="button" onclick={() => selectIndex(position)} disabled={!!busy}
							aria-current={index === position ? 'step' : undefined}
							class="group w-full rounded-sm p-2 text-left text-sm hover:bg-accent hover:text-accent-foreground focus-visible:outline-2 focus-visible:outline-ring disabled:opacity-50"
							class:bg-accent={index === position}
							class:text-accent-foreground={index === position}>
							<span class="font-semibold">{position + 1}. {staged.sources[position].title}</span>
							<span class="block text-xs group-hover:text-accent-foreground"
								class:text-muted-foreground={index !== position}
								class:text-accent-foreground={index === position}>
								{!selections[entry.occurrenceId] ? 'Undecided' : selections[entry.occurrenceId] === 'skip' ? 'Skipped' : 'Accepted'}
							</span>
						</button>
					{/each}
				</nav>
				<section aria-label="Review selected occurrence" class="min-w-0 space-y-4">
					<div class="space-y-2">
						<h3 class="text-xl font-semibold break-words">{original.title}</h3>
						<p class="text-sm text-muted-foreground">{original.artists.join(' & ')} · {original.durationMs ? `${Math.round(original.durationMs / 1000)} s` : 'Duration unavailable'}</p>
						<p class="text-sm text-muted-foreground">
							{current.outcome === 'unsupported' || current.outcome === 'source_error'
								? 'Source metadata is incomplete or unsupported. Enter a search manually to choose a replacement, or leave this occurrence pending.'
								: 'Candidates are unverified. Compare title, artists, duration and version before accepting.'}
						</p>
					</div>
					<form class="flex flex-wrap gap-2" onsubmit={(event) => { event.preventDefault(); search(); }}>
						<label class="sr-only" for="spotify-candidate-query">YouTube Music search</label>
						<Input id="spotify-candidate-query" bind:value={query} class="min-w-0 flex-1" maxlength={500} disabled={!!busy} />
						<Button type="submit" variant="outline" disabled={!!busy || !query.trim()}>Search candidates</Button>
					</form>
					<div class="space-y-2">
						{#each current.candidates as candidate (candidate.candidateId)}
							<button type="button" disabled={!!busy}
								aria-pressed={selections[current.occurrenceId] === candidate.candidateId}
								onclick={() => { if (current) selections[current.occurrenceId] = candidate.candidateId; }}
								class="group w-full rounded-md border p-3 text-left text-sm hover:bg-accent hover:text-accent-foreground focus-visible:outline-2 focus-visible:outline-ring disabled:opacity-50"
								class:bg-accent={selections[current.occurrenceId] === candidate.candidateId}
								class:text-accent-foreground={selections[current.occurrenceId] === candidate.candidateId}>
								<strong class="block break-words">{candidate.title}</strong>
								<span class="block group-hover:text-accent-foreground"
									class:text-muted-foreground={selections[current.occurrenceId] !== candidate.candidateId}
									class:text-accent-foreground={selections[current.occurrenceId] === candidate.candidateId}>{candidate.artists.join(' & ')} · {candidate.durationMs ? `${Math.round(candidate.durationMs / 1000)} s` : 'Duration unavailable'}</span>
								<span class="block text-xs group-hover:text-accent-foreground"
									class:text-muted-foreground={selections[current.occurrenceId] !== candidate.candidateId}
									class:text-accent-foreground={selections[current.occurrenceId] === candidate.candidateId}>{candidate.videoId} · {selections[current.occurrenceId] === candidate.candidateId ? 'Accepted for this occurrence' : 'Accept this candidate'}</span>
							</button>
						{:else}
							<p class="rounded-md border p-4 text-sm text-muted-foreground">No candidates offered. Search for alternatives; this occurrence stays pending until you choose.</p>
						{/each}
					</div>
					<div class="flex flex-wrap gap-2">
						<Button variant="outline" disabled={!!busy} onclick={() => { if (current) selections[current.occurrenceId] = 'skip'; }}>Omit this occurrence</Button>
						<Button variant="ghost" disabled={!!busy || !selections[current.occurrenceId]} onclick={leavePending}>Leave pending</Button>
						<Button variant="ghost" disabled={!!busy || summary.pending === 0} onclick={nextUndecided}>Next undecided</Button>
						<Button variant="ghost" disabled={!!busy || index === staged.review.occurrences.length - 1} onclick={() => selectIndex(index + 1)}>Next occurrence</Button>
					</div>
				</section>
			</div>
			<div class="flex flex-wrap justify-end gap-2">
				<Button variant="outline" onclick={recover} disabled={!!busy}>Reload status</Button>
				<Button onclick={save} disabled={!!busy || summary.pending !== 0 || summary.accepted === 0 || staged.review.phase !== 'review'}>
					Save {summary.accepted} occurrences locally
				</Button>
			</div>
		{/if}

		{#if busy}<p role="status" class="rounded-md border p-4 text-sm">{busy}</p>{/if}
		{#if error}<p role="alert" class="rounded-md border p-4 text-sm">{error}</p>{/if}
		<div class="flex justify-end">
			<Button variant="ghost" onclick={() => void close()}>Close importer</Button>
		</div>
	</Dialog.Content>
</Dialog.Root>
