<script lang="ts">
	import { page } from '$app/state';
	import { base } from '$app/paths';
	import { Terminal, Server, KeyRound, Settings, PanelLeftClose, PanelLeftOpen, LogOut, Sun, Moon, X } from 'lucide-svelte';
	import type { Server as ServerType } from '$lib/types/index.js';
	import { logout } from '$lib/api/client.js';
	import * as themeState from '$lib/state/theme.svelte.js';

	interface Props {
		servers: ServerType[];
		loading?: boolean;
		error?: string | null;
		collapsed?: boolean;
		mobileOpen?: boolean;
		onToggle: () => void;
		onMobileClose?: () => void;
		onConnectServer: (server: ServerType) => void;
	}

	let { servers, loading = false, error = null, collapsed = false, mobileOpen = false, onToggle, onMobileClose, onConnectServer }: Props = $props();

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

	function handleNavClick() {
		onMobileClose?.();
	}

	function handleServerConnect(server: ServerType) {
		onMobileClose?.();
		onConnectServer(server);
	}
</script>

<aside
	class="flex-col bg-sidebar border-r border-sidebar-border transition-all duration-200
		{mobileOpen
			? 'fixed inset-y-0 left-0 z-40 w-64 flex translate-x-0 pl-[env(safe-area-inset-left)]'
			: 'hidden md:flex'} {!mobileOpen && collapsed ? 'w-12' : !mobileOpen ? 'w-56' : ''}"
	aria-label="Sidebar"
>
	<!-- Logo / Toggle -->
	<div class="flex items-center justify-between py-3 border-b border-sidebar-border {mobileOpen ? 'pl-12 pr-3' : 'px-3'}">
		{#if mobileOpen || !collapsed}
			<span class="text-sm font-bold text-sidebar-foreground tracking-tight">SvrCtlRS</span>
		{/if}
		{#if mobileOpen}
			<button
				class="p-1 text-sidebar-muted hover:text-sidebar-foreground rounded-sm"
				onclick={onMobileClose}
				title="Close sidebar"
				aria-label="Close sidebar"
			>
				<X class="w-4 h-4" />
			</button>
		{:else}
			<button
				class="p-1 text-sidebar-muted hover:text-sidebar-foreground rounded-sm"
				onclick={onToggle}
				title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
				aria-expanded={!collapsed}
				aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
			>
				{#if collapsed}
					<PanelLeftOpen class="w-4 h-4" />
				{:else}
					<PanelLeftClose class="w-4 h-4" />
				{/if}
			</button>
		{/if}
	</div>

	<!-- Navigation -->
	<nav class="flex flex-col gap-0.5 px-2 py-2" aria-label="Main navigation">
		{#each navItems as item}
			<a
				href={item.href}
				class="flex items-center gap-2.5 px-2 py-2.5 md:py-1.5 rounded-sm text-sm transition-colors
					{isActive(item.href)
						? 'bg-sidebar-accent text-sidebar-foreground'
						: 'text-sidebar-muted hover:text-sidebar-foreground hover:bg-sidebar-accent/50'}"
				title={collapsed && !mobileOpen ? item.label : undefined}
				aria-current={isActive(item.href) ? 'page' : undefined}
				onclick={handleNavClick}
			>
				<item.icon class="w-4 h-4 flex-shrink-0" />
				{#if mobileOpen || !collapsed}
					<span>{item.label}</span>
				{/if}
			</a>
		{/each}
	</nav>

	<!-- Server list (not collapsed, or mobile open) -->
	{#if mobileOpen || !collapsed}
		<div class="flex-1 overflow-y-auto border-t border-sidebar-border" aria-label="Servers">
			<div class="px-3 py-2">
				<h3 class="text-[10px] uppercase tracking-wider text-sidebar-muted font-semibold mb-1.5">Servers</h3>
				{#if loading}
					<p class="text-xs text-sidebar-muted italic">Loading...</p>
				{:else if error}
					<p class="text-xs text-error italic">{error}</p>
				{:else if servers.length === 0}
					<p class="text-xs text-sidebar-muted italic">No servers configured</p>
				{:else}
					<div class="flex flex-col gap-0.5">
						{#each servers as server}
							<button
								class="flex items-center gap-2 px-2 py-2.5 md:py-1.5 text-xs rounded-sm text-left w-full
									text-sidebar-muted hover:text-sidebar-foreground hover:bg-sidebar-accent/50 transition-colors"
								onclick={() => handleServerConnect(server)}
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

	<!-- Theme toggle + Logout -->
	<div class="mt-auto border-t border-sidebar-border px-2 py-2 flex flex-col gap-0.5">
		<button
			class="flex items-center gap-2.5 px-2 py-2.5 md:py-1.5 rounded-sm text-sm transition-colors w-full
				text-sidebar-muted hover:text-sidebar-foreground hover:bg-sidebar-accent/50"
			onclick={themeState.toggleTheme}
			title={themeState.isDark() ? 'Switch to light mode' : 'Switch to dark mode'}
			aria-label={themeState.isDark() ? 'Switch to light mode' : 'Switch to dark mode'}
		>
			{#if themeState.isDark()}
				<Sun class="w-4 h-4 flex-shrink-0" />
			{:else}
				<Moon class="w-4 h-4 flex-shrink-0" />
			{/if}
			{#if mobileOpen || !collapsed}
				<span>{themeState.isDark() ? 'Light Mode' : 'Dark Mode'}</span>
			{/if}
		</button>
		<button
			class="flex items-center gap-2.5 px-2 py-2.5 md:py-1.5 rounded-sm text-sm transition-colors w-full
				text-sidebar-muted hover:text-sidebar-foreground hover:bg-sidebar-accent/50"
			onclick={logout}
			title="Logout"
			aria-label="Logout"
		>
			<LogOut class="w-4 h-4 flex-shrink-0" />
			{#if mobileOpen || !collapsed}
				<span>Logout</span>
			{/if}
		</button>
	</div>
</aside>
