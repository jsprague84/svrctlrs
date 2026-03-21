<script lang="ts">
	import Button from './Button.svelte';
	import Input from './Input.svelte';
	import { setServerUrl } from '$lib/platform/index.js';

	interface Props {
		onComplete: () => void;
	}

	let { onComplete }: Props = $props();

	let url = $state('');
	let testing = $state(false);
	let testResult = $state<'success' | 'error' | null>(null);
	let errorMessage = $state('');

	async function testConnection() {
		if (!url.trim()) return;
		testing = true;
		testResult = null;
		errorMessage = '';

		const cleanUrl = url.trim().replace(/\/+$/, '');

		try {
			const res = await fetch(`${cleanUrl}/api/v1/health`, {
				method: 'GET',
				headers: { 'Content-Type': 'application/json' }
			});
			if (res.ok) {
				testResult = 'success';
			} else {
				testResult = 'error';
				errorMessage = `Server returned ${res.status} ${res.statusText}`;
			}
		} catch (e) {
			testResult = 'error';
			errorMessage = 'Could not connect. Check the URL and ensure the server is running.';
		} finally {
			testing = false;
		}
	}

	function handleSave() {
		if (!url.trim()) return;
		setServerUrl(url.trim());
		onComplete();
	}
</script>

<div class="min-h-screen flex items-center justify-center bg-background px-4">
	<div class="w-full max-w-md bg-surface border border-border rounded-lg p-6 shadow-lg">
		<h1 class="text-lg font-semibold text-text-primary mb-1">Connect to SvrCtlRS Server</h1>
		<p class="text-sm text-text-muted mb-4">
			Enter the URL of your SvrCtlRS server to get started.
		</p>

		<div class="flex flex-col gap-3">
			<Input
				label="Server URL"
				bind:value={url}
				placeholder="https://svrctlrs.example.com"
			/>

			{#if testResult === 'success'}
				<p class="text-sm text-success">Connected successfully!</p>
			{:else if testResult === 'error'}
				<p class="text-sm text-error">{errorMessage}</p>
			{/if}

			<div class="flex gap-2">
				<Button variant="secondary" onclick={testConnection} disabled={!url.trim() || testing}>
					{testing ? 'Testing...' : 'Test Connection'}
				</Button>
				<Button onclick={handleSave} disabled={!url.trim() || testResult !== 'success'}>
					Save & Continue
				</Button>
			</div>
		</div>
	</div>
</div>
