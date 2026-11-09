<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		PlayIcon,
		MusicNote01Icon,
		UserIcon,
		ListRestartIcon
	} from '@hugeicons/core-free-icons';
	import { ON_REPEAT_ID, isSmartPlaylistId } from '$lib/api';
	import type { BrowseItem } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import { setDragItem } from '$lib/dnd';
	import { asSong, openItem, playItem } from '$lib/browse';
	import { openAddToPlaylist } from '$lib/player.svelte';
	import { warmStream } from '$lib/warm-stream';
	import TrackMenu from './TrackMenu.svelte';
	import PlaylistMenu from './PlaylistMenu.svelte';
	import ExplicitIcon from './ExplicitIcon.svelte';
	import SmartPlaylistArt from './SmartPlaylistArt.svelte';

	let { item, compact = false }: { item: BrowseItem; compact?: boolean } = $props();

	const round = $derived(item.kind === 'artist');
	// On Repeat has no artwork by nature, so its cover is the icon rather than the neutral
	// placeholder every failed thumbnail lands on.
	const onRepeat = $derived(item.id === ON_REPEAT_ID);
	const smart = $derived(isSmartPlaylistId(item.id));

	// Google's CDN doesn't serve every rewritten size — asking for one it doesn't have 404s, and the
	// browser then paints its broken-image glyph. So: try the sized URL, retry the original once, and
	// only then fall back to a neutral icon tile.
	let attempt = $state(0);
	let imageLoaded = $state(false);
	$effect(() => {
		item.thumbnail; // re-arm when the card is reused for a different item
		attempt = 0;
		imageLoaded = false;
	});
	const sized = $derived(thumb(item.thumbnail, 256));
	// Account-library renderers sometimes carry signed/variant image URLs where synthesising another
	// CDN size returns 404. The exact URL is the one already proven in the sidebar, so use it first;
	// only try a resized derivative as a recovery path. Reliability matters more than saving a few KB.
	const src = $derived(attempt === 0 ? item.thumbnail : sized);
	const imgFailed = () => {
		if (attempt === 0 && sized && sized !== item.thumbnail) attempt = 1;
		else attempt = 2;
	};

	let playing = $state(false); // in-flight guard for the fetch-then-play path
	let warmTimer: ReturnType<typeof setTimeout> | undefined;
	function warmSoon() {
		if (item.kind !== 'song') return;
		clearTimeout(warmTimer);
		warmTimer = setTimeout(() => warmStream(item.id, !!item.isUpload), 450);
	}
	function cancelWarm() { clearTimeout(warmTimer); }

	async function playNow() {
		if (playing) return;
		playing = true;
		try {
			await playItem(item);
		} finally {
			playing = false;
		}
	}
</script>


<div class="ryo-row-host group relative flex w-full flex-col gap-2" data-ctx>
	
	<div
		class="flex flex-col text-left transition-colors hover:bg-accent/10 {compact
			? 'gap-1.5 rounded-lg p-1.5'
			: 'gap-2 rounded-md p-1.5'}"
		role="presentation"
		draggable="true"
		onpointerenter={warmSoon}
		onpointerleave={cancelWarm}
		ondragstart={(e) => setDragItem(e, item)}
		title={item.subtitle ? `${item.title} — ${item.subtitle}` : item.title}
	>
		
		<div class="relative">
			<div
				class="pointer-events-none absolute inset-0 opacity-0 transition-opacity duration-200 group-hover:opacity-100 {round
					? 'rounded-full'
					: 'rounded-md'}"
			></div>
			
			<div
				class="ryo-art-slot relative aspect-square w-full overflow-hidden bg-muted {round ? 'is-artist ' : ''}{round
					? 'rounded-full'
					: 'rounded-md'}"
			>
				{#if smart}
					<SmartPlaylistArt id={item.id} />
				{:else if item.thumbnail && attempt < 2 && !onRepeat}
					<img
						{src}
						alt=""
						class="h-full w-full object-cover transition-transform duration-[210ms] ease-out group-hover:scale-105"
						loading="lazy"
						decoding="async"
						draggable="false"
						onerror={imgFailed}
						onload={() => (imageLoaded = true)}
						data-loaded={imageLoaded ? 'true' : 'false'}
					/>
				{:else}
					<div
						class="flex h-full w-full items-center justify-center {onRepeat
							? 'bg-primary/10 text-primary'
							: 'text-muted-foreground/50'}"
					>
						
						<HugeiconsIcon
							icon={round ? UserIcon : MusicNote01Icon}
							altIcon={ListRestartIcon}
							showAlt={onRepeat}
							class={onRepeat
								? compact
									? 'h-7 w-7'
									: 'h-10 w-10'
								: compact
									? 'h-5 w-5'
									: 'h-7 w-7'}
						/>
					</div>
				{/if}
				{#if item.kind !== 'artist'}
					
					<button
						class="absolute z-10 flex translate-y-1 cursor-pointer items-center justify-center rounded-md border border-border bg-background text-foreground opacity-0 transition-[opacity,transform] duration-[170ms] ease-out group-hover:translate-y-0 group-hover:opacity-100 focus-visible:opacity-100 {compact
							? 'bottom-1.5 right-1.5 h-7 w-7'
							: 'bottom-2 right-2 h-9 w-9'}"
						class:animate-pulse={playing}
						disabled={playing}
						aria-label="Play {item.title}"
						onclick={(e) => {
							e.stopPropagation();
							playNow();
						}}
					>
						<HugeiconsIcon icon={PlayIcon} class={compact ? 'h-3 w-3' : 'h-4 w-4'} />
					</button>
				{/if}
			</div>
		</div>
		<div class="min-w-0 {round ? 'text-center' : ''}">
			<button
				type="button"
				class="ryo-row-play block w-full truncate font-medium {round ? 'text-center' : 'text-left'} {compact ? 'text-xs' : 'text-sm'}"
				onclick={() => openItem(item)}
			>
				{item.title}
			</button>
			{#if item.subtitle || item.explicit}
				<div
					class="flex items-center gap-1 text-muted-foreground {round
						? 'justify-center'
						: ''} {compact ? 'text-[0.6875rem]' : 'text-xs'}"
				>
					{#if item.explicit}
						<ExplicitIcon class="h-3 w-3 shrink-0" />
					{/if}
					<span class="truncate">{item.subtitle}</span>
				</div>
			{/if}
		</div>
	</div>
	
	{#if item.kind === 'song'}
		<TrackMenu
			song={asSong(item)}
			onAdd={() => openAddToPlaylist(asSong(item))}
			triggerClass="absolute right-3 top-3 flex h-8 w-8 items-center justify-center rounded-md border border-border bg-background/95 text-foreground transition hover:bg-background cursor-pointer"
		/>
	{:else}
		<PlaylistMenu
			{item}
			showPin={item.kind === 'playlist'}
			triggerClass="absolute right-3 top-3 flex h-8 w-8 items-center justify-center rounded-md border border-border bg-background/95 text-foreground transition hover:bg-background cursor-pointer"
		/>
	{/if}
</div>
