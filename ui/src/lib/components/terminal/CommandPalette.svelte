<script lang="ts">
	import { Search } from 'lucide-svelte';
	import { isMobile } from '$lib/state/mobile.svelte.js';
	import * as quickCommandsState from '$lib/state/quickCommands.svelte.js';
	import * as serversState from '$lib/state/servers.svelte.js';
	import * as themeState from '$lib/state/theme.svelte.js';
	import * as terminalState from '$lib/state/terminal.svelte.js';

	interface PaletteItem {
		id: string;
		name: string;
		detail: string;
		category: 'command' | 'action';
		action: () => void;
	}

	interface Props {
		open: boolean;
		onClose: () => void;
		onInjectCommand?: (cmd: string) => void;
		onSaveProfile?: () => void;
		onClearTerminal?: () => void;
		onCopyOutput?: () => void;
		onDownloadOutput?: () => void;
		onToggleSearch?: () => void;
		onConnect?: () => void;
		onDisconnect?: () => void;
		onConnectServer?: (serverId: number, serverName: string) => void;
		serverId: number | null;
	}

	let { open, onClose, onInjectCommand, onSaveProfile, onClearTerminal, onCopyOutput, onDownloadOutput, onToggleSearch, onConnect, onDisconnect, onConnectServer, serverId }: Props = $props();

	let query = $state('');
	let selectedIndex = $state(0);
	let inputEl: HTMLInputElement | undefined = $state();
	let pendingAction: (() => void) | null = null;

	let commands = $derived(quickCommandsState.getCommands());

	// Build palette items
	let items = $derived.by(() => {
		const result: PaletteItem[] = [];

		// Quick commands
		for (const cmd of commands) {
			result.push({
				id: `cmd-${cmd.id}`,
				name: cmd.name,
				detail: cmd.command,
				category: 'command',
				action: () => { if (onInjectCommand) onInjectCommand(cmd.command); }
			});
		}

		// Server connections
		const servers = serversState.getServers();
		for (const server of servers) {
			result.push({
				id: `server-${server.id}`,
				name: `Connect: ${server.name}`,
				detail: server.hostname ?? 'local',
				category: 'action',
				action: () => { onConnectServer?.(server.id, server.name); }
			});
		}

		if (servers.length === 0) {
			result.push({
				id: 'action-add-server',
				name: 'Add Server',
				detail: 'Configure a new server',
				category: 'action',
				action: () => { window.location.href = '/servers'; }
			});
		}

		// App actions
		result.push(
			{ id: 'action-new-tab', name: 'New Tab', detail: 'Open a new terminal tab', category: 'action',
				action: () => { terminalState.createTab(null, null, 'pty'); } },
			{ id: 'action-close-tab', name: 'Close Tab', detail: 'Close the active terminal tab', category: 'action',
				action: () => { const id = terminalState.getActiveTabId(); if (id) terminalState.closeTab(id); } },
			{ id: 'action-connect', name: 'Connect', detail: 'Connect active tab to server', category: 'action',
				action: () => { onConnect?.(); } },
			{ id: 'action-disconnect', name: 'Disconnect', detail: 'Disconnect active tab', category: 'action',
				action: () => { onDisconnect?.(); } },
			{ id: 'action-clear', name: 'Clear Terminal', detail: 'Clear terminal output', category: 'action',
				action: () => { onClearTerminal?.(); } },
			{ id: 'action-copy', name: 'Copy Output', detail: 'Copy terminal output to clipboard', category: 'action',
				action: () => { onCopyOutput?.(); } },
			{ id: 'action-download', name: 'Download Output', detail: 'Download terminal output as file', category: 'action',
				action: () => { onDownloadOutput?.(); } },
			{ id: 'action-search', name: 'Search Output', detail: 'Search terminal output', category: 'action',
				action: () => { onToggleSearch?.(); } },
			{ id: 'action-save-profile', name: 'Save Layout as Profile', detail: 'Save current tabs and layout', category: 'action',
				action: () => { onSaveProfile?.(); } },
			{ id: 'action-toggle-theme', name: 'Toggle Theme', detail: 'Switch between dark and light mode', category: 'action',
				action: () => { themeState.toggleTheme(); } }
		);

		return result;
	});

	// Fuzzy filter
	let filtered = $derived.by(() => {
		if (!query.trim()) return items;
		const q = query.toLowerCase();
		return items.filter(
			(item) => item.name.toLowerCase().includes(q) || item.detail.toLowerCase().includes(q)
		);
	});

	// Reset state when palette opens
	$effect(() => {
		if (open) {
			query = '';
			selectedIndex = 0;
			pendingAction = null;
			void quickCommandsState.loadCommands();
			// Only auto-focus search input on desktop — on mobile it opens the keyboard
			if (!isMobile()) {
				requestAnimationFrame(() => inputEl?.focus());
			}
		}
	});

	// Clamp selection index
	$effect(() => {
		if (selectedIndex >= filtered.length) {
			selectedIndex = Math.max(0, filtered.length - 1);
		}
	});

	function handleKeydown(e: KeyboardEvent) {
		e.stopPropagation();
		if (e.key === 'Escape') {
			e.preventDefault();
			closePalette();
		} else if (e.key === 'ArrowDown') {
			e.preventDefault();
			if (filtered.length > 0) selectedIndex = (selectedIndex + 1) % filtered.length;
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			if (filtered.length > 0) selectedIndex = (selectedIndex - 1 + filtered.length) % filtered.length;
		} else if (e.key === 'Enter') {
			e.preventDefault();
			if (filtered[selectedIndex]) {
				selectItem(filtered[selectedIndex]);
			}
		}
	}

	function selectItem(item: PaletteItem) {
		// Store action, close palette first, then run action after DOM settles
		pendingAction = item.action;
		closePalette();
	}

	function closePalette() {
		// Close first, run pending action after a delay so DOM fully settles
		onClose();
		if (pendingAction) {
			const action = pendingAction;
			pendingAction = null;
			setTimeout(() => action(), 50);
		}
	}
