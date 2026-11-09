export type ToastKind = 'info' | 'success' | 'error';

const READING_MS_PER_CHAR = 60;

export function toastDuration(msg: string, kind: ToastKind): number {
	const reading = msg.length * READING_MS_PER_CHAR;
	return kind === 'error' ? Math.max(10_000, reading + 5_000) : Math.max(2_500, reading);
}
