import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const [port = '9337', out] = process.argv.slice(2);
if (!out) throw new Error('usage: node win-capture.mjs <cdp-port> <output-dir>');
mkdirSync(out, { recursive: true });

const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = targets.find((t) => t.type === 'page' && t.url.startsWith('http://tauri.localhost/') && !t.url.includes('mini'));
if (!page) throw new Error('main Ryotunes WebView not found');
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve) => ws.addEventListener('open', resolve, { once: true }));

let nextId = 0;
const pending = new Map();
ws.addEventListener('message', (event) => {
	const msg = JSON.parse(event.data);
	if (pending.has(msg.id)) {
		pending.get(msg.id)(msg);
		pending.delete(msg.id);
	}
});
const send = (method, params = {}) =>
	new Promise((resolve, reject) => {
		const id = ++nextId;
		pending.set(id, (msg) => (msg.error ? reject(new Error(`${method}: ${msg.error.message}`)) : resolve(msg.result)));
		ws.send(JSON.stringify({ id, method, params }));
	});
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const evaluate = async (expression) => {
	const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
	if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? 'evaluate failed');
	return r.result.value;
};
const invoke = (cmd, args = {}) =>
	evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args)})`);
const shot = async (name, waitMs = 6000) => {
	await sleep(waitMs);
	const { data } = await send('Page.captureScreenshot', { format: 'png' });
	writeFileSync(join(out, `${name}.png`), Buffer.from(data, 'base64'));
	console.log(name);
};
const go = async (path) => {
	await evaluate(`location.assign(${JSON.stringify(path)})`).catch(() => {});
	await sleep(500);
};
const setStorage = (key, value) => evaluate(`localStorage.setItem(${JSON.stringify(key)}, ${JSON.stringify(value)})`);
const setTheme = (mode) =>
	evaluate(`(() => { const k = 'ryotunes:appearance'; const a = JSON.parse(localStorage.getItem(k) || '{}'); a.themeMode = ${JSON.stringify(mode)}; localStorage.setItem(k, JSON.stringify(a)); })()`);
const key = async (code, keyName, modifiers = 0) => {
	for (const type of ['keyDown', 'keyUp']) {
		await send('Input.dispatchKeyEvent', { type, code, key: keyName, modifiers, windowsVirtualKeyCode: keyName.toUpperCase().charCodeAt(0) });
	}
};
const fontsFor = (selector) =>
	(async () => {
		const found = await send('Runtime.evaluate', { expression: `document.querySelector(${JSON.stringify(selector)})` });
		if (!found.result.objectId) return null;
		const { nodeId } = await send('DOM.requestNode', { objectId: found.result.objectId });
		return (await send('CSS.getPlatformFontsForNode', { nodeId })).fonts;
	})();

const ALBUM = 'MPREb_K8qWMWVqXGi';
const ARTIST = 'UCRr1xG_2WIDs18a6cIiCxeA';
const QUERY = 'Daft Punk Get Lucky';

await send('Page.enable');
await send('Runtime.enable');
await send('DOM.enable');
await send('CSS.enable');
await send('Emulation.setDeviceMetricsOverride', { width: 1376, height: 820, deviceScaleFactor: 1, mobile: false });

for (const theme of ['dark', 'light']) {
	await setTheme(theme);
	await setStorage('sidebar_collapsed', '0');
	await go('/');
	await shot(`${theme}-00-home-loading`, 1500);
	await shot(`${theme}-01-home-no-track`, 8000);
	if (theme === 'dark') {
		await send('DOM.getDocument', { depth: 0 });
		const fonts = { heading: await fontsFor('h1'), body: await fontsFor('main p') };
		writeFileSync(join(out, 'fonts.json'), JSON.stringify(fonts, null, 2));
	}
	await go(`/search?q=${encodeURIComponent(QUERY)}`);
	await shot(`${theme}-02-search`);
	await go('/library');
	await shot(`${theme}-03-library`);
	await go(`/album/${ALBUM}`);
	await shot(`${theme}-04-album`, 8000);
	await go(`/artist/${ARTIST}`);
	await shot(`${theme}-05-artist`, 8000);
	await setStorage('sidebar_collapsed', '1');
	await go(`/artist/${ARTIST}`);
	await shot(`${theme}-06-rail-collapsed`, 8000);
	await setStorage('sidebar_collapsed', '0');
}

await setTheme('dark');
await go(`/search?q=${encodeURIComponent(QUERY)}`);
await sleep(4000);
const items = await invoke('search', { query: QUERY });
await invoke('play', { item: items[0] });
await shot('dark-07-playing', 15000);
await invoke('toggle_pause');
await shot('dark-08-paused', 3000);
await invoke('toggle_pause');
await key('KeyQ', 'q');
await shot('dark-09-panel-queue', 4000);
await key('KeyQ', 'q');
await key('KeyE', 'e', 2);
await shot('dark-10-nowplaying-queue', 5000);
await key('KeyL', 'l');
await shot('dark-11-nowplaying-lyrics', 6000);
await key('Escape', 'Escape');
await key('Escape', 'Escape');
await setTheme('light');
await go(`/search?q=${encodeURIComponent(QUERY)}`);
await sleep(4000);
await key('KeyE', 'e', 2);
await shot('light-10-nowplaying-queue', 5000);
await key('Escape', 'Escape');
await setTheme('dark');
writeFileSync(join(out, 'playback.json'), JSON.stringify(await invoke('get_playback'), null, 2));
ws.close();
