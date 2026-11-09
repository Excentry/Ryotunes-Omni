<script lang="ts">
	import { HugeiconsIcon, type IconSvgElement } from '@hugeicons/svelte';
	import {
		FavouriteIcon,
		MusicNote01Icon,
		PlayIcon,
		PlayListAddIcon,
		ThumbsDownIcon,
		ThumbsUpIcon,
		Loading03Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { SongItem } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import { lt } from '$lib/lt.svelte';
	import { anySaved, isLiked, playback, ratingOf, savedPlaylists, toggleRating } from '$lib/player.svelte';
	import { warmStream } from '$lib/warm-stream';
	import SavedInPlaylists from './SavedInPlaylists.svelte';
	import TrackMenu from './TrackMenu.svelte';
	import ArtistLine from './ArtistLine.svelte';
	import ExplicitIcon from './ExplicitIcon.svelte';

	let {
		song,
		index,
		active = false,
		hideThumb = false,
		compact = false,
		showPlayCount = false,
		hideRating = false,
		showMenu = true,
		contextMenu = true,
		onplay,
		onAdd,
		onRemove,
		onMoveUp,
		onMoveDown,
		removeLabel = 'Remove from playlist'
	}: {
		song: SongItem;
		/** Position badge when set (playlist/queue); omitted for flat search results. */
		index?: number;
		active?: boolean;
		/** Hide the leading thumbnail (album track lists show a number, not a cover). */
		hideThumb?: boolean;
		/**
		 * Grid variant (home's Forgotten favourites): the duration joins the artist line instead of
		 * claiming its own column, and a like heart sits next to the ⋯ — narrow columns have no room
		 * for a separate duration column, and hearting is the whole point of that shelf.
		 */
		compact?: boolean;
		/**
		 * Opt-in, because `play_count` rides along on the song object wherever it goes after an album
		 * page (queue, previously played) and a narrow panel has no width to spare for it.
		 */
		showPlayCount?: boolean;
		/**
		 * The narrow queue-panel variant: drops the inline thumbs and the explicit mark. Two buttons
		 * plus the duration leave nothing for the title and artists at that width, and the queue is
		 * not where you decide what to listen to. The ⋯ menu carries like and dislike either way.
		 */
		hideRating?: boolean;
		/** Show the visible overflow/action trigger. Disabled in compact surfaces such as mini-player queue. */
		showMenu?: boolean;
		/** Allow pointer/keyboard context-menu invocation for this row. */
		contextMenu?: boolean;
		onplay: () => void;
		/** Adds an "Add to playlist" menu item. */
		onAdd?: () => void;
		/** Adds a remove menu item (label via `removeLabel`). */
		onRemove?: () => void;
		/** Keyboard-reachable alternatives to dragging a row (queue only). */
		onMoveUp?: () => void;
		onMoveDown?: () => void;
		removeLabel?: string;
	} = $props();

	// In a session as guest, clicking a song adds it to the shared queue instead of playing it —
	// reflect that in the hover icon + label so the row doesn't lie.
	const guestAdd = $derived(lt.role === 'guest');
	const pending = $derived(playback.pendingVideoId === song.video_id);

	// Digits and colons, nothing else. A queue saved before the parser stopped reading a name with a
	// colon in it ("Cast of EPIC: The Musical") as a length still holds those strings, and printing
	// one here squeezes the title and artists down to nothing.
	const duration = $derived(/^[\d:]+$/.test(song.duration ?? '') ? song.duration : undefined);

	const rated = $derived(ratingOf(song));
	// A local file has no YouTube identity, so there is nothing to rate (the same guard the ⋯ menu
	// applies to its like item). The compact variant has no room: it keeps its single heart.
	const showRating = $derived(!compact && !hideRating && !api.isLocalId(song.video_id));

	// Your own playlists holding this song, for the "saved" mark. Gated on `showRating` because it
	// answers the same three questions: a local file is in no YTM playlist, and the compact and
	// queue variants have no width left for another mark.
	const inPlaylists = $derived(showRating ? savedPlaylists(song.video_id) : []);

	let warmTimer: ReturnType<typeof setTimeout> | undefined;
	function warmSoon() {
		clearTimeout(warmTimer);
		warmTimer = setTimeout(() => warmStream(song.video_id, !!song.is_upload), 450);
	}
	function cancelWarm() { clearTimeout(warmTimer); }
</script>


{#snippet rateButton(icon: IconSvgElement, want: 'like' | 'dislike', label: string)}
	<button
		class="cursor-pointer rounded-md p-1.5 text-muted-foreground transition hover:bg-accent/20 hover:text-foreground"
		aria-label={rated === want ? 'Remove rating' : label}
		aria-pressed={rated === want}
		onclick={(e) => {
			e.stopPropagation();
			toggleRating(song, want);
		}}
	>
		
		<HugeiconsIcon
			{icon}
			class="h-4 w-4 {rated === want
				? `fill-current ${want === 'like' ? 'text-primary' : 'text-foreground'}`
				: ''}"
		/>
	</button>
{/snippet}


<div
	role="presentation"
	data-ctx={contextMenu ? 'track' : undefined}
	onpointerenter={warmSoon}
	onpointerleave={cancelWarm}
	class="ryo-row-host ryo-track-row group flex w-full cursor-pointer items-center gap-3 rounded-lg p-2 transition-colors hover:bg-accent/10 {active
		? 'bg-accent/10'
		: ''} {compact ? '' : '[content-visibility:auto] [contain-intrinsic-size:auto_3.5rem]'}"
