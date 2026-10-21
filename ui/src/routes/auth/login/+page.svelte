<script lang="ts">
	// The desktop app's sign-in. (The browser never sees this page: it loads the server's own
	// login page, which offers the identity provider, a password and two-factor.)
	import { onDestroy } from 'svelte';
	import { ExternalLink } from '@lucide/svelte';
	import { getServerUrl } from '#lib/platform/index.js';
	import { signInWithBrowser } from '#lib/platform/appToken.js';
	import Button from '#lib/components/ui/Button.svelte';
	import { extractErrorMessage } from '#lib/utils/error.js';

	let waiting = $state(false);
	let error = $state<string | null>(null);
	let left = true;
	onDestroy(() => (left = false));

	async function signIn() {
		waiting = true;
		error = null;
		try {
			await signInWithBrowser();
			// Full reload so every state module starts from the signed-in session
			if (left) window.location.href = '/';
		} catch (e) {
			error = extractErrorMessage(e, 'Sign-in did not finish');
		} finally {
			waiting = false;
		}
	}
</script>

<div class="min-h-screen flex items-center justify-center bg-background px-4">
	<div class="w-full max-w-sm bg-surface border border-border rounded-lg p-6 shadow-lg">
		<h1 class="text-lg font-semibold text-text-primary mb-1">Sign in</h1>
		<p class="text-sm text-text-muted mb-4 break-all">{getServerUrl()}</p>

		{#if waiting}
			<p class="text-sm text-text-secondary mb-4" role="status">
				Finish signing in in your browser, then come back here. The browser asks you to approve this
				app.
			</p>
		{:else}
			<p class="text-sm text-text-secondary mb-4">
				SvrCtlRS opens your browser, where you sign in as usual (with your identity provider, or a
				password and code) and approve this app.
			</p>
		{/if}

		{#if error}
			<p class="text-sm text-error mb-4" role="alert">{error}</p>
		{/if}

		<Button onclick={signIn} disabled={waiting}>
			<ExternalLink class="w-4 h-4" />
			{waiting ? 'Waiting for the browser…' : 'Sign in with your browser'}
		</Button>
	</div>
</div>
