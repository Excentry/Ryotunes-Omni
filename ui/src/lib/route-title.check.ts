import { routeTitle } from './route-title.ts';

function ok(condition: unknown, message: string): asserts condition {
	if (!condition) throw new Error(`route title regression: ${message}`);
}

ok(routeTitle('/') === 'Home · Ryotunes Omni', 'the root route is Home');
ok(routeTitle('/search') === 'Search · Ryotunes Omni', 'search has its own title');
ok(routeTitle('/album/MPREb_123') === 'Album · Ryotunes Omni', 'detail routes use their section');
ok(routeTitle('/search-more') === 'More results · Ryotunes Omni', 'hyphenated sections are distinct');
ok(routeTitle('/unknown/path') === 'Ryotunes Omni', 'unknown routes fall back to the app name');
console.log('Route title: OK');
