const requestUrl = /https?:\/\/[^\s"'<>)]*/gi;

export function safeErrorMessage(error: unknown): string {
	return String(error).replace(requestUrl, '[request URL redacted]');
}
