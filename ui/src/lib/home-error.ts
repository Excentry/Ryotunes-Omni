export function homeRequestFailureMessage(
	_error: unknown,
	translate: (key: string) => string
): string {
	return translate('Could not load Home. Check your connection and try again.');
}
