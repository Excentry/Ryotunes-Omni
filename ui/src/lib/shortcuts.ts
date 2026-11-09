// One registry, one dispatcher, one command path. The Settings keybind reference renders this exact
// registry, so a shortcut cannot be documented unless an executable action exists here.
import { browser } from '$app/environment';
import { goto } from '$app/navigation';
import * as api from './api';
import { matchesShortcutChord, type ShortcutChord } from './shortcut-match';
import { MODAL_SURFACE, singleKeyAllowed, spaceBelongsToTarget } from './shortcut-guard';
import {
	cycleRepeat,
	np,
	nudgeVolume,
	playback,
	setPlaybackPosition,
	toggleMute,
	ui
} from './player.svelte';
import { appearance } from './theme.svelte';
import { zoomIn, zoomOut, zoomReset } from './zoom';
import { peelRoute } from './session';

const isMacOS = browser && navigator.platform.startsWith('Mac');
export const MOD = isMacOS ? '⌘' : 'Ctrl+';
const VOLUME_STEP = 5;
const SEEK_STEP = 5;

export type KeybindGroup = 'Search' | 'Playback' | 'Navigation' | 'Interface';
export type Keybind = ShortcutChord & {
	id: string;
	group: KeybindGroup;
	label: string;
	display: string;
};

export const KEYBINDINGS: Keybind[] = [
	{ id: 'search.global', group: 'Search', label: 'Global search', display: `${MOD}K`, codes: ['KeyK'], primary: true },
	{ id: 'search.link', group: 'Search', label: 'Open a music link', display: `${MOD}L`, codes: ['KeyL'], primary: true },
	{ id: 'search.page', group: 'Search', label: 'Search current view', display: '/', codes: ['Slash'] },

	{ id: 'playback.toggle', group: 'Playback', label: 'Play / pause', display: 'Space', codes: ['Space'] },
	{ id: 'playback.previous', group: 'Playback', label: 'Previous track', display: `${MOD}←`, codes: ['ArrowLeft'], primary: true },
	{ id: 'playback.next', group: 'Playback', label: 'Next track', display: `${MOD}→`, codes: ['ArrowRight'], primary: true },
	{ id: 'playback.seekBack', group: 'Playback', label: `Seek back ${SEEK_STEP}s`, display: '←', codes: ['ArrowLeft'] },
	{ id: 'playback.seekForward', group: 'Playback', label: `Seek forward ${SEEK_STEP}s`, display: '→', codes: ['ArrowRight'] },
	{ id: 'playback.mute', group: 'Playback', label: 'Mute / unmute', display: 'M', codes: ['KeyM'] },
	{ id: 'playback.shuffle', group: 'Playback', label: 'Shuffle', display: 'S', codes: ['KeyS'] },
	{ id: 'playback.repeat', group: 'Playback', label: 'Cycle repeat', display: 'R', codes: ['KeyR'] },
	{ id: 'playback.queue', group: 'Playback', label: 'Show / hide queue', display: 'Q', codes: ['KeyQ'] },
	{ id: 'playback.lyrics', group: 'Playback', label: 'Show / hide lyrics', display: 'L', codes: ['KeyL'] },
	{ id: 'playback.now', group: 'Playback', label: 'Show / hide Now Playing', display: `${MOD}E`, codes: ['KeyE'], primary: true },
	{ id: 'playback.volumeUp', group: 'Playback', label: 'Volume up', display: `${MOD}↑`, codes: ['ArrowUp'], primary: true },
	{ id: 'playback.volumeDown', group: 'Playback', label: 'Volume down', display: `${MOD}↓`, codes: ['ArrowDown'], primary: true },

	{ id: 'nav.back', group: 'Navigation', label: 'Back', display: 'Alt+←', codes: ['ArrowLeft'], alt: true },
	{ id: 'nav.forward', group: 'Navigation', label: 'Forward', display: 'Alt+→', codes: ['ArrowRight'], alt: true },
	{ id: 'nav.escape', group: 'Navigation', label: 'Close top layer / go back', display: 'Esc', codes: ['Escape'] },

	{ id: 'interface.settings', group: 'Interface', label: 'Settings', display: `${MOD},`, codes: ['Comma'], primary: true },
	{ id: 'interface.shortcuts', group: 'Interface', label: 'Shortcut reference', display: `${MOD}H`, codes: ['KeyH'], primary: true },
	{ id: 'interface.zoomIn', group: 'Interface', label: 'Zoom in', display: `${MOD}+`, codes: ['Equal', 'NumpadAdd'], primary: true, shift: 'any' },
	{ id: 'interface.zoomOut', group: 'Interface', label: 'Zoom out', display: `${MOD}-`, codes: ['Minus', 'NumpadSubtract'], primary: true },
	{ id: 'interface.zoomReset', group: 'Interface', label: 'Reset interface scale to 110%', display: `${MOD}0`, codes: ['Digit0', 'Numpad0'], primary: true }
];

