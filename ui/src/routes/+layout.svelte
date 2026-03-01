<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import Toast from '$lib/components/ui/Toast.svelte';
	import * as serversApi from '$lib/api/servers.js';
	import * as terminalState from '$lib/state/terminal.svelte.js';
	import type { Server } from '$lib/types/index.js';
	import { goto } from '$app/navigation';
	import { base } from '$app/paths';

	let { children } = $props();

	let servers = $state<Server[]>([]);
	let sidebarCollapsed = $state(false);

	onMount(() => {
		serversApi.listServers().then((s) => { servers = s; }).catch(() => {});

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
		terminalState.createTab(server.id, server.name, 'pty');
		goto(`${base}/`);
	}
</script>

<div class="flex h-screen bg-background text-foreground">
	<Sidebar
		{servers}
		collapsed={sidebarCollapsed}
		onToggle={toggleSidebar}
		onConnectServer={handleConnectServer}
	/>
	<main class="flex-1 min-w-0 flex flex-col">
		{@render children()}
	</main>
</div>
<Toast />
