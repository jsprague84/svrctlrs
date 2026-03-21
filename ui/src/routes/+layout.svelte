<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import Toast from '$lib/components/ui/Toast.svelte';
	import * as serversState from '$lib/state/servers.svelte.js';
	import * as profilesState from '$lib/state/profiles.svelte.js';
	import { isMobile } from '$lib/state/mobile.svelte.js';
	import * as terminalState from '$lib/state/terminal.svelte.js';
	import * as toast from '$lib/state/toast.svelte.js';
	import type { Server, TerminalProfile, LayoutMode } from '$lib/types/index.js';
	import { extractErrorMessage } from '$lib/utils/error.js';
	import { goto } from '$app/navigation';
	import { base } from '$app/paths';
	import { Menu } from 'lucide-svelte';

	let { children } = $props();

	let sidebarCollapsed = $state(false);
	let mobileOpen = $state(false);

	let servers = $derived(serversState.getServers());
	let serversLoading = $derived(serversState.isLoading());
	let serversError = $derived(serversState.getError());
	let profiles = $derived(profilesState.getProfiles());

	// Set compact density on mobile
	$effect(() => {
		document.documentElement.dataset.density = isMobile() ? 'compact' : 'comfortable';
	});

	onMount(() => {
		serversState.loadServers();
		profilesState.loadProfiles();

		// Load sidebar preference
		const saved = localStorage.getItem('svrctlrs-sidebar-collapsed');
		if (saved === 'true') sidebarCollapsed = true;
	});

	function toggleSidebar() {
		sidebarCollapsed = !sidebarCollapsed;
		localStorage.setItem('svrctlrs-sidebar-collapsed', String(sidebarCollapsed));
	}

	function closeMobileSidebar() {
		mobileOpen = false;
	}

	function handleConnectServer(server: Server) {
		// Navigate to terminal page and create a tab for this server
		const tab = terminalState.createTab(server.id, server.name, 'pty');
		if (tab) {
			terminalState.setPendingAutoConnect(tab.id);
		}
		goto(`${base}/`);
	}

	function handleLoadProfile(profile: TerminalProfile) {
		const serverNames: Record<number, string> = {};
		for (const s of servers) {
			serverNames[s.id] = s.name;
		}
		terminalState.applyProfile(
			profile.layout as LayoutMode,
			profile.pane_configs ?? [],
			serverNames
		);
		goto(`${base}/`);
	}

	async function handleDeleteProfile(id: number) {
		try {
			await profilesState.deleteProfile(id);
			toast.success('Profile deleted');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to delete profile'));
		}
	}
</script>

<!-- Hamburger button (mobile only) -->
<button
	class="fixed top-2 left-2 z-50 p-2 rounded-md bg-surface border border-border text-foreground md:hidden"
	onclick={() => (mobileOpen = !mobileOpen)}
	aria-label={mobileOpen ? 'Close sidebar' : 'Open sidebar'}
>
	<Menu class="w-5 h-5" />
</button>

<!-- Mobile backdrop -->
{#if mobileOpen}
	<button
		class="fixed inset-0 bg-black/50 z-30 md:hidden"
		onclick={closeMobileSidebar}
		aria-label="Close sidebar"
		tabindex="-1"
	></button>
{/if}

<div class="flex h-screen bg-background text-foreground">
	<Sidebar
		{servers}
		{profiles}
		loading={serversLoading}
		error={serversError}
		collapsed={sidebarCollapsed}
		{mobileOpen}
		onToggle={toggleSidebar}
		onMobileClose={closeMobileSidebar}
		onConnectServer={handleConnectServer}
		onLoadProfile={handleLoadProfile}
		onDeleteProfile={handleDeleteProfile}
	/>
	<main class="flex-1 min-w-0 flex flex-col pl-12 md:pl-0">
		{@render children()}
	</main>
</div>
<Toast />
