<script lang="ts">
	// The home grid the user curates (was "Quick Picks" — renamed: YouTube Music has a shelf by that
	// name and it isn't this one). It holds what was put in it, in the order it was dragged into, plus
	// On Repeat once that has enough songs (the only tile the app suggests, and removing it is
	// permanent). Unlike before it renders even when empty — a section that hides itself is a section
	// nobody discovers. Logic in $lib/personal.ts.
	//
	// Not square cards: these were 5.5rem tiles and every label came out as "アプソリュ…" over a
	// Repeated creator metadata adds noise here. A shortcut is a thing you already know, so
	// what it owes you is its *name* at a size you can read, not another piece of cover art competing
	// with the shelves below. Hence wide tiles with the art flush to the leading edge — four to a row
	// instead of seven, and the title gets four times the width.
	//
	// Note: drag is the only way to reorder (no keyboard equivalent). Add/remove/open all work
	// from the keyboard; wire arrow-key moves onto the tiles if anyone actually needs it.
	import { flip } from 'svelte/animate';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Cancel01Icon,
		Add01Icon,
		DashboardSquare02Icon,
		Edit01Icon,
		PlayIcon,
		MusicNote01Icon,
		UserIcon
	} from '@hugeicons/core-free-icons';
	import SectionHeading from './SectionHeading.svelte';
	import ShortcutPicker from './ShortcutPicker.svelte';
	import SmartPlaylistArt from './SmartPlaylistArt.svelte';
	import ItemMenu from './ItemMenu.svelte';
	import { isSmartPlaylistId } from '$lib/api';
	import type { BrowseItem } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import { openItem, playItem } from '$lib/browse';
	import { library, personal, placePick, removePick } from '$lib/player.svelte';
	import { freshen, MAX_PICKS } from '$lib/personal';
	import { getDragItem, isDragItem, setDragItem } from '$lib/dnd';

	// The page owns the Edit-home modal; this section only lends it a place to be opened from. Its
	// header is the first thing on home and the one row that's always there, so the button that
	// rearranges the rest of the page lives here rather than following a section that can be hidden.
	let { onEdit }: { onEdit?: () => void } = $props();

	// Tiles are stored as a snapshot of the card, so a playlist that has gained tracks since it was
	// pinned would keep showing the old count; `freshen` overlays the live library row (#67).
	const picks = $derived(personal.picks.map((p) => freshen(p, library.items)));
	let picking = $state(false);
	let addButton = $state<HTMLButtonElement | null>(null);
	// Where a drop would land: the id of the tile it goes in front of, `null` for the end of the grid,
	// `undefined` when no drag of ours is over the section at all.
	let before = $state<string | null | undefined>(undefined);
	let busy = $state<string | null>(null); // id of the tile whose fetch-then-play is in flight
	// Google's CDN 404s some rewritten sizes; a dead thumb degrades to a placeholder icon rather than
	// the browser's broken-image glyph.
	let failed = $state<Record<string, boolean>>({});

	// One handler for the whole section: the tile under the cursor carries its id in `data-pick`, so
	// hovering anywhere else (the header, the empty dropzone, past the last row) means "append".
	// The gaps *between* tiles are the exception — they belong to the grid, and reading them as
	// "append" made the marker teleport to the end of the row every time the cursor crossed one, so
	// there they hold whatever the last tile decided.
	function targetId(e: DragEvent): string | null {
		const el = e.target as HTMLElement | null;
		const tile = el?.closest('[data-pick]');
		if (tile) return tile.getAttribute('data-pick');
		return el?.closest('[data-grid]') ? (before ?? null) : null;
	}

	function over(e: DragEvent) {
		if (!isDragItem(e)) return; // a file or a link — leave it to the page
		e.preventDefault(); // required, or the drop is refused
		if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
		before = targetId(e);
	}

	function drop(e: DragEvent) {
		const beforeId = before ?? targetId(e);
		before = undefined;
		const item = getDragItem(e);
		if (!item) return;
		e.preventDefault();
		placePick(item, beforeId);
	}

	async function play(item: BrowseItem) {
		if (busy) return;
		busy = item.id;
		try {
			await playItem(item);
		} finally {
			busy = null;
		}
	}
