<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { SpotifyIcon } from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import * as api from '$lib/api';
	import { spotify, signInToSpotify } from '$lib/spotify.svelte';
	import { t } from '$lib/i18n.svelte';

	const waiting = $derived(spotify.flow === 'browser');
</script>

<section class="ryo-spotify-gate mx-auto flex max-w-md flex-col items-center gap-4 px-6 py-16 text-center" aria-live="polite">
	<HugeiconsIcon icon={SpotifyIcon} class="h-10 w-10 text-[#1db954]" aria-hidden="true" />
	<h2 class="font-heading text-2xl">{t('Connect Spotify')}</h2>
	<Button onclick={signInToSpotify} disabled={waiting}>
		{waiting ? t('Waiting for your browser…') : spotify.error ? t('Try again') : t('Sign in to Spotify')}
	</Button>
	{#if spotify.error}
		<p class="text-sm text-destructive" role="alert">{spotify.error}</p>
	{:else if waiting}
		<p class="text-sm text-muted-foreground">{t('Finish the authorization in the browser tab that opened, then come back here.')}</p>
	{:else}
		<p class="text-sm text-muted-foreground">
			{t('Your Spotify playlists, saved music and Home appear here. Spotify requires a Premium account to connect, and songs play from YouTube Music.')}
		</p>
	{/if}
	{#if waiting && spotify.authUrl}
		<button class="text-sm underline underline-offset-4" onclick={() => api.openExternal(spotify.authUrl)}>
			{t('Open the Spotify page again')}
		</button>
	{/if}
</section>
