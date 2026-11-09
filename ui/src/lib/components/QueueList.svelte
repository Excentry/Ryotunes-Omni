<script module lang="ts">
	let savedQueueQuery = '';
	let savedQueueScroll = 0;
</script>

<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { flip } from 'svelte/animate';
	import { cubicOut } from 'svelte/easing';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { HistoryIcon, InfinityIcon } from '@hugeicons/core-free-icons';
	import TrackRow from '$lib/components/TrackRow.svelte';
	import TrackFilter from '$lib/components/TrackFilter.svelte';
	import { indexCustom, match, normalizeSearchText } from '$lib/localsearch';
	import * as api from '$lib/api';
	import { queueBlocks, moveTarget, type QueueRow } from '$lib/queue';
	import { blockWindows, fullWindow, rowWindow, type RowWindow } from '$lib/rows';
	import { rowScroller } from '$lib/rows.svelte';
	import { dragScroll, QUEUE_ROW_MIME } from '$lib/dnd';
	import { ryokuWheelScroll } from '$lib/ryoku-scroll';
	import { enqueue, playback, openAddToPlaylist, toast, toggleStopAfterCurrent } from '$lib/player.svelte';
	import type { SongItem } from '$lib/api';
	import { lt } from '$lib/lt.svelte';

	let { compact = false, showMenus = true }: { compact?: boolean; showMenus?: boolean } = $props();

	// Guests are add-only in a session — no removing (theirs or anyone's) and no reordering. The
	// playing row can't be removed either (backend guards it too).
	const canRemove = $derived(lt.role !== 'guest');

	// Search is visual-only: it never mutates queue order or playback. Keep the backend index on
	// each hit so clicking/removing still targets the exact queue item (duplicates included).
	let query = $state(savedQueueQuery);
	const normalizedQuery = $derived(normalizeSearchText(query));
	const searchableQueue = $derived(playback.queue.items.map((item, i) => ({ item, i, key: `${item.video_id}:${i}` })));
	const queueSearchIndex = $derived(indexCustom(searchableQueue, (row) => row.item.title ?? '', (row) => `${row.item.artists ?? ''} ${row.item.album ?? ''}`));
	const searchRows = $derived(normalizedQuery ? match(queueSearchIndex, query) : []);
	$effect(() => { savedQueueQuery = query; });

	// --- drag to reorder ---------------------------------------------------------------------
	// Small/normal queues use a pointer-driven reorder instead of WebKit's browser drag image. The
	// row itself follows the pointer, the insertion rule stays in the list, and an rAF exists only
	// while the pointer is actually near an edge. Very large/windowed queues keep HTML DnD so their
	// off-screen virtualization is never disabled just to drag one row.
	let dragFrom = $state<number | null>(null);
	let dropAt = $state<number | null>(null);
	let dragY = $state(0);
	let dragRowHeight = $state(0);
	let dragPointerY = 0;
	let dragStartY = 0;
	let dragStartScroll = 0;
	let dragPointerId: number | null = null;
	let dragNode: HTMLElement | null = null;
	let pressTimer: number | undefined;
	let pressStartX = 0;
	let pressStartY = 0;
	let pressIndex: number | null = null;
	let pressNode: HTMLElement | null = null;
	let autoFrame = 0;
	let dragFrame = 0;
	let autoAt = 0;
	let suppressPlayUntil = 0;
	let dragCommitting = $state(false);

	const canDrag = (i: number) => canRemove && i > playback.queue.currentIndex;
	function rowShift(i: number) {
		if (dragFrom === null || dropAt === null || i === dragFrom || !dragRowHeight) return 0;
		if (dropAt < dragFrom && i >= dropAt && i < dragFrom) return dragRowHeight;
		if (dropAt > dragFrom + 1 && i > dragFrom && i < dropAt) return -dragRowHeight;
		return 0;
	}
	const nonDragTarget = (target: EventTarget | null) =>
		target instanceof Element && !!target.closest('button,a,input,textarea,select,[contenteditable="true"]');

	function clearPress() {
		if (pressTimer !== undefined) window.clearTimeout(pressTimer);
		pressTimer = undefined;
		pressIndex = null;
		pressNode = null;
	}

	function updateDraggedRow() {
		if (dragFrom === null) return;
		dragY = dragPointerY - dragStartY + (el.scrollTop - dragStartScroll);
	}

	function scheduleDragFrame() {
		if (dragFrame) return;
		dragFrame = requestAnimationFrame(() => {
			dragFrame = 0;
			if (dragFrom === null) return;
			updateDraggedRow();
			updateDrop(dragPointerY);
		});
	}

	function updateDrop(pointerY: number) {
		if (dragFrom === null) return;
		const minimum = playback.queue.currentIndex + 1;
		let next = minimum;
		const nodes = Array.from(el.querySelectorAll<HTMLElement>('[data-queue-index]'));
		for (const node of nodes) {
			const i = Number(node.dataset.queueIndex);
			if (!Number.isFinite(i) || i === dragFrom || i < minimum) continue;
			const rect = node.getBoundingClientRect();
			const logicalMid = rect.top - rowShift(i) + rect.height / 2;
			if (pointerY < logicalMid) {
				next = i;
				dropAt = next;
				return;
			}
			next = i + 1;
		}
		dropAt = Math.max(minimum, next);
	}

	function autoScroll(now: number) {
		autoFrame = 0;
		if (dragFrom === null) return;
		const box = el.getBoundingClientRect();
		const edge = Math.min(84, box.height * 0.18);
		let speed = 0;
		if (dragPointerY < box.top + edge) speed = -Math.min(1, (box.top + edge - dragPointerY) / edge);
		else if (dragPointerY > box.bottom - edge) speed = Math.min(1, (dragPointerY - (box.bottom - edge)) / edge);
		if (!speed) {
			autoAt = 0;
			return;
		}
		const dt = autoAt ? Math.min(32, now - autoAt) : 16;
		autoAt = now;
		el.scrollTop += speed * 720 * (dt / 1000);
		updateDraggedRow();
		updateDrop(dragPointerY);
		autoFrame = requestAnimationFrame(autoScroll);
	}

	function ensureAutoScroll() {
		if (!autoFrame) autoFrame = requestAnimationFrame(autoScroll);
	}

	function startPointerDrag(i: number, node: HTMLElement, pointerId: number, y: number) {
		if (windowed || !canDrag(i)) return;
		clearPress();
		dragRowHeight = Math.ceil(node.getBoundingClientRect().height);
		dragFrom = i;
		dropAt = i;
		dragPointerId = pointerId;
		dragNode = node;
		dragStartY = y;
		dragPointerY = y;
		dragStartScroll = el.scrollTop;
		dragY = 0;
		suppressPlayUntil = performance.now() + 500;
		try { node.setPointerCapture(pointerId); } catch {}
	}

	function onPointerDown(e: PointerEvent, i: number) {
		if (windowed || !canDrag(i) || nonDragTarget(e.target) || e.button !== 0) return;
		clearPress();
		pressStartX = e.clientX;
		pressStartY = e.clientY;
		pressIndex = i;
		pressNode = e.currentTarget as HTMLElement;
		if (e.pointerType !== 'mouse') {
			const id = e.pointerId;
			const y = e.clientY;
			pressTimer = window.setTimeout(() => {
				if (pressIndex === i && pressNode) startPointerDrag(i, pressNode, id, y);
			}, 220);
		}
	}

	function onPointerMove(e: PointerEvent, i: number) {
		if (dragFrom !== null && e.pointerId === dragPointerId) {
			dragPointerY = e.clientY;
			scheduleDragFrame();
			ensureAutoScroll();
			e.preventDefault();
			return;
		}
		if (pressIndex !== i || !pressNode) return;
		const distance = Math.hypot(e.clientX - pressStartX, e.clientY - pressStartY);
		if (e.pointerType === 'mouse' && distance > 5) {
			startPointerDrag(i, pressNode, e.pointerId, e.clientY);
		} else if (e.pointerType !== 'mouse' && distance > 9) {
			// A touch gesture that moves before the hold threshold is ordinary queue scrolling.
			clearPress();
		}
	}

	async function finishPointerDrag(e?: PointerEvent) {
		clearPress();
		if (dragFrom === null || dragCommitting) return;
		const from = dragFrom;
		const to = dropAt === null ? null : moveTarget(from, dropAt);
		suppressPlayUntil = performance.now() + 550;
		if (autoFrame) cancelAnimationFrame(autoFrame);
		if (dragFrame) cancelAnimationFrame(dragFrame);
		autoFrame = 0;
		dragFrame = 0;
		autoAt = 0;
		if (e && dragNode && dragPointerId !== null) {
			try { dragNode.releasePointerCapture(dragPointerId); } catch {}
		}
		dragPointerId = null;
		dragCommitting = true;
		try {
			if (to !== null) await api.moveInQueue(from, to);
			// The backend emits the authoritative queue before the command resolves. Let Svelte paint
			// that order once while the lifted row is still present, then settle cleanly into place.
			await tick();
			await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
		} finally {
			dragFrom = null;
			dropAt = null;
			dragY = 0;
			dragRowHeight = 0;
			dragNode = null;
			dragCommitting = false;
		}
	}

	function moveBy(i: number, delta: -1 | 1) {
		void api.moveInQueue(i, i + delta);
	}
	const canMoveUp = (i: number) => canDrag(i) && i - 1 > playback.queue.currentIndex;
	const canMoveDown = (i: number) => canDrag(i) && i < lastIndex;

	function playIndex(i: number) {
		if (performance.now() < suppressPlayUntil) return;
		void api.playIndex(i);
	}

	// Windowed queues keep HTML5 DnD so the dragged origin can be virtualized safely. Suppress the
	// browser's huge translucent row snapshot: the insertion rule is the feedback we need there.
	const dragPixel = new Image();
	dragPixel.src = 'data:image/gif;base64,R0lGODlhAQABAAD/ACwAAAAAAQABAAACADs=';
	function onDragStart(e: DragEvent, i: number) {
		if (!windowed || !e.dataTransfer) return;
		e.dataTransfer.setData(QUEUE_ROW_MIME, String(i));
		e.dataTransfer.effectAllowed = 'move';
		e.dataTransfer.setDragImage(dragPixel, 0, 0);
		dragFrom = i;
	}

	function onDragOver(e: DragEvent, i: number) {
		if (!windowed || dragFrom === null || !e.dataTransfer?.types.includes(QUEUE_ROW_MIME)) return;
		e.preventDefault();
		e.dataTransfer.dropEffect = 'move';
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
		dropAt = e.clientY < r.top + r.height / 2 ? i : i + 1;
	}

	function onDrop() {
		const to = dragFrom !== null && dropAt !== null ? moveTarget(dragFrom, dropAt) : null;
		if (to !== null) void api.moveInQueue(dragFrom!, to);
		dragFrom = null;
		dropAt = null;
		dragRowHeight = 0;
	}

	// Blocks in play order, cut wherever the upcoming tracks change origin (`queue.ts`).
	const view = $derived(queueBlocks(playback.queue));
	// The tail of the queue, for the one drop position no row can mark from its own top edge.
	const lastIndex = $derived(view.blocks.at(-1)?.rows.at(-1)?.i ?? -1);

	// The tracks already heard, hidden until asked for: a queue played deep into has hundreds of
	// them, and they sit above everything anyone opened the panel to look at. The untouched prefix
	// above them (`view.earlier`) is not hidden: it is bounded by the playlist and shrinks every
	// time you press previous, where history only grows.
	let showPrev = $state(false);
	let el: HTMLElement;
	let nowEl: HTMLElement | undefined = $state();

	// Open on the playing track. Everything in front of it is drawn above, so a queue opened three
	// thousand tracks into Liked Songs would otherwise open on track 1.
	//
	// Measured off the heading rather than computed from row heights, because the run above reserves
	// `rows × rowPx` and `rowPx` starts at the assumed 56 before settling to the panel's real 72 a
	// frame later (`rows.svelte.ts`), which moves the heading down by a quarter of the run. So land,
	// then land again once it has settled.
	onMount(() => {
		const restore = savedQueueScroll > 0;
		const land = () => {
			if (restore) {
				el.scrollTop = Math.min(savedQueueScroll, Math.max(0, el.scrollHeight - el.clientHeight));
				return;
			}
			if (!nowEl) return;
			el.scrollTop += nowEl.getBoundingClientRect().top - el.getBoundingClientRect().top;
		};
		land();
		let frame = requestAnimationFrame(() => (frame = requestAnimationFrame(land)));
		return () => {
			cancelAnimationFrame(frame);
			savedQueueScroll = el?.scrollTop ?? savedQueueScroll;
		};
	});

	// Playing a playlist queues the whole playlist, so this panel can be handed five figures of
	// rows the moment it opens, at roughly 165 KB of web-process memory each (`rows.ts`). Past a
	// couple of hundred it renders only what is near the viewport.
	//
	// Below that it is exactly what it always was, flip animation included: windowing costs the
	// reorder animation (flip measures against the viewport, so it would fight the scroll), and
	// that is a bad trade for a queue you can see the end of.
	const WINDOW_ABOVE = 200;
	const sc = rowScroller();
	// One entry per block, in render order. A collapsed history is 0 rows but still charged a
	// heading it doesn't draw, which shifts every window's *choice* of slice by 40px and none of
	// their heights: the overscan swallows it (see HEADING_PX).
	const counts = $derived([
		compact ? 0 : view.earlier.length,
		compact ? 0 : showPrev ? view.prev.length : 0,
		view.now ? 1 : 0,
		...view.blocks.map((b) => b.rows.length)
	]);
	const windowed = $derived(counts.reduce((a, c) => a + c, 0) > WINDOW_ABOVE);
	const wins = $derived(
		windowed
			? blockWindows(sc.scrollTop, sc.viewportPx, counts, sc.rowPx)
			: counts.map(fullWindow)
	);

	const searchWindow = $derived(
		searchRows.length > WINDOW_ABOVE
			? rowWindow(sc.scrollTop, sc.viewportPx, searchRows.length, sc.rowPx)
			: fullWindow(searchRows.length)
	);

	function clearQueue() {
		const { items, currentIndex } = playback.queue;
		const upcoming = items.slice(currentIndex + 1);
		const playNext: SongItem[] = upcoming.filter((s) => s.queued);
		const addToQueue: SongItem[] = upcoming.filter((s) => !s.queued && s.queued_end);
		const count = playNext.length + addToQueue.length;
		if (!count) return;
		void api.clearQueued();
		toast.info(`Removed ${count} ${count === 1 ? 'song' : 'songs'} from the queue`, {
			label: 'Undo',
			run: () => {
				void (async () => {
					if (playNext.length) await enqueue(playNext, true);
					if (addToQueue.length) await enqueue(addToQueue, false);
				})();
			}
		});
	}

	async function togglePrev() {
		const before = el.scrollHeight;
		showPrev = !showPrev;
		await tick();
		// Rows appear (or vanish) above the viewport, and WebKit implements no scroll anchoring, so
		// without this the panel jumps by the whole height of the history. Keeps Now playing still.
		el.scrollTop += el.scrollHeight - before;
	}
