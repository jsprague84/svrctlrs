<script lang="ts">
	import { onMount } from 'svelte';
	import { Terminal as TerminalIcon, Settings2, Trash2, ClipboardCopy, Download } from 'lucide-svelte';
	import TerminalPane from '$lib/components/terminal/TerminalPane.svelte';
	import TerminalTabs from '$lib/components/terminal/TerminalTabs.svelte';
	import SplitView from '$lib/components/terminal/SplitView.svelte';
	import CommandInput from '$lib/components/terminal/CommandInput.svelte';
	import ConnectionBadge from '$lib/components/terminal/ConnectionBadge.svelte';
	import TerminalPrefsPanel from '$lib/components/terminal/TerminalPrefsPanel.svelte';
	import CommandPalette from '$lib/components/terminal/CommandPalette.svelte';
	import ProfileManager from '$lib/components/terminal/ProfileManager.svelte';
	import MobileStatusBar from '$lib/components/terminal/MobileStatusBar.svelte';
	import ExtraKeysRow from '$lib/components/terminal/ExtraKeysRow.svelte';
	import * as terminalState from '$lib/state/terminal.svelte.js';
	import * as serversState from '$lib/state/servers.svelte.js';
	import * as profilesState from '$lib/state/profiles.svelte.js';
	import { isMobile } from '$lib/state/mobile.svelte.js';
	import { isKeyboardVisible, initKeyboardDetection, destroyKeyboardDetection } from '$lib/platform/keyboard.svelte.js';
	import type { TerminalMode, ConnectionStatus } from '$lib/types/index.js';

	let selectedServerId = $state<number | null>(null);
	let selectedMode = $state<TerminalMode>('pty');
	let paneRefs = $state<Record<string, ReturnType<typeof TerminalPane>>>({});
	let searchOpen = $state(false);
	let searchTerm = $state('');
	let prefsOpen = $state(false);
	let paletteOpen = $state(false);
	let profileSaveOpen = $state(false);
	let showGestureHint = $state(false);

	let servers = $derived(serversState.getServers());
	let serversLoading = $derived(serversState.isLoading());
	let serversError = $derived(serversState.getError());
	let tabs = $derived(terminalState.getTabs());
	let activeTabId = $derived(terminalState.getActiveTabId());
	let activeTab = $derived(terminalState.getActiveTab());
	let layout = $derived(terminalState.getLayout());
	let visibleTabs = $derived(terminalState.getVisibleTabs());
	let pendingAutoConnect = $derived(terminalState.getPendingAutoConnect());

	// Auto-connect when a pending tab's pane is ready
	$effect(() => {
		if (pendingAutoConnect && paneRefs[pendingAutoConnect]) {
			const tabId = pendingAutoConnect;
			terminalState.clearPendingAutoConnect();
			requestAnimationFrame(() => paneRefs[tabId]?.connect());
		}
	});

	// Warn before unloading if any tab is connected
	$effect(() => {
		const hasConnected = tabs.some((t) => t.status === 'connected');
		const handler = (e: BeforeUnloadEvent) => { e.preventDefault(); };
		if (hasConnected) {
			window.addEventListener('beforeunload', handler);
			return () => window.removeEventListener('beforeunload', handler);
		}
	});

	onMount(() => {
		// Initialize keyboard detection for mobile extra keys row
		if (isMobile()) initKeyboardDetection();

		// Show gesture hint on first mobile visit
		if (isMobile() && !localStorage.getItem('svrctlrs-gesture-hint-seen')) {
			showGestureHint = true;
			setTimeout(() => {
				showGestureHint = false;
				localStorage.setItem('svrctlrs-gesture-hint-seen', 'true');
			}, 5000);
		}

		// Create initial tab — either from default profile or empty
		if (tabs.length === 0) {
			const allProfiles = profilesState.getProfiles();
			const defaultProfile = allProfiles.find(p => p.is_default);
			if (defaultProfile && defaultProfile.pane_configs) {
				const serverNames: Record<number, string> = {};
				for (const s of serversState.getServers()) {
					serverNames[s.id] = s.name;
				}
				terminalState.applyProfile(
					defaultProfile.layout as import('$lib/types/index.js').LayoutMode,
					defaultProfile.pane_configs,
					serverNames
				);
			} else {
				terminalState.createTab(null, null, 'pty');
			}
		}

		// Focus the terminal pane for a given tab id
		function focusTerminal(tabId: string | null) {
			if (tabId) {
				requestAnimationFrame(() => paneRefs[tabId]?.focus());
			}
		}

		// Keyboard shortcuts
		function handleKeydown(e: KeyboardEvent) {
			// Ctrl+K / Cmd+K — command palette
			if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
				e.preventDefault();
				paletteOpen = !paletteOpen;
			}
			// Ctrl+Shift+T — new tab
			if (e.ctrlKey && e.shiftKey && e.key === 'T') {
				e.preventDefault();
				terminalState.createTab(null, null, selectedMode);
			}
			// Ctrl+Shift+W — close tab
			if (e.ctrlKey && e.shiftKey && e.key === 'W') {
				e.preventDefault();
				if (activeTabId) terminalState.closeTab(activeTabId);
			}
			// Ctrl+Tab — next tab (cycles all tabs, rearranges slots)
			if (e.ctrlKey && e.key === 'Tab' && !e.shiftKey) {
				e.preventDefault();
				terminalState.nextTab();
				focusTerminal(terminalState.getActiveTabId());
			}
			// Ctrl+Shift+Tab — prev tab
			if (e.ctrlKey && e.shiftKey && e.key === 'Tab') {
				e.preventDefault();
				terminalState.prevTab();
				focusTerminal(terminalState.getActiveTabId());
			}
			// Alt+1/2/3/4 — focus pane by slot number (no rearranging)
			if (e.altKey && !e.ctrlKey && !e.shiftKey && e.key >= '1' && e.key <= '4') {
				e.preventDefault();
				const slot = parseInt(e.key) - 1;
				const tabId = terminalState.focusSlot(slot);
				focusTerminal(tabId);
			}
			// Alt+] — next visible pane (no rearranging)
			if (e.altKey && !e.ctrlKey && e.key === ']') {
				e.preventDefault();
				focusTerminal(terminalState.focusNextPane());
			}
			// Alt+[ — prev visible pane (no rearranging)
			if (e.altKey && !e.ctrlKey && e.key === '[') {
				e.preventDefault();
				focusTerminal(terminalState.focusPrevPane());
			}
			// Alt+Arrow — spatial navigation between visible panes
			if (e.altKey && !e.ctrlKey && !e.shiftKey) {
				const dirMap: Record<string, 'left' | 'right' | 'up' | 'down'> = {
					ArrowLeft: 'left', ArrowRight: 'right',
					ArrowUp: 'up', ArrowDown: 'down'
				};
				const dir = dirMap[e.key];
				if (dir) {
					e.preventDefault();
					focusTerminal(terminalState.focusDirection(dir));
				}
			}
			// Ctrl+\ — toggle split
			if (e.ctrlKey && e.key === '\\') {
				e.preventDefault();
				const modes: Array<'single' | 'split-h' | 'split-v' | 'quad'> = ['single', 'split-h', 'split-v', 'quad'];
				const idx = modes.indexOf(layout);
				terminalState.setLayout(modes[(idx + 1) % modes.length]);
			}
			// Ctrl+Shift+F — search
			if (e.ctrlKey && e.shiftKey && e.key === 'F') {
				e.preventDefault();
				searchOpen = !searchOpen;
				if (!searchOpen) {
					searchTerm = '';
					if (activeTabId) paneRefs[activeTabId]?.clearSearch();
				}
			}
		}

		window.addEventListener('keydown', handleKeydown, true);
		return () => {
			window.removeEventListener('keydown', handleKeydown, true);
			destroyKeyboardDetection();
		};
	});

	function handleConnect() {
		if (!activeTabId || selectedServerId === null) return;
		const server = servers.find((s) => s.id === selectedServerId);
		terminalState.updateTabServer(activeTabId, selectedServerId, server?.name ?? null);
		terminalState.updateTabMode(activeTabId, selectedMode);
		paneRefs[activeTabId]?.connect();
	}

	function handleDisconnect() {
		if (!activeTabId) return;
		paneRefs[activeTabId]?.disconnect();
	}

	function handleStatusChange(tabId: string, status: ConnectionStatus) {
		terminalState.updateTabStatus(tabId, status);
	}

	function handleNewTab() {
		terminalState.createTab(null, null, selectedMode);
	}

	function handleCmdSubmit(cmd: string) {
		if (!activeTabId) return;
		paneRefs[activeTabId]?.executeCommand(cmd);
	}

	function handleSearch() {
		if (!activeTabId || !searchTerm) return;
		paneRefs[activeTabId]?.search(searchTerm);
	}

	function handleSearchNext() {
		if (!activeTabId || !searchTerm) return;
		paneRefs[activeTabId]?.searchNext(searchTerm);
	}

	function handleSearchPrev() {
		if (!activeTabId || !searchTerm) return;
		paneRefs[activeTabId]?.searchPrevious(searchTerm);
	}
