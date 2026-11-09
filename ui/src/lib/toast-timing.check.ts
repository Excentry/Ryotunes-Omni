import { toastDuration } from './toast-timing.ts';

function ok(condition: unknown, message: string): asserts condition {
	if (!condition) throw new Error(`toast timing regression: ${message}`);
}

ok(toastDuration('Saved', 'success') === 2_500, 'short confirmations keep the 2.5 s floor');
ok(toastDuration('Saved', 'error') >= 10_000, 'errors stay at least 10 s');
ok(toastDuration('x'.repeat(200), 'info') > 2_500, 'long notices stay longer');
ok(
	toastDuration('x'.repeat(200), 'error') > toastDuration('x'.repeat(20), 'error'),
	'long errors outlast short ones'
);
console.log('Toast timing: OK');
