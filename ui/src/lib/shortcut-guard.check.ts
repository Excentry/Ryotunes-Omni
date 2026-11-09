import { insideModalSurface, singleKeyAllowed, spaceBelongsToTarget } from './shortcut-guard.ts';

function ok(condition: unknown, message: string): asserts condition {
	if (!condition) throw new Error(`shortcut guard regression: ${message}`);
}

const within = (...selectors: string[]) => ({
	closest: (selector: string) => (selectors.some((s) => selector.includes(s)) ? {} : null)
});
const bare = { ctrlKey: false, metaKey: false, altKey: false };

ok(spaceBelongsToTarget(within('button')), 'Space on a button must activate the button');
ok(spaceBelongsToTarget(within('[role="switch"]')), 'Space on a switch must toggle the switch');
ok(spaceBelongsToTarget(within('[role="tab"]')), 'Space on a tab must select the tab');
ok(!spaceBelongsToTarget(within()), 'Space on plain content must reach playback');
ok(!spaceBelongsToTarget(null), 'Space without a target must reach playback');

ok(insideModalSurface(within('[role="dialog"]')), 'dialog content is a modal surface');
ok(!insideModalSurface(within()), 'page content is not a modal surface');

ok(singleKeyAllowed(bare, { enabled: true, modalOpen: false }), 'bare keys work when enabled and no modal is open');
ok(!singleKeyAllowed(bare, { enabled: false, modalOpen: false }), 'bare keys stop when the setting is off');
ok(!singleKeyAllowed(bare, { enabled: true, modalOpen: true }), 'bare keys stop behind a modal');
ok(
	singleKeyAllowed({ ...bare, ctrlKey: true }, { enabled: false, modalOpen: true }),
	'Ctrl chords are never gated by the single-key setting'
);
console.log('Shortcut guard: OK');