</script>

<div class="flex flex-col h-screen">
	<!-- Mobile: minimal status bar -->
	{#if isMobile()}
		<MobileStatusBar
			serverName={activeTab?.serverName ?? null}
			connectionStatus={activeTab?.status ?? 'disconnected'}
			{tabs}
			{activeTabId}
		/>
	{:else}
	<!-- Desktop: full toolbar -->
	<div class="flex flex-wrap items-center gap-2 md:gap-3 md:flex-nowrap px-2 md:px-4 py-2 bg-surface border-b border-border">
		<TerminalIcon class="hidden md:block w-5 h-5 text-accent" />
		<h1 class="hidden md:block text-sm font-semibold text-text-primary">SvrCtlRS</h1>

		<select
			class="flex-1 min-w-0 ml-0 md:ml-4 px-2 py-2 md:py-1 text-sm bg-input border border-border rounded-sm text-text-primary focus:ring-1 focus:ring-ring focus:outline-none"
			bind:value={selectedServerId}
		>
			{#if serversLoading}
				<option value={null}>Loading servers...</option>
			{:else if serversError}
				<option value={null}>Failed to load servers</option>
			{:else}
				<option value={null}>Select server...</option>
				{#each servers as server}
					<option value={server.id}>{server.name}{server.hostname ? ` (${server.hostname})` : ''}</option>
				{/each}
			{/if}
		</select>

		<select
			class="min-w-0 px-2 py-2 md:py-1 text-sm bg-input border border-border rounded-sm text-text-primary focus:ring-1 focus:ring-ring focus:outline-none"
			bind:value={selectedMode}
		>
			<option value="pty">Interactive (PTY)</option>
			<option value="cmd">Command (CMD)</option>
		</select>

		{#if activeTab?.status !== 'connected'}
			<button
				class="px-3 py-1 text-sm bg-primary text-primary-foreground rounded-sm hover:opacity-90 disabled:opacity-50"
				disabled={selectedServerId === null}
				onclick={handleConnect}
			>
				Connect
			</button>
		{:else}
			<button
				class="px-3 py-1 text-sm bg-destructive text-destructive-foreground rounded-sm hover:opacity-90"
				onclick={handleDisconnect}
			>
				Disconnect
			</button>
		{/if}

		<div class="w-full md:w-auto md:ml-auto flex items-center gap-1 md:gap-2 justify-end">
			{#if activeTab}
				<span aria-live="polite">
					<ConnectionBadge status={activeTab.status} />
				</span>
			{/if}
			<button class="p-2 md:p-1 text-text-muted hover:text-text-primary" onclick={() => { if (activeTabId) paneRefs[activeTabId]?.clear(); }} title="Clear" aria-label="Clear terminal">
				<Trash2 class="w-4 h-4" />
			</button>
			<button class="p-2 md:p-1 text-text-muted hover:text-text-primary" onclick={() => { if (activeTabId) paneRefs[activeTabId]?.copyOutput(); }} title="Copy" aria-label="Copy output">
				<ClipboardCopy class="w-4 h-4" />
			</button>
			<button class="p-2 md:p-1 text-text-muted hover:text-text-primary" onclick={() => { if (activeTabId) paneRefs[activeTabId]?.downloadOutput(); }} title="Download" aria-label="Download output">
				<Download class="w-4 h-4" />
			</button>
			<button class="p-2 md:p-1 text-text-muted hover:text-text-primary" class:text-accent={prefsOpen} onclick={() => prefsOpen = !prefsOpen} title="Terminal Preferences" aria-label="Terminal preferences">
				<Settings2 class="w-4 h-4" />
			</button>
		</div>
	</div>
	{/if}

	<!-- Search bar (desktop only) -->
	{#if searchOpen && !isMobile()}
		<div class="flex flex-wrap items-center gap-2 px-2 md:px-4 py-1.5 bg-surface-raised border-b border-border">
			<input
				type="text"
				bind:value={searchTerm}
				oninput={handleSearch}
				class="px-2 py-1 text-sm bg-input border border-border rounded-sm text-text-primary placeholder:text-text-muted w-full md:w-64"
				placeholder="Search terminal output..."
			/>
			<div class="flex gap-1 mt-1 md:mt-0 md:gap-2">
				<button class="px-2 py-1 text-xs text-text-muted hover:text-text-primary" onclick={handleSearchPrev}>Prev</button>
				<button class="px-2 py-1 text-xs text-text-muted hover:text-text-primary" onclick={handleSearchNext}>Next</button>
				<button class="px-2 py-1 text-xs text-text-muted hover:text-text-primary" onclick={() => { searchOpen = false; searchTerm = ''; if (activeTabId) paneRefs[activeTabId]?.clearSearch(); }}>Close</button>
			</div>
		</div>
	{/if}

	<!-- Tab bar (desktop only) -->
	{#if !isMobile()}
	<TerminalTabs
		{tabs}
		{activeTabId}
		{layout}
		onSelectTab={(id) => terminalState.setActiveTab(id)}
		onCloseTab={(id) => {
			const tab = tabs.find((t) => t.id === id);
			if (tab?.status === 'connected' && !confirm('Terminal is connected. Close anyway?')) return;
			paneRefs[id]?.disconnect();
			delete paneRefs[id];
			terminalState.closeTab(id);
		}}
		onNewTab={handleNewTab}
		onSetLayout={(mode) => terminalState.setLayout(mode)}
	/>
	{/if}

	<!-- CMD input bar (desktop only) -->
	{#if !isMobile() && activeTab?.mode === 'cmd' && activeTab?.status === 'connected'}
		<CommandInput
			onSubmit={handleCmdSubmit}
			onHistoryUp={() => activeTabId ? paneRefs[activeTabId]?.getPreviousCommand() : null}
			onHistoryDown={() => activeTabId ? paneRefs[activeTabId]?.getNextCommand() ?? '' : ''}
		/>
	{/if}

	<!-- Terminal panes -->
	<div class="flex-1 min-h-0 relative" style:padding-bottom={isMobile() && !isKeyboardVisible() ? 'env(safe-area-inset-bottom)' : '0'}>
		<SplitView {layout} slotCount={visibleTabs.length}>
			{#each tabs as tab (tab.id)}
				<div
					class="min-h-0 min-w-0 relative"
					class:hidden={tab.slot === null}
					class:ring-1={tab.id === activeTabId && layout !== 'single' && visibleTabs.length > 1}
					class:ring-accent={tab.id === activeTabId && layout !== 'single' && visibleTabs.length > 1}
				>
					{#if tab.slot !== null && layout !== 'single' && visibleTabs.length > 1}
						<div class="absolute top-0 left-1 z-10 px-1.5 py-0 text-[10px] font-mono rounded-b-sm {tab.id === activeTabId ? 'bg-accent text-background' : 'bg-surface-raised/80 text-text-muted'}">
							{tab.slot + 1}
						</div>
					{/if}
					<TerminalPane
						bind:this={paneRefs[tab.id]}
						tabId={tab.id}
						serverId={tab.serverId}
						mode={tab.mode}
						active={tab.slot !== null}
						onStatusChange={handleStatusChange}
						onOpenPalette={() => paletteOpen = true}
					/>
				</div>
			{/each}
		</SplitView>
		<!-- Gesture hint (mobile, first visit only) -->
		{#if showGestureHint}
			<div class="absolute bottom-2 left-0 right-0 text-center text-[10px] text-text-muted transition-opacity duration-1000 z-10 pointer-events-none"
				class:opacity-0={!showGestureHint}>
				double-tap for commands · swipe ← → tabs
			</div>
		{/if}

		<!-- Extra keys row (mobile only, when keyboard visible) -->
		{#if isMobile() && isKeyboardVisible()}
			<ExtraKeysRow onKeyPress={(seq) => {
				if (activeTabId && paneRefs[activeTabId]) {
					paneRefs[activeTabId].injectInput(seq);
				}
			}} />
		{/if}
		<TerminalPrefsPanel open={prefsOpen} onClose={() => prefsOpen = false} />
		<ProfileManager
			open={profileSaveOpen}
			onClose={() => profileSaveOpen = false}
			serverNames={Object.fromEntries(serversState.getServers().map(s => [s.id, s.name]))}
		/>
		<CommandPalette
			open={paletteOpen}
			onClose={() => paletteOpen = false}
			onInjectCommand={(cmd) => {
				if (activeTabId && paneRefs[activeTabId]) {
					paneRefs[activeTabId].injectInput(cmd + '\n');
				}
			}}
			onSaveProfile={() => profileSaveOpen = true}
			onClearTerminal={() => { if (activeTabId) paneRefs[activeTabId]?.clear(); }}
			onCopyOutput={() => { if (activeTabId) paneRefs[activeTabId]?.copyOutput(); }}
			onDownloadOutput={() => { if (activeTabId) paneRefs[activeTabId]?.downloadOutput(); }}
			onToggleSearch={() => { searchOpen = !searchOpen; }}
			onConnect={handleConnect}
			onDisconnect={handleDisconnect}
			onConnectServer={(id, name) => {
				const tab = terminalState.createTab(id, name, 'pty');
				if (tab) terminalState.setPendingAutoConnect(tab.id);
			}}
			serverId={activeTab?.serverId ?? null}
		/>
	</div>
</div>