export const KEYBIND_GROUPS = (['Search', 'Playback', 'Navigation', 'Interface'] as KeybindGroup[]).map((title) => ({
	title,
	rows: KEYBINDINGS.filter((b) => b.group === title)
}));

export function matchesKeybind(e: KeyboardEvent, id: string): boolean {
	const binding = KEYBINDINGS.find((x) => x.id === id);
	return !!binding && matchesShortcutChord(e, binding, isMacOS);
}

function editableTarget(target: EventTarget | null): boolean {
	const el = target instanceof Element ? target : null;
	return !!el?.closest('input, textarea, select, [contenteditable="true"], [role="textbox"]');
}

function targetElement(e: KeyboardEvent): Element | null {
	return e.target instanceof Element ? e.target : null;
}

function singleKeysActive(e: KeyboardEvent): boolean {
	return singleKeyAllowed(e, {
		enabled: appearance.singleKeyShortcuts,
		modalOpen: !!document.querySelector(MODAL_SURFACE)
	});
}

function arrowOwnedByControl(target: EventTarget | null): boolean {
	const el = target instanceof Element ? target : null;
	return !!el?.closest('input[type="range"], [role="slider"], [role="tab"], [role="radio"], [role="listbox"], [role="option"], [role="menu"], [role="menuitem"], select');
}

function consume(e: KeyboardEvent) {
	e.preventDefault();
	e.stopPropagation();
}

function peelOne(): boolean {
	if (ui.paletteOpen) return ((ui.paletteOpen = false), true);
	if (ui.shortcutsOpen) return ((ui.shortcutsOpen = false), true);
	if (ui.share) return ((ui.share = null), true);
	if (ui.linkOpen) return ((ui.linkOpen = false), true);
	if (ui.ltOpen) return ((ui.ltOpen = false), true);
	if (ui.settingsOpen) return ((ui.settingsOpen = false), true);
	if (ui.channelPickerOpen && !ui.channelPickerRequired) return ((ui.channelPickerOpen = false), true);
	if (ui.addSongs) return ((ui.addSongs = null), true);
	if (np.lyricsFocus) return ((np.lyricsFocus = false), true);
	if (ui.lyricsOpen) return ((ui.lyricsOpen = false), true);
	if (ui.queueOpen) return ((ui.queueOpen = false), true);
	if (np.open) return ((np.open = false), true);
	return false;
}

function focusPageSearch() {
	const input = document.querySelector<HTMLInputElement>(
		'.ryo-search-page-form input, .ryo-home-searchbox input, .ryo-track-filter-input, .ryo-shortcut-picker-search input'
	);
	if (!input) return false;
	input.focus();
	input.select();
	return true;
}

function seekBy(delta: number) {
	if (!playback.now) return;
	const duration = playback.duration > 0 ? playback.duration : Number.POSITIVE_INFINITY;
	const position = Math.max(0, Math.min(duration, playback.position + delta));
	setPlaybackPosition(position);
	void api.seek(position);
}

function toggleQueue() {
	if (!playback.now) return;
	if (appearance.tabbedPlayer) {
		if (np.open && np.tab === 'queue') np.open = false;
		else {
			np.open = true;
			np.tab = 'queue';
		}
		return;
	}
	ui.lyricsOpen = false;
	ui.queueOpen = !ui.queueOpen;
}

function toggleLyrics() {
	if (!playback.now) return;
	if (appearance.tabbedPlayer) {
		if (np.open && np.tab === 'lyrics') np.open = false;
		else {
			np.open = true;
			np.tab = 'lyrics';
		}
		return;
	}
	ui.queueOpen = false;
	ui.lyricsOpen = !ui.lyricsOpen;
}

