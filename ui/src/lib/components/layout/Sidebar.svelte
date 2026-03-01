<script lang="ts">
	import { page } from '$app/state';
	import { base } from '$app/paths';
	import { Terminal, Server, KeyRound, Settings, PanelLeftClose, PanelLeftOpen } from 'lucide-svelte';
	import type { Server as ServerType } from '$lib/types/index.js';

	interface Props {
		servers: ServerType[];
		collapsed?: boolean;
		onToggle: () => void;
		onConnectServer: (server: ServerType) => void;
	}

	let { servers, collapsed = false, onToggle, onConnectServer }: Props = $props();

	const navItems = [
		{ href: `${base}/`, icon: Terminal, label: 'Terminal' },
		{ href: `${base}/servers`, icon: Server, label: 'Servers' },
		{ href: `${base}/credentials`, icon: KeyRound, label: 'Credentials' },
		{ href: `${base}/settings`, icon: Settings, label: 'Settings' }
	];

	function isActive(href: string): boolean {
		const path = page.url?.pathname ?? '';
		if (href === `${base}/`) return path === `${base}/` || path === base;
		return path.startsWith(href);
	}
</script>

<aside
	class="flex flex-col bg-sidebar border-r border-sidebar-border transition-all duration-200
		{collapsed ? 'w-12' : 'w-56'}"
>
	<!-- Logo / Toggle -->
	<div class="flex items-center justify-between px-3 py-3 border-b border-sidebar-border">
		{#if !collapsed}
			<span class="text-sm font-bold text-sidebar-foreground tracking-tight">SvrCtlRS</span>
		{/if}
		<button
			class="p-1 text-sidebar-muted hover:text-sidebar-foreground rounded-sm"
			onclick={onToggle}
			title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
		>
			{#if collapsed}
				<PanelLeftOpen class="w-4 h-4" />
			{:else}
				<PanelLeftClose class="w-4 h-4" />
			{/if}
		</button>
	</div>

	<!-- Navigation -->
	<nav class="flex flex-col gap-0.5 px-2 py-2">
		{#each navItems as item}
			<a
				href={item.href}
				class="flex items-center gap-2.5 px-2 py-1.5 rounded-sm text-sm transition-colors
					{isActive(item.href)
						? 'bg-sidebar-accent text-sidebar-foreground'
						: 'text-sidebar-muted hover:text-sidebar-foreground hover:bg-sidebar-accent/50'}"
				title={collapsed ? item.label : undefined}
			>
				<item.icon class="w-4 h-4 flex-shrink-0" />
				{#if !collapsed}
					<span>{item.label}</span>
				{/if}
			</a>
		{/each}
	</nav>

	<!-- Server list (not collapsed) -->
	{#if !collapsed}
		<div class="flex-1 overflow-y-auto border-t border-sidebar-border">
			<div class="px-3 py-2">
				<h3 class="text-[10px] uppercase tracking-wider text-sidebar-muted font-semibold mb-1.5">Servers</h3>
				{#if servers.length === 0}
					<p class="text-xs text-sidebar-muted italic">No servers configured</p>
				{:else}
					<div class="flex flex-col gap-0.5">
						{#each servers as server}
							<button
								class="flex items-center gap-2 px-2 py-1.5 text-xs rounded-sm text-left w-full
									text-sidebar-muted hover:text-sidebar-foreground hover:bg-sidebar-accent/50 transition-colors"
								onclick={() => onConnectServer(server)}
								title="Connect to {server.name}"
							>
								<span class="w-1.5 h-1.5 rounded-full flex-shrink-0
									{server.enabled ? 'bg-success' : 'bg-text-muted'}"></span>
								<span class="truncate">{server.name}</span>
								{#if server.is_local}
									<span class="text-[9px] text-sidebar-muted ml-auto">local</span>
								{/if}
							</button>
						{/each}
					</div>
				{/if}
			</div>
		</div>
	{/if}
</aside>