</script>

<!-- Always in DOM, toggle visibility via CSS to avoid WebKitGTK layout corruption -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="fixed inset-0 bg-black/40 z-40 transition-opacity duration-100
		{open ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none'}"
	onclick={(e) => { e.stopPropagation(); closePalette(); }}
	onkeydown={() => {}}
	role="presentation"
></div>

<div
	class="fixed z-50 bg-surface border border-border shadow-lg flex flex-col overflow-hidden transition-opacity duration-100
		{isMobile()
			? 'bottom-0 inset-x-0 rounded-t-lg max-h-[60vh]'
			: 'top-[20%] left-1/2 -translate-x-1/2 w-full max-w-md rounded-lg'
		}
		{open ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none'}"
	role="dialog"
	aria-label="Command palette"
	aria-hidden={!open}
	tabindex="-1"
	onkeydown={handleKeydown}
>
	<!-- Search input -->
	<div class="flex items-center gap-2 px-3 py-2 border-b border-border">
		<Search class="w-4 h-4 text-text-muted flex-shrink-0" />
		<input
			bind:this={inputEl}
			bind:value={query}
			type="text"
			class="flex-1 bg-transparent text-text-primary text-sm placeholder:text-text-muted outline-none"
			placeholder="Type a command or action..."
		/>
	</div>

	<!-- Results list -->
	<div class="overflow-y-auto max-h-[50vh] py-1">
		{#if !open}
			<!-- empty when hidden -->
		{:else if filtered.length === 0}
			<div class="px-3 py-4 text-center text-text-muted text-sm">No matches found</div>
		{:else}
			{#each filtered as item, i (item.id)}
				<button
					class="w-full text-left px-3 py-2 flex items-center gap-3 text-sm transition-colors
						{i === selectedIndex ? 'bg-accent/10 text-text-primary' : 'text-text-secondary hover:bg-surface-raised'}"
					onclick={(e) => { e.stopPropagation(); selectItem(item); }}
					role="option"
					aria-selected={i === selectedIndex}
				>
					<span class="flex-1 truncate">
						<span class="font-medium">{item.name}</span>
						{#if item.detail && item.category === 'command'}
							<span class="text-text-muted ml-2 font-mono text-xs">{item.detail}</span>
						{/if}
					</span>
					<span class="text-[10px] uppercase text-text-muted flex-shrink-0">
						{item.category === 'command' ? 'cmd' : 'action'}
					</span>
				</button>
			{/each}
		{/if}
	</div>
</div>