</script>

{#snippet rows(list: QueueRow[], w: RowWindow)}
	
	<div role="list" style="padding-top:{w.padTop}px;padding-bottom:{w.padBottom}px">
		{#each list.slice(w.start, w.end) as { item, key, i } (key)}
			
			<div
				data-row
				data-queue-index={i}
				role="listitem"
				class="relative ryo-queue-reorder-row"
				class:ryo-queue-draggable={canDrag(i)}
				class:ryo-queue-dragging={!windowed && dragFrom === i}
				style:transform={!windowed && dragFrom === i ? `translate3d(0, ${dragY}px, 0)` : (!windowed && rowShift(i) ? `translate3d(0, ${rowShift(i)}px, 0)` : undefined)}
				style:z-index={!windowed && dragFrom === i ? '35' : undefined}
				animate:flip={{ duration: windowed || dragFrom !== null ? 0 : 170, easing: cubicOut }}
				draggable={windowed && canDrag(i)}
				onpointerdown={(e) => onPointerDown(e, i)}
				onpointermove={(e) => onPointerMove(e, i)}
				onpointerup={(e) => void finishPointerDrag(e)}
				onpointercancel={(e) => void finishPointerDrag(e)}
				onpointerleave={() => { if (dragFrom === null) clearPress(); }}
				ondragstart={(e) => onDragStart(e, i)}
				ondragover={(e) => onDragOver(e, i)}
				ondrop={onDrop}
			>
				
				{#if dropAt === i}
					<div
						class="ryo-queue-insert-rule pointer-events-none absolute inset-x-3 top-0 z-10 h-0.5 rounded-full bg-primary"
					></div>
				{:else if dropAt === i + 1 && i === lastIndex}
					<div
						class="ryo-queue-insert-rule pointer-events-none absolute inset-x-3 bottom-0 z-10 h-0.5 rounded-full bg-primary"
					></div>
				{/if}
				<TrackRow
					song={item}
					index={i}
					active={i === playback.queue.currentIndex}
					hideRating
					{compact}
					showMenu={showMenus}
					contextMenu={showMenus}
					onplay={() => playIndex(i)}
					onAdd={() => openAddToPlaylist(item)}
					onRemove={canRemove && i !== playback.queue.currentIndex
						? () => api.removeFromQueue(i)
						: undefined}
					onMoveUp={canMoveUp(i) ? () => moveBy(i, -1) : undefined}
					onMoveDown={canMoveDown(i) ? () => moveBy(i, 1) : undefined}
					removeLabel="Remove from queue"
				/>
			</div>
		{/each}
	</div>
{/snippet}


<svelte:window
	ondragend={() => {
		dragFrom = null;
		dropAt = null;
		dragRowHeight = 0;
	}}
/>


<div
	class="min-h-0 flex-1 overflow-y-auto p-2"
	class:ryo-queue-compact={compact}
	style="background:var(--ryo-paper);color:var(--ryo-ink);"
	bind:this={el}
	{@attach sc.attach}
	{@attach ryokuWheelScroll}
	{@attach (node) => dragScroll(node, QUEUE_ROW_MIME)}
>
	{#if !compact}
		<div class="sticky top-0 z-20 -mx-1 mb-2 space-y-2 bg-background/95 px-1 pb-2 pt-1 backdrop-blur-sm">
			<div class="ryo-queue-search"><TrackFilter bind:value={query} placeholder="Search queue" /></div>
			<div class="flex items-center justify-between gap-2 px-1">
				<span class="text-[11px] text-muted-foreground">
					{normalizedQuery ? `${searchRows.length} ${searchRows.length === 1 ? 'match' : 'matches'}` : 'Up next'}
				</span>
				{#if lt.role !== 'guest' && playback.now}
					<button
						class="cursor-pointer rounded-lg px-2 py-1 text-[11px] font-medium transition-colors hover:bg-muted hover:text-foreground {playback.stopAfterCurrent ? 'bg-muted text-foreground' : 'text-muted-foreground'}"
						title="Finish this song, then pause before the next one"
						onclick={toggleStopAfterCurrent}
					>
						{playback.stopAfterCurrent ? 'Stopping after this' : 'Stop after current'}
					</button>
				{/if}
			</div>
		</div>
	{/if}

	{#if normalizedQuery}
		{#if searchRows.length}
			<div role="list" style="padding-top:{searchWindow.padTop}px;padding-bottom:{searchWindow.padBottom}px">
				{#each searchRows.slice(searchWindow.start, searchWindow.end) as { item, key, i } (key)}
					<div role="listitem" data-row>
						<TrackRow
							song={item}
							index={i}
							active={i === playback.queue.currentIndex}
							hideRating
							{compact}
							showMenu={showMenus}
							contextMenu={showMenus}
							onplay={() => playIndex(i)}
							onAdd={() => openAddToPlaylist(item)}
							onRemove={canRemove && i !== playback.queue.currentIndex ? () => api.removeFromQueue(i) : undefined}
							removeLabel="Remove from queue"
						/>
					</div>
				{/each}
			</div>
		{:else}
			<p class="p-4 text-center text-sm text-muted-foreground">No queued songs match “{query.trim()}”.</p>
		{/if}
	{:else if view.now}
		
		{#if !compact && view.earlier.length}
			<h3 class="truncate px-2 pt-2 pb-1.5 text-sm font-semibold text-muted-foreground">
				{view.earlierHeading}
			</h3>
			{@render rows(view.earlier, wins[0])}
		{/if}
		{#if !compact && showPrev && view.prev.length}
			<h3 class="px-2 pt-2 pb-1.5 text-sm font-semibold text-muted-foreground">
				Previously played
			</h3>
			{@render rows(view.prev, wins[1])}
		{/if}
		<div bind:this={nowEl} class="flex items-center justify-between gap-2 px-2 pt-2 pb-1.5">
			<h3 class="truncate text-sm font-semibold">Now playing</h3>
			{#if !compact && view.prev.length}
				<button
					class="flex shrink-0 cursor-pointer items-center gap-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
					onclick={togglePrev}
				>
					<HugeiconsIcon icon={HistoryIcon} class="h-3.5 w-3.5" />
					{showPrev ? 'Hide previous' : 'Load previous'}
				</button>
			{/if}
		</div>
		{@render rows([view.now], wins[2])}

		{#each view.blocks as block, b (block.key)}
			{#if block.autoplay}
				<div
					class="mt-3 flex items-center gap-2 border-t px-2 pt-2.5 pb-1.5 text-muted-foreground"
					title="Autoplay keeps the music going with similar songs. Turn it off in Settings ▸ Playback."
				>
					<HugeiconsIcon icon={InfinityIcon} class="h-3.5 w-3.5" />
					<span class="text-xs font-medium">Autoplay</span>
					<span class="truncate text-xs">· similar music</span>
				</div>
			{:else}
				<div class="mt-3 flex items-center justify-between gap-2 px-2 pb-1.5">
					<h3 class="truncate text-sm font-semibold">{block.heading}</h3>
					{#if block.clearable && canRemove}
						<button
							class="shrink-0 cursor-pointer text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
							onclick={() => clearQueue()}
						>
							Clear queue
						</button>
					{/if}
				</div>
			{/if}
			{@render rows(block.rows, wins[b + 3])}
		{/each}
	{:else}
		<p class="p-4 text-sm text-muted-foreground">The queue is empty.</p>
	{/if}
</div>


<style>
	.ryo-queue-compact { padding:3px; scrollbar-width:thin; background:transparent !important; }
	.ryo-queue-compact h3 { font-size:10px; padding-top:4px; padding-bottom:3px; color:var(--ryo-ink-muted); }
</style>
