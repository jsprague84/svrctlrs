<script lang="ts">
	import { onMount } from 'svelte';
	import { getPlatform } from '$lib/platform/index.js';
	import { get } from '$lib/api/client.js';
	import Badge from '$lib/components/ui/Badge.svelte';

	let serverHealth = $state<{ status: string; version?: string } | null>(null);
	let healthError = $state<string | null>(null);

	onMount(async () => {
		try {
			const res = await get<{ status: string; version: string }>('/health');
			serverHealth = res;
		} catch {
			healthError = 'Unable to reach server';
		}
	});

	const platform = getPlatform();
	const platformLabel = platform === 'web' ? 'Web' : platform === 'tauri-desktop' ? 'Desktop' : 'Mobile';

	const shortcuts = [
		{ category: 'Tabs', keys: 'Ctrl+Shift+T', action: 'New tab' },
		{ category: 'Tabs', keys: 'Ctrl+Shift+W', action: 'Close tab' },
		{ category: 'Tabs', keys: 'Ctrl+Tab', action: 'Next tab' },
		{ category: 'Tabs', keys: 'Ctrl+Shift+Tab', action: 'Previous tab' },
		{ category: 'Panes', keys: 'Alt+1-4', action: 'Focus pane by slot' },
		{ category: 'Panes', keys: 'Alt+[ / ]', action: 'Cycle panes' },
		{ category: 'Panes', keys: 'Alt+Arrow', action: 'Spatial navigation' },
		{ category: 'Layout', keys: 'Ctrl+\\', action: 'Cycle layouts' },
		{ category: 'Actions', keys: 'Ctrl+K', action: 'Command palette' },
		{ category: 'Actions', keys: 'Ctrl+Shift+F', action: 'Search output' },
	];

	let categories = $derived([...new Set(shortcuts.map(s => s.category))]);
</script>

<div class="flex flex-col gap-6">
	<!-- App Info -->
	<div>
		<h3 class="text-[11px] uppercase tracking-wider text-text-muted font-semibold mb-3">Application</h3>
		<div class="flex flex-col gap-2">
			<div class="flex items-center justify-between py-1">
				<span class="text-sm text-text-secondary">Name</span>
				<span class="text-sm text-text-primary font-medium">SvrCtlRS</span>
			</div>
			<div class="flex items-center justify-between py-1">
				<span class="text-sm text-text-secondary">Version</span>
				<span class="text-sm text-text-primary font-mono">0.1.0</span>
			</div>
			<div class="flex items-center justify-between py-1">
				<span class="text-sm text-text-secondary">Platform</span>
				<Badge>{platformLabel}</Badge>
			</div>
		</div>
	</div>

	<!-- Server Health -->
	<div>
		<h3 class="text-[11px] uppercase tracking-wider text-text-muted font-semibold mb-3">Server</h3>
		<div class="flex items-center justify-between py-1">
			<span class="text-sm text-text-secondary">Status</span>
			{#if serverHealth}
				<Badge variant="success">{serverHealth.status}</Badge>
			{:else if healthError}
				<Badge variant="error">{healthError}</Badge>
			{:else}
				<span class="text-sm text-text-muted">Checking...</span>
			{/if}
		</div>
		{#if serverHealth?.version}
			<div class="flex items-center justify-between py-1">
				<span class="text-sm text-text-secondary">Server Version</span>
				<span class="text-sm text-text-primary font-mono">{serverHealth.version}</span>
			</div>
		{/if}
	</div>

	<!-- Keyboard Shortcuts -->
	<div>
		<h3 class="text-[11px] uppercase tracking-wider text-text-muted font-semibold mb-3">Keyboard Shortcuts</h3>
		{#each categories as cat}
			<div class="mb-3">
				<p class="text-xs font-medium text-text-secondary mb-1">{cat}</p>
				<div class="flex flex-col gap-1">
					{#each shortcuts.filter(s => s.category === cat) as shortcut}
						<div class="flex items-center justify-between py-0.5">
							<span class="text-xs text-text-primary">{shortcut.action}</span>
							<kbd class="px-1.5 py-0.5 text-[10px] font-mono bg-surface-raised border border-border rounded text-text-secondary">{shortcut.keys}</kbd>
						</div>
					{/each}
				</div>
			</div>
		{/each}
	</div>
</div>
