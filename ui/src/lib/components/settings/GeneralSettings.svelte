<script lang="ts">
	import { onMount } from 'svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import { isTauri, getServerUrl } from '$lib/platform/index.js';
	import * as themeState from '$lib/state/theme.svelte.js';
	import * as profilesState from '$lib/state/profiles.svelte.js';
	import * as settingsState from '$lib/state/settings.svelte.js';
	import * as toast from '$lib/state/toast.svelte.js';
	import { extractErrorMessage } from '$lib/utils/error.js';
	import { put } from '$lib/api/client.js';

	let profiles = $derived(profilesState.getProfiles());
	let defaultProfileId = $state('');

	onMount(async () => {
		await profilesState.loadProfiles();
		// Load current default profile setting
		const settings = settingsState.getSettings();
		const appSettings = settings['app'] ?? [];
		const defaultSetting = appSettings.find((s: { key: string }) => s.key === 'app.default_profile_id');
		if (defaultSetting) {
			defaultProfileId = defaultSetting.value;
		}
	});

	async function saveDefaultProfile() {
		try {
			await put('/settings/app.default_profile_id', { value: defaultProfileId });
			toast.success('Default profile updated');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to save'));
		}
	}

	function changeServerUrl() {
		localStorage.removeItem('svrctlrs-server-url');
		window.location.reload();
	}
</script>

<div class="flex flex-col gap-6">
	<!-- Server Connection (Tauri only) -->
	{#if isTauri()}
		<div class="p-4 bg-surface border border-border rounded-lg">
			<h3 class="text-[11px] uppercase tracking-wider text-text-muted font-semibold mb-3">Server Connection</h3>
			<div class="flex items-center gap-3">
				<span class="text-sm text-text-secondary flex-1 font-mono truncate">{getServerUrl() || 'Not configured'}</span>
				<Button size="sm" variant="secondary" onclick={changeServerUrl}>Change</Button>
			</div>
			<p class="text-[10px] text-text-muted mt-2">Connected • Session active</p>
		</div>
	{/if}

	<!-- Appearance -->
	<div>
		<h3 class="text-[11px] uppercase tracking-wider text-text-muted font-semibold mb-3">Appearance</h3>
		<div class="flex items-center justify-between py-2">
			<div>
				<p class="text-sm text-text-primary">Theme</p>
				<p class="text-[10px] text-text-muted">Choose dark or light mode</p>
			</div>
			<div class="flex gap-1">
				<button
					class="px-3 py-1 text-xs rounded-sm transition-colors {themeState.isDark() ? 'bg-accent text-background font-medium' : 'bg-surface-raised text-text-secondary'}"
					onclick={() => { if (!themeState.isDark()) themeState.toggleTheme(); }}
				>Dark</button>
				<button
					class="px-3 py-1 text-xs rounded-sm transition-colors {!themeState.isDark() ? 'bg-accent text-background font-medium' : 'bg-surface-raised text-text-secondary'}"
					onclick={() => { if (themeState.isDark()) themeState.toggleTheme(); }}
				>Light</button>
			</div>
		</div>
	</div>

	<!-- Data -->
	<div>
		<h3 class="text-[11px] uppercase tracking-wider text-text-muted font-semibold mb-3">Data</h3>
		<div class="flex items-center justify-between py-2">
			<div>
				<p class="text-sm text-text-primary">Default Terminal Profile</p>
				<p class="text-[10px] text-text-muted">Auto-load this profile on app start</p>
			</div>
			<div class="flex items-center gap-2">
				<select
					class="px-2 py-1 text-xs bg-input border border-border rounded-sm text-text-primary"
					bind:value={defaultProfileId}
					onchange={saveDefaultProfile}
				>
					<option value="">None</option>
					{#each profiles as profile}
						<option value={String(profile.id)}>{profile.name}</option>
					{/each}
				</select>
			</div>
		</div>
	</div>
</div>
