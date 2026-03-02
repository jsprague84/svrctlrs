<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import Toast from '$lib/components/ui/Toast.svelte';
	import * as serversState from '$lib/state/servers.svelte.js';
	import * as terminalState from '$lib/state/terminal.svelte.js';
	import type { Server } from '$lib/types/index.js';
	import { goto } from '$app/navigation';
	import { base } from '$app/paths';

	let { children } = $props();

	let sidebarCollapsed = $state(false);

	let servers = $derived(serversState.getServers());
	let serversLoading = $derived(serversState.isLoading());
	let serversError = $derived(serversState.getError());

	onMount(() => {
		serversState.loadServers();

		// Load sidebar preference
		const saved = localStorage.getItem('svrctlrs-sidebar-collapsed');
		if (saved === 'true') sidebarCollapsed = true;
	});

	function toggleSidebar() {
		sidebarCollapsed = !sidebarCollapsed;
		localStorage.setItem('svrctlrs-sidebar-collapsed', String(sidebarCollapsed));
	}

	function handleConnectServer(server: Server) {
		// Navigate to terminal page and create a tab for this server
		const tab = terminalState.createTab(server.id, server.name, 'pty');
		if (tab) {
			terminalState.setPendingAutoConnect(tab.id);
		}
		goto(`${base}/`);
	}
</script>

<div class="flex h-screen bg-background text-foreground">
	<Sidebar
		{servers}
		loading={serversLoading}
		error={serversError}
		collapsed={sidebarCollapsed}
		onToggle={toggleSidebar}
		onConnectServer={handleConnectServer}
	/>
	<main class="flex-1 min-w-0 flex flex-col">
		{@render children()}
	</main>
</div>
<Toast />
