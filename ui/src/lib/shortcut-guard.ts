export type ClosestLike = { closest(selector: string): unknown } | null;

const SPACE_ACTIVATED =
	'button, a[href], summary, [role="button"], [role="switch"], [role="checkbox"], [role="radio"], [role="tab"], [role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"], [role="option"], [role="link"], [role="combobox"], [role="slider"]';

export const MODAL_SURFACE = '[role="dialog"], [role="alertdialog"], [aria-modal="true"], [role="menu"]';

export function spaceBelongsToTarget(target: ClosestLike): boolean {
	return !!target?.closest(SPACE_ACTIVATED);
}

export function insideModalSurface(target: ClosestLike): boolean {
	return !!target?.closest(MODAL_SURFACE);
}

export type BareKeyEvent = { ctrlKey: boolean; metaKey: boolean; altKey: boolean };

export function isUnmodified(e: BareKeyEvent): boolean {
	return !e.ctrlKey && !e.metaKey && !e.altKey;
}

export function singleKeyAllowed(
	e: BareKeyEvent,
	opts: { enabled: boolean; modalOpen: boolean }
): boolean {
	return !isUnmodified(e) || (opts.enabled && !opts.modalOpen);
}
