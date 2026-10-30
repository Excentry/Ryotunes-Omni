import { homeRequestFailureMessage } from './home-error.ts';
import { safeErrorMessage } from './safe-error.ts';

function ok(condition: boolean, message: string): void {
	if (!condition) throw new Error(message);
}

const raw = new Error(
	'HTTP request failed for url (https://music.youtube.com/youtubei/v1/browse?key=api-secret&token=session-secret)'
);
const message = homeRequestFailureMessage(raw, (key) => key);
const diagnostic = safeErrorMessage(raw);
ok(
	message === 'Could not load Home. Check your connection and try again.',
	'home error is recoverable and generic'
);
ok(!message.includes('music.youtube.com'), 'home message hides request hosts');
ok(!diagnostic.includes('music.youtube.com'), 'diagnostics hide request hosts');
ok(!diagnostic.includes('api-secret'), 'diagnostics hide API query values');
ok(!diagnostic.includes('session-secret'), 'diagnostics hide session-like query values');

console.log('ok');