/** Execute a real app action. Returns true only when this dispatcher owns the key event. */
function dispatchShortcut(e: KeyboardEvent, scope: 'main' | 'mini' = 'main'): boolean {
	// The mini WebView owns only transport controls. App-navigation/overlay shortcuts remain scoped
	// to the full Ryotunes window so Ctrl+K, Settings, Q/L and route navigation can never open
	// invisible surfaces behind the widget. Native MPRIS still owns hardware media keys globally.
	if (scope === 'mini') {
		if (editableTarget(e.target)) return false;
		if (!singleKeysActive(e)) return false;
		if (matchesKeybind(e, 'playback.previous')) { if (!playback.now) return false; void api.prevTrack(); return true; }
		if (matchesKeybind(e, 'playback.next')) { if (!playback.now) return false; void api.nextTrack(); return true; }
		if (matchesKeybind(e, 'playback.volumeUp')) { nudgeVolume(VOLUME_STEP); return true; }
		if (matchesKeybind(e, 'playback.volumeDown')) { nudgeVolume(-VOLUME_STEP); return true; }
		if (matchesKeybind(e, 'playback.toggle')) { if (!playback.now || spaceBelongsToTarget(targetElement(e))) return false; void api.togglePause(); return true; }
		if (matchesKeybind(e, 'playback.seekBack')) { if (arrowOwnedByControl(e.target)) return false; seekBy(-SEEK_STEP); return true; }
		if (matchesKeybind(e, 'playback.seekForward')) { if (arrowOwnedByControl(e.target)) return false; seekBy(SEEK_STEP); return true; }
		if (matchesKeybind(e, 'playback.mute')) { toggleMute(); return true; }
		if (matchesKeybind(e, 'playback.shuffle')) { if (!playback.now) return false; void api.toggleShuffle(); return true; }
		if (matchesKeybind(e, 'playback.repeat')) { if (!playback.now) return false; void cycleRepeat(); return true; }
		return false;
	}
	// Browser-style navigation and global command chords remain available from text fields.
	if (matchesKeybind(e, 'nav.back')) { history.back(); return true; }
	if (matchesKeybind(e, 'nav.forward')) { history.forward(); return true; }
	if (matchesKeybind(e, 'search.global')) { ui.paletteOpen = !ui.paletteOpen; return true; }
	if (matchesKeybind(e, 'interface.shortcuts')) { ui.shortcutsOpen = !ui.shortcutsOpen; return true; }
	if (matchesKeybind(e, 'search.link')) { ui.linkOpen = true; return true; }
	if (matchesKeybind(e, 'interface.settings')) { ui.settingsOpen = true; return true; }
	if (matchesKeybind(e, 'interface.zoomIn')) { zoomIn(); return true; }
	if (matchesKeybind(e, 'interface.zoomOut')) { zoomOut(); return true; }
	if (matchesKeybind(e, 'interface.zoomReset')) { zoomReset(); return true; }
	if (matchesKeybind(e, 'playback.now')) {
		if (!playback.now || editableTarget(e.target)) return false;
		np.open = !np.open;
		return true;
	}

	// Escape closes exactly one layer. Local controls can explicitly claim the first Escape (for
	// example: clear a filter, close suggestions, or dismiss an anchored picker). Because this
	// listener owns the capture phase, the ownership check must happen before global overlays peel.
	if (matchesKeybind(e, 'nav.escape')) {
		const targetEl = e.target instanceof Element ? e.target : null;
		if (targetEl?.closest('[data-ryo-escape-owner]')) return false;
		if (peelOne()) return true;
		if (editableTarget(e.target)) return false;
		const target = peelRoute(new URL(window.location.href));
		if (target) { void goto(target); return true; }
		return false;
	}

	if (editableTarget(e.target)) return false;
	if (!singleKeysActive(e)) return false;
	if (matchesKeybind(e, 'playback.previous')) { if (!playback.now) return false; void api.prevTrack(); return true; }
	if (matchesKeybind(e, 'playback.next')) { if (!playback.now) return false; void api.nextTrack(); return true; }
	if (matchesKeybind(e, 'playback.volumeUp')) { nudgeVolume(VOLUME_STEP); return true; }
	if (matchesKeybind(e, 'playback.volumeDown')) { nudgeVolume(-VOLUME_STEP); return true; }
	if (matchesKeybind(e, 'search.page')) return focusPageSearch();
	if (matchesKeybind(e, 'playback.toggle')) {
		if (!playback.now || spaceBelongsToTarget(targetElement(e))) return false;
		void api.togglePause();
		return true;
	}
	if (matchesKeybind(e, 'playback.seekBack')) {
		if (arrowOwnedByControl(e.target)) return false;
		seekBy(-SEEK_STEP);
		return true;
	}
	if (matchesKeybind(e, 'playback.seekForward')) {
		if (arrowOwnedByControl(e.target)) return false;
		seekBy(SEEK_STEP);
		return true;
	}
	if (matchesKeybind(e, 'playback.mute')) { toggleMute(); return true; }
	if (matchesKeybind(e, 'playback.shuffle')) { if (!playback.now) return false; void api.toggleShuffle(); return true; }
	if (matchesKeybind(e, 'playback.repeat')) { if (!playback.now) return false; void cycleRepeat(); return true; }
	if (matchesKeybind(e, 'playback.queue')) { toggleQueue(); return !!playback.now; }
	if (matchesKeybind(e, 'playback.lyrics')) { toggleLyrics(); return !!playback.now; }
	return false;
}

export function initShortcuts(scope: 'main' | 'mini' = 'main') {
	const onKey = (e: KeyboardEvent) => {
		if (dispatchShortcut(e, scope)) consume(e);
	};
	// Capture gives Escape one owner before nested dialogs/command widgets see it.
	window.addEventListener('keydown', onKey, { capture: true });
	return () => window.removeEventListener('keydown', onKey, { capture: true });
}
