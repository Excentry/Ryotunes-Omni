import type { UnlistenFn } from '@tauri-apps/api/event';

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
type Listen = <T>(
	event: string,
	handler: (event: { payload: T }) => void
) => Promise<UnlistenFn>;

export function createAuthBridge(invoke: Invoke, listen: Listen) {
	return {
		getAccount: <T>() => invoke<T>('get_account'),
		getAccountIdentities: <T>() => invoke<T>('get_account_identities'),
		switchAccount: <T>(selectionKey: string) =>
			invoke<T>('switch_account', { selectionKey }),
		signOut: () => invoke<void>('sign_out'),
		loginWebview: () => invoke<void>('login_webview'),
		onAuthChanged: <T>(cb: (account: T) => void) =>
			listen<T>('auth-changed', (event) => cb(event.payload)),
		onAccountSelectionRequired: (cb: () => void) =>
			listen<void>('account-selection-required', () => cb()),
		onLoginError: (cb: (message: string) => void) =>
			listen<string>('login-error', (event) => cb(event.payload)),
		onLoginDone: (cb: () => void) => listen<void>('login-done', () => cb())
	};
}

let loginInFlight: Promise<void> | null = null;

export function startLogin(
	openLogin: () => Promise<void>,
	onError: (message: string) => void
): Promise<void> {
	if (loginInFlight) return loginInFlight;
	loginInFlight = (async () => {
		try {
			await openLogin();
		} catch (error) {
			onError(String(error));
		} finally {
			loginInFlight = null;
		}
	})();
	return loginInFlight;
}
