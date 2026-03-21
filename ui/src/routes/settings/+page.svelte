<script lang="ts">
	import { Settings, Bell, TerminalSquare, Info } from 'lucide-svelte';
	import { isMobile } from '$lib/state/mobile.svelte.js';
	import GeneralSettings from '$lib/components/settings/GeneralSettings.svelte';

	type TabId = 'general' | 'notifications' | 'commands' | 'about';

	let activeTab = $state<TabId>('general');

	const tabs: Array<{ id: TabId; label: string; icon: typeof Settings }> = [
		{ id: 'general', label: 'General', icon: Settings },
		{ id: 'notifications', label: 'Notifications', icon: Bell },
		{ id: 'commands', label: 'Quick Commands', icon: TerminalSquare },
		{ id: 'about', label: 'About', icon: Info }
	];
</script>

<div class="flex flex-col h-full">
	<!-- Header -->
	<div class="flex items-center gap-3 px-4 md:px-6 py-4 border-b border-border">
		<Settings class="w-5 h-5 text-accent" />
		<h1 class="text-lg font-semibold text-text-primary">Settings</h1>
	</div>

	<!-- Tab bar -->
	<div class="flex border-b border-border px-2 md:px-4">
		{#each tabs as tab}
			<button
				class="flex items-center gap-1.5 px-3 md:px-4 py-2.5 text-sm font-medium transition-colors
					{activeTab === tab.id
						? 'text-accent border-b-2 border-accent -mb-px'
						: 'text-text-muted hover:text-text-secondary'}"
				onclick={() => activeTab = tab.id}
			>
				<tab.icon class="w-4 h-4" />
				{#if !isMobile()}<span>{tab.label}</span>{/if}
			</button>
		{/each}
	</div>

	<!-- Tab content -->
	<div class="flex-1 overflow-y-auto p-4 md:p-6">
		{#if activeTab === 'general'}
			<GeneralSettings />
		{:else if activeTab === 'notifications'}
			<div class="text-text-muted text-sm">Notification settings — coming soon</div>
		{:else if activeTab === 'commands'}
			<div class="text-text-muted text-sm">Quick commands — coming soon</div>
		{:else if activeTab === 'about'}
			<div class="text-text-muted text-sm">About — coming soon</div>
		{/if}
	</div>
</div>