>
	<div class="flex min-w-0 flex-1 items-center gap-3">
		<div class="flex min-w-0 shrink-0 items-center gap-3">
			{#if index !== undefined}
				<span
					class="relative w-5 shrink-0 text-center text-xs {active
						? 'text-primary'
						: 'text-muted-foreground'}"
				>
					<span class="group-hover:opacity-0">{index + 1}</span>
					<HugeiconsIcon
						icon={pending ? Loading03Icon : guestAdd ? PlayListAddIcon : PlayIcon}
						class="absolute inset-0 m-auto h-3.5 w-3.5 {pending ? 'animate-spin opacity-100' : 'opacity-0 group-hover:opacity-100'}"
					/>
				</span>
			{/if}
			{#if !hideThumb}
				<div class="relative shrink-0">
				{#if song.thumbnail}
					<img src={thumb(song.thumbnail, 96)} alt="" class="h-10 w-10 shrink-0 rounded-md object-cover" loading="lazy" decoding="async" />
				{:else}
					
					<div
						class="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground/50"
					>
						<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4" />
					</div>
				{/if}
				{#if pending && index === undefined}<span class="absolute inset-0 grid place-items-center rounded-md bg-black/45"><HugeiconsIcon icon={Loading03Icon} class="h-4 w-4 animate-spin text-white" /></span>{/if}
				</div>
			{/if}
		</div>
		<div class="min-w-0 flex-1">
			<div class="flex min-w-0 items-center gap-2">
				{#if song.added_by_avatar}
					<img src={thumb(song.added_by_avatar, 48)} alt="" title={`Added by ${song.added_by ?? 'collaborator'}`} class="h-4 w-4 shrink-0 rounded-full object-cover" loading="lazy" decoding="async" />
				{/if}
				<button
					type="button"
					class="ryo-row-play min-w-0 truncate text-left text-sm font-medium {active ? 'text-primary' : ''}"
					aria-label={`${guestAdd ? 'Add to the session queue' : 'Play'}: ${song.title}${song.artists ? `, ${song.artists}` : ''}`}
					aria-current={active ? 'true' : undefined}
					aria-busy={pending}
					onclick={onplay}
				>
					{song.title}
				</button>
				{#if song.queued_by}
					<span
						class="shrink-0 rounded-full bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary"
					>
						{song.queued_by}
					</span>
				{/if}
			</div>
			<div class="flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
				<ArtistLine runs={song.artist_runs} text={song.artists} />
				{#if compact && duration}
					<span class="shrink-0">· {duration}</span>
				{/if}
			</div>
		</div>
	</div>

	{#if compact}
		<div class="flex shrink-0 items-center gap-0.5">
			<button
				class="cursor-pointer rounded-md p-1.5 text-muted-foreground transition hover:bg-accent/20 hover:text-foreground"
				aria-label={isLiked(song) ? 'Remove from liked songs' : 'Save to liked songs'}
				aria-pressed={isLiked(song)}
				onclick={(e) => { e.stopPropagation(); toggleRating(song, 'like'); }}
			>
				<HugeiconsIcon icon={FavouriteIcon} class="h-4 w-4 {isLiked(song) ? 'fill-current text-primary' : ''}" />
			</button>
			{#if showMenu}<TrackMenu {song} {onAdd} {onRemove} {onMoveUp} {onMoveDown} {removeLabel} triggerClass="cursor-pointer rounded-md p-1.5 text-muted-foreground transition hover:bg-accent/20 hover:text-foreground" />{/if}
		</div>
	{:else}
		<!-- One deterministic metadata grid for every row. Optional values leave a small fixed
		     slot instead of changing the row geometry or claiming flex:1, so duration/rating columns
		     stay aligned even when only some tracks have play counts or explicit metadata. -->
		<div class="ryo-track-meta" class:rating-hidden={hideRating}>
			<span class="ryo-track-meta-plays">
				{#if showPlayCount && song.play_count}<span>{song.play_count} plays</span>{/if}
			</span>
			<span class="ryo-track-meta-saved">
				{#if showRating && anySaved() && inPlaylists.length}<SavedInPlaylists playlists={inPlaylists} />{/if}
			</span>
			<span class="ryo-track-meta-explicit">
				{#if !hideRating && song.explicit}<ExplicitIcon class="h-3.5 w-3.5 text-muted-foreground" />{/if}
			</span>
			<span class="ryo-track-meta-rating">
				{#if showRating}
					<span role="group" aria-label="Rating" class="flex items-center gap-0.5 transition-opacity focus-within:opacity-100 group-hover:opacity-100 {rated === 'indifferent' ? 'opacity-0' : ''}">
						{@render rateButton(ThumbsUpIcon, 'like', 'Like')}
						{@render rateButton(ThumbsDownIcon, 'dislike', 'Dislike')}
					</span>
				{/if}
			</span>
			<span class="ryo-track-meta-duration">{duration ?? ''}</span>
			<span class="ryo-track-meta-menu">
				{#if showMenu}
					<TrackMenu
						{song}
						{onAdd}
						{onRemove}
						{onMoveUp}
						{onMoveDown}
						{removeLabel}
						triggerClass="cursor-pointer rounded-md p-1.5 text-muted-foreground transition hover:bg-accent/20 hover:text-foreground"
					/>
				{/if}
			</span>
		</div>
	{/if}
</div>