</script>


<svelte:window ondragend={() => (before = undefined)} />

<section class="ryo-pinboard-section">
	<SectionHeading title="Shortcuts" icon={DashboardSquare02Icon}>
		{#snippet lead()}
			{#if onEdit}
				<button onclick={onEdit} title="Edit home" class="ryo-pinboard-head-action">
					<HugeiconsIcon icon={Edit01Icon} class="h-3.5 w-3.5" />
					Edit home
				</button>
			{/if}
		{/snippet}
	</SectionHeading>

	<div
		class="ryo-pinboard"
		role="group"
		aria-label="Shortcuts"
		ondragover={over}
		ondrop={drop}
		ondragleave={(e) => {
			const r = e.currentTarget.getBoundingClientRect();
			if (e.clientX < r.left || e.clientX >= r.right || e.clientY < r.top || e.clientY >= r.bottom)
				before = undefined;
		}}
	>
		<div data-grid class="ryo-pinboard-grid">
			{#each picks as item (item.id)}
				{@const round = item.kind === 'artist'}
				{@const smart = isSmartPlaylistId(item.id)}
				<div class="group/pick ryo-pin ryo-row-host" data-ctx data-pick={item.id} animate:flip={{ duration: 170 }}>
					{#if before === item.id}<span class="ryo-pin-drop"></span>{/if}
					<div
						class="ryo-pin-main"
						role="presentation"
						draggable="true"
						ondragstart={(e) => setDragItem(e, item)}
						title={item.subtitle ? `${item.title} — ${item.subtitle}` : item.title}
					>
						<span class="ryo-pin-art {round ? 'round' : ''}">
							{#if smart}
								<SmartPlaylistArt id={item.id} />
							{:else if item.thumbnail && !failed[item.id]}
								<img src={thumb(item.thumbnail, 128)} alt="" loading="lazy" decoding="async" draggable="false" onerror={() => (failed = { ...failed, [item.id]: true })} />
							{:else}
								<span class="ryo-pin-placeholder"><HugeiconsIcon icon={round ? UserIcon : MusicNote01Icon} class="h-4 w-4" /></span>
							{/if}
							{#if !round}
								<button
									class="ryo-pin-play"
									class:busy={busy === item.id}
									disabled={busy === item.id}
									aria-label="Play {item.title}"
									onclick={(e) => { e.stopPropagation(); play(item); }}
								>
									<HugeiconsIcon icon={PlayIcon} class="h-3.5 w-3.5" />
								</button>
							{/if}
						</span>
						<span class="ryo-pin-copy">
							<strong><button type="button" class="ryo-row-play block max-w-full truncate text-left" onclick={() => openItem(item)}>{item.title}</button></strong>
							<small>{item.subtitle ?? item.kind}</small>
						</span>
					</div>
					<ItemMenu
						{item}
						triggerClass="ryo-pin-menu"
					/>
					<button onclick={() => removePick(item.id)} title="Remove from shortcuts" aria-label="Remove {item.title} from shortcuts" class="ryo-pin-remove">
						<HugeiconsIcon icon={Cancel01Icon} class="h-3 w-3" />
					</button>
				</div>
			{/each}

			{#if picks.length < MAX_PICKS}
				<button
					bind:this={addButton}
					type="button"
					class="ryo-pin-add {before === null ? 'drop-target' : ''}"
					onclick={() => (picking = true)}
				>
					<span><HugeiconsIcon icon={Add01Icon} class="h-4 w-4" /></span>
					<b>{picks.length ? 'Add shortcut' : 'Pin something here'}</b>
					<small>{picks.length ? 'Library or any card' : 'One click from Home'}</small>
				</button>
			{/if}
		</div>
	</div>
</section>

{#if picking}
	<ShortcutPicker anchor={addButton} onclose={() => (picking = false)} />
{/if}
