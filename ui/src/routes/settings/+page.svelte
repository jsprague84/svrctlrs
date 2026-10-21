<script lang="ts">
	import { Settings, Bell, TerminalSquare, KeyRound, ScrollText, Info } from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import GeneralSettings from '#lib/components/settings/GeneralSettings.svelte';
	import NotificationSettings from '#lib/components/settings/NotificationSettings.svelte';
	import QuickCommandsSettings from '#lib/components/settings/QuickCommandsSettings.svelte';
	import AboutSettings from '#lib/components/settings/AboutSettings.svelte';
	import UsersSettings from '#lib/components/settings/UsersSettings.svelte';
	import SignInSettings from '#lib/components/settings/SignInSettings.svelte';
	import DevicesSettings from '#lib/components/settings/DevicesSettings.svelte';
	import TokensSettings from '#lib/components/settings/TokensSettings.svelte';
	import AuditSettings from '#lib/components/settings/AuditSettings.svelte';

	type TabId = 'general' | 'notifications' | 'commands' | 'access' | 'audit' | 'about';

	const tabs: Array<{ id: TabId; label: string; short?: string; icon: typeof Settings }> = [
		{ id: 'general', label: 'General', icon: Settings },
		{ id: 'notifications', label: 'Notifications', icon: Bell },
		{ id: 'commands', label: 'Quick Commands', short: 'Commands', icon: TerminalSquare },
		{ id: 'access', label: 'Access', icon: KeyRound },
		{ id: 'audit', label: 'Audit', icon: ScrollText },
		{ id: 'about', label: 'About', icon: Info }
	];

	// The tab lives in the URL (?tab=audit) so a reload keeps it and docs can link to it.
	// `users` is the Access tab's old name.
	let activeTab = $derived.by((): TabId => {
		const raw = page.url?.searchParams.get('tab');
		const t = raw === 'users' ? 'access' : raw;
		return tabs.some((x) => x.id === t) ? (t as TabId) : 'general';
	});

	let tabButtons: HTMLButtonElement[] = $state([]);

	function select(id: TabId) {
		goto(`${resolve('/settings')}?tab=${id}`, { replace: true });
	}

	/** Roving focus: arrows move between tabs, Home/End jump, the focused tab is activated */
	function onKeydown(e: KeyboardEvent, index: number) {
		let next: number;
		if (e.key === 'ArrowRight') next = (index + 1) % tabs.length;
		else if (e.key === 'ArrowLeft') next = (index - 1 + tabs.length) % tabs.length;
		else if (e.key === 'Home') next = 0;
		else if (e.key === 'End') next = tabs.length - 1;
		else return;
		e.preventDefault();
		select(tabs[next].id);
		tabButtons[next]?.focus();
	}
</script>

<div class="flex flex-col h-full">
	<div class="flex items-center gap-3 px-4 md:px-6 py-4 border-b border-border">
		<Settings class="w-5 h-5 text-accent" />
		<h1 class="text-lg font-semibold text-text-primary">Settings</h1>
	</div>

	<!-- Phones: a 3×2 grid so every section is visible; wider screens: one row -->
	<div
		class="grid grid-cols-3 md:flex border-b border-border px-2 md:px-4"
		role="tablist"
		aria-label="Settings sections"
	>
		{#each tabs as tab, i (tab.id)}
			<button
				bind:this={tabButtons[i]}
				role="tab"
				id="settings-tab-{tab.id}"
				aria-selected={activeTab === tab.id}
				aria-controls="settings-panel-{tab.id}"
				tabindex={activeTab === tab.id ? 0 : -1}
				class="flex items-center justify-center md:justify-start gap-1.5 px-2 md:px-4 min-h-11 md:min-h-0 py-2.5 text-sm font-medium whitespace-nowrap transition-colors border-b-2 -mb-px
					{activeTab === tab.id
					? 'text-accent border-accent'
					: 'text-text-muted hover:text-text-secondary border-transparent'}"
				onclick={() => select(tab.id)}
				onkeydown={(e) => onKeydown(e, i)}
			>
				<tab.icon class="w-4 h-4 flex-shrink-0" />
				{#if tab.short}
					<span class="md:hidden">{tab.short}</span>
					<span class="hidden md:inline">{tab.label}</span>
				{:else}
					<span>{tab.label}</span>
				{/if}
			</button>
		{/each}
	</div>

	<div
		class="flex-1 overflow-y-auto p-4 md:p-6"
		role="tabpanel"
		id="settings-panel-{activeTab}"
		aria-labelledby="settings-tab-{activeTab}"
	>
		{#if activeTab === 'general'}
			<GeneralSettings />
		{:else if activeTab === 'notifications'}
			<NotificationSettings />
		{:else if activeTab === 'commands'}
			<QuickCommandsSettings />
		{:else if activeTab === 'access'}
			<div class="flex flex-col gap-6 max-w-3xl">
				<SignInSettings />
				<DevicesSettings />
				<TokensSettings />
				<UsersSettings />
			</div>
		{:else if activeTab === 'audit'}
			<AuditSettings />
		{:else if activeTab === 'about'}
			<AboutSettings />
		{/if}
	</div>
</div>
