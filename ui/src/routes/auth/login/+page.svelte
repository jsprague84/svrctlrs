<script lang="ts">
	import { isTauri } from '$lib/platform/index.js';
	import { getServerUrl } from '$lib/platform/index.js';
	import { goto } from '$app/navigation';
	import Button from '$lib/components/ui/Button.svelte';
	import Input from '$lib/components/ui/Input.svelte';
	import * as toast from '$lib/state/toast.svelte.js';
	import { extractErrorMessage } from '$lib/utils/error.js';

	let username = $state('');
	let password = $state('');
	let loading = $state(false);

	async function handleLogin() {
		if (!username.trim() || !password.trim()) return;
		loading = true;

		try {
			const serverUrl = getServerUrl();
			const res = await fetch(`${serverUrl}/api/v1/auth/login`, {
				method: 'POST',
				credentials: 'include',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ username, password })
			});

			if (res.ok) {
				const data = await res.json().catch(() => ({}));
				// Store session token for Tauri mode (WebKitGTK can't send cross-origin cookies)
				if (data.session_id) {
					localStorage.setItem('svrctlrs-session-token', data.session_id);
				}
				// Full reload — token is in localStorage, survives reload
				window.location.href = '/';
			} else {
				const data = await res.json().catch(() => ({ error: 'Login failed' }));
				toast.error(data.error || 'Login failed');
			}
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Connection failed'));
		} finally {
			loading = false;
		}
	}

	// In web mode (not Tauri), redirect to server's login page
	if (typeof window !== 'undefined' && !isTauri()) {
		// Web mode — the server handles login via its own HTML page
		// This SvelteKit route is only used in Tauri mode
	}
</script>

<div class="min-h-screen flex items-center justify-center bg-background px-4">
	<div class="w-full max-w-sm bg-surface border border-border rounded-lg p-6 shadow-lg">
		<h1 class="text-lg font-semibold text-text-primary mb-1">Sign In</h1>
		<p class="text-sm text-text-muted mb-4">Enter your SvrCtlRS credentials</p>

		<form class="flex flex-col gap-3" onsubmit={(e) => { e.preventDefault(); handleLogin(); }}>
			<Input label="Username" bind:value={username} required />
			<div>
				<label class="block text-xs font-medium text-text-secondary mb-1">Password</label>
				<input
					type="password"
					bind:value={password}
					required
					class="w-full px-3 py-1.5 text-sm bg-input border border-border rounded-sm text-text-primary focus:ring-1 focus:ring-ring focus:outline-none"
				/>
			</div>
			<Button onclick={handleLogin} disabled={loading || !username.trim() || !password.trim()}>
				{loading ? 'Signing in...' : 'Sign In'}
			</Button>
		</form>
	</div>
</div>
