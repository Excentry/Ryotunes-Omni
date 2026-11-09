/* Ryotunes appearance and behaviour preferences.
 *
 * Light/dark ownership is explicit here so the app can follow the desktop without allowing
 * component-local hard-coded colours to fight the user's choice. Ryoku still supplies accent and
 * motion tokens through ryoku-live.ts; the luminance palette remains a comfortable Ryotunes one.
 */

const APPEARANCE_KEY = 'ryotunes:appearance';
const LEGACY_APPEARANCE_KEY = 'appearance';

export type ThemeMode = 'system' | 'light' | 'dark';

export const appearance = $state({
	/** Follow the desktop by default; Light/Dark are deliberate user overrides. */
	themeMode: 'system' as ThemeMode,
	/** The now-playing view owns Queue/Lyrics as tabs. Off, the player-bar buttons open panels. */
	tabbedPlayer: true,
	/** Starting a track brings the full now-playing instrument forward. */
	openPlayerOnPlay: true,
	/** Resource-first mode: disables speculative work and decorative motion while preserving audio. */
	lowResourceMode: false,
	/** Unmodified keys (Space, arrows, M, S, R, Q, L, /) act as playback shortcuts when focus is outside text fields. */
	singleKeyShortcuts: true
});

let mediaQuery: MediaQueryList | null = null;
let mediaListener: ((event: MediaQueryListEvent) => void) | null = null;
// In Follow system mode, Ryoku's resolved Material surface is more authoritative than the generic
// portal/media-query hint. Native theme events keep this updated without polling.
let ryokuSystemLight: boolean | null = null;

export function resolvedTheme(): 'light' | 'dark' {
	if (appearance.themeMode === 'light' || appearance.themeMode === 'dark') return appearance.themeMode;
	if (ryokuSystemLight !== null) return ryokuSystemLight ? 'light' : 'dark';
	return typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches
		? 'dark'
		: 'light';
}

export function setRyokuSystemTheme(light: boolean | null): void {
	ryokuSystemLight = light;
	if (appearance.themeMode === 'system') applyTheme();
}

function applyTheme(): void {
	if (typeof document === 'undefined') return;
	const root = document.documentElement;
	const resolved = resolvedTheme();
	root.dataset.ryoTheme = resolved;
	root.dataset.themePreference = appearance.themeMode;
	root.dataset.lowResource = appearance.lowResourceMode ? 'true' : 'false';
	root.classList.toggle('dark', resolved === 'dark');
	root.style.colorScheme = resolved;
}

function watchSystemTheme(): void {
	if (typeof window === 'undefined') return;
	mediaQuery ??= window.matchMedia('(prefers-color-scheme: dark)');
	if (mediaListener) mediaQuery.removeEventListener('change', mediaListener);
	mediaListener = () => {
		if (appearance.themeMode === 'system') applyTheme();
	};
	mediaQuery.addEventListener('change', mediaListener);
}

export function setAppearance(patch: Partial<typeof appearance>): void {
	Object.assign(appearance, patch);
	applyTheme();
	try {
		localStorage.setItem(APPEARANCE_KEY, JSON.stringify(appearance));
	} catch {
		// A locked/quota-limited store must never make playback or settings fail.
	}
	if (typeof window !== 'undefined') window.dispatchEvent(new Event('ryotunes-appearance-changed'));
}

/** Restore safe, shape-checked preferences and attach the system-theme watcher once. */
export function initAppearance(): void {
	try {
		let raw = localStorage.getItem(APPEARANCE_KEY);
		if (!raw) {
			raw = localStorage.getItem(LEGACY_APPEARANCE_KEY);
			if (raw) {
				localStorage.setItem(APPEARANCE_KEY, raw);
				localStorage.removeItem(LEGACY_APPEARANCE_KEY);
			}
		}
		const saved = JSON.parse(raw ?? '{}');
		if (saved?.themeMode === 'system' || saved?.themeMode === 'light' || saved?.themeMode === 'dark') {
			appearance.themeMode = saved.themeMode;
		}
		for (const key of ['tabbedPlayer', 'openPlayerOnPlay', 'lowResourceMode', 'singleKeyShortcuts'] as const) {
			if (typeof saved?.[key] === 'boolean') appearance[key] = saved[key];
		}
		localStorage.removeItem('primary-theme');
		localStorage.removeItem('custom-theme');
	} catch {
		// Corrupt or inaccessible storage: defaults are already the safe state.
	}

	const root = document.documentElement;
	for (const cls of [...root.classList]) if (cls.startsWith('theme-')) root.classList.remove(cls);
	for (const variable of [
		'--primary', '--primary-foreground', '--accent', '--accent-foreground', '--hue', '--radius',
		'--font-sans', '--font-heading'
	]) root.style.removeProperty(variable);
	watchSystemTheme();
	applyTheme();
}
