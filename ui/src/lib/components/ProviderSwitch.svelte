<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { SpotifyIcon, YoutubeIcon } from '@hugeicons/core-free-icons';
	import type { Provider } from '$lib/api';
	import { spotify, selectProvider } from '$lib/spotify.svelte';
	import { toast } from '$lib/player.svelte';
	import { safeErrorMessage } from '$lib/safe-error';
	import { t } from '$lib/i18n.svelte';

	const options: { key: Provider; label: string; icon: typeof SpotifyIcon; brand: string }[] = [
		{ key: 'youtube', label: 'YouTube Music', icon: YoutubeIcon, brand: '#ff0033' },
		{ key: 'spotify', label: 'Spotify', icon: SpotifyIcon, brand: '#1db954' }
	];

	function choose(key: Provider) {
		selectProvider(key).catch((error) => toast.error(safeErrorMessage(error)));
	}
</script>

<div class="ryo-provider-switch flex h-7 items-center gap-0.5 rounded-md border border-border/70 p-0.5" role="group" aria-label={t('Music catalogue')}>
	{#each options as option (option.key)}
		{@const selected = spotify.provider === option.key}
		<button
			aria-pressed={selected}
			aria-label={option.label}
			title={option.label}
			onclick={() => choose(option.key)}
			class="flex h-full w-8 items-center justify-center rounded-[5px] transition-colors {selected ? '' : 'text-foreground/60 hover:bg-accent/10 hover:text-foreground'}"
			style={selected ? `color:${option.brand};background:color-mix(in srgb, ${option.brand} 18%, transparent);box-shadow:inset 0 0 0 1px color-mix(in srgb, ${option.brand} 50%, transparent)` : ''}
		>
			<HugeiconsIcon icon={option.icon} class="h-4 w-4" />
		</button>
	{/each}
</div>
