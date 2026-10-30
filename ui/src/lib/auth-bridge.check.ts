import { createAuthBridge, startLogin } from './auth-bridge.ts';

function ok(condition: boolean, message: string): void {
	if (!condition) throw new Error(`FAIL: ${message}`);
}

const calls: Array<{ kind: string; name: string; payload?: unknown }> = [];
const listeners = new Map<string, (event: { payload: unknown }) => void>();
let rejectLogin = false;
let rejectListener = false;
const bridge = createAuthBridge(
	async <T>(command: string, args?: Record<string, unknown>) => {
		calls.push({ kind: 'invoke', name: command, payload: args });
		if (command === 'login_webview' && rejectLogin) {
			throw new Error('Sign-in timed out. Please try again.');
		}
		return undefined as T;
	},
	async <T>(event: string, handler: (event: { payload: T }) => void) => {
		calls.push({ kind: 'listen', name: event });
		if (rejectListener) throw new Error('Listener unavailable');
		listeners.set(event, handler as (event: { payload: unknown }) => void);
		return () => { listeners.delete(event); };
	}
);

await bridge.loginWebview();
ok(calls[0]?.kind === 'invoke' && calls[0].name === 'login_webview', 'login invokes native command');

let receivedAuth: unknown;
let receivedError = '';
let completed = false;
let selectionRequired = false;
const stopAuth = await bridge.onAuthChanged((account: { signedIn: boolean }) => (receivedAuth = account));
const stopSelection = await bridge.onAccountSelectionRequired(() => (selectionRequired = true));
const stopError = await bridge.onLoginError((message) => (receivedError = message));
const stopDone = await bridge.onLoginDone(() => (completed = true));
listeners.get('auth-changed')?.({ payload: { signedIn: true } });
listeners.get('account-selection-required')?.({ payload: undefined });
listeners.get('login-error')?.({ payload: 'Sign-in timed out. Please try again.' });
listeners.get('login-done')?.({ payload: undefined });
ok((receivedAuth as { signedIn: boolean }).signedIn, 'auth event payload reaches UI');
ok(selectionRequired, 'account-selection event reaches UI');
ok(receivedError.includes('timed out'), 'login error payload reaches UI');
ok(completed, 'login completion event reaches UI');

stopAuth();
stopSelection();
stopError();
stopDone();
ok(listeners.size === 0, 'bridge returns listener cleanup functions');
listeners.get('auth-changed')?.({ payload: { signedIn: false } });
ok((receivedAuth as { signedIn: boolean }).signedIn, 'removed listener receives no further events');

await bridge.switchAccount('account-1');
const switchCall = calls.find((call) => call.name === 'switch_account');
ok(
	JSON.stringify(switchCall?.payload) === JSON.stringify({ selectionKey: 'account-1' }),
	'account switch sends selection key'
);

rejectListener = true;
let listenerError = '';
try {
	await bridge.onAuthChanged(() => {});
} catch (error) {
	listenerError = String(error);
}
ok(listenerError === 'Error: Listener unavailable', 'listener registration failure propagates');
ok(listeners.size === 0, 'failed registration leaves no listener');

let recoverableError = '';
rejectLogin = true;
await startLogin(bridge.loginWebview, (message) => (recoverableError = message));
ok(
	recoverableError === 'Error: Sign-in timed out. Please try again.',
	'login command rejection becomes recoverable UI error'
);

let finishLogin!: () => void;
let loginFinished = false;
let unexpectedError = '';
const pendingLogin = startLogin(
	() => new Promise<void>((resolve) => { finishLogin = resolve; }),
	(message) => { unexpectedError = message; }
).then(() => { loginFinished = true; });
await Promise.resolve();
ok(!loginFinished, 'login stays pending while host command is pending');
finishLogin();
await pendingLogin;
ok(loginFinished && unexpectedError === '', 'successful login completes without error');

console.log('ok');
