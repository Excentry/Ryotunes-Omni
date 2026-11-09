const APP_NAME = 'Ryotunes Omni';

const SEGMENT_TITLES: Record<string, string> = {
	search: 'Search',
	'search-more': 'More results',
	library: 'Library',
	list: 'List',
	radio: 'Radio',
	album: 'Album',
	artist: 'Artist',
	playlist: 'Playlist'
};

export function routeTitle(pathname: string): string {
	const first = pathname.split('/').filter(Boolean)[0] ?? '';
	const section = first ? SEGMENT_TITLES[first] : 'Home';
	return section ? `${section} · ${APP_NAME}` : APP_NAME;
}
