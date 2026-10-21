<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		Terminal as TerminalIcon,
		Settings2,
		Trash2,
		ClipboardCopy,
		Download,
		History,
		FolderOpen,
		Server as ServerIcon
	} from '@lucide/svelte';
	import TerminalPane from './TerminalPane.svelte';
	import TerminalTabs from './TerminalTabs.svelte';
	import SplitView from './SplitView.svelte';
	import ConnectionBadge from './ConnectionBadge.svelte';
	import TerminalPrefsPanel from './TerminalPrefsPanel.svelte';
	import SessionsPanel from './SessionsPanel.svelte';
	import FilesPanel from './FilesPanel.svelte';
	import FileViewer from './FileViewer.svelte';
	import CommandPalette from './CommandPalette.svelte';
	import SnippetVarsDialog from './SnippetVarsDialog.svelte';
	import {
		fillSnippet,
		promptedVariables,
		serverVariables,
		type SnippetRequest
	} from '#lib/utils/snippet.js';
	import ProfileManager from './ProfileManager.svelte';
	import MobileStatusBar from './MobileStatusBar.svelte';
	import MobileSheet, { type SheetView } from './MobileSheet.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import ExtraKeysRow from './ExtraKeysRow.svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import { serverStatus } from '#lib/utils/serverStatus.js';
	import * as terminalState from '#lib/state/terminal.svelte.js';
	import * as serversState from '#lib/state/servers.svelte.js';
	import * as profilesState from '#lib/state/profiles.svelte.js';
	import * as sessionsState from '#lib/state/sessions.svelte.js';
	import * as filesState from '#lib/state/files.svelte.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { isMobile } from '#lib/state/mobile.svelte.js';
	import * as terminalPrefs from '#lib/state/terminalPrefs.svelte.js';
	import {
		isLeaderChord,
		leaderAction,
		LEADER_BINDINGS,
		LEADER_KEY_LABELS
	} from '#lib/utils/leader.js';
	import type { ConnectionStatus, LayoutMode, LiveSession } from '#lib/types/index.js';

	interface Props {
		visible: boolean;
	}

	let { visible }: Props = $props();

	/** The toolbar's picker: the active tab's server. Picking another one and pressing Connect
	 *  points the tab there; switching tabs shows that tab's server again. */
	let selectedServerId = $derived<number | null>(terminalState.getActiveTab()?.serverId ?? null);
	let paneRefs = $state<Record<string, ReturnType<typeof TerminalPane>>>({});
	let extraKeysRow = $state<ReturnType<typeof ExtraKeysRow> | null>(null);
	let searchOpen = $state(false);
	let searchTerm = $state('');
	let prefsOpen = $state(false);
	/** Desktop side panel listing the shells still running on the server */
	let sessionsOpen = $state(false);
	/** Desktop side panel with the active server's files over SFTP */
	let filesOpen = $state(false);
	/** Phone bottom sheet: tab list, server picker or the sessions list */
	let sheetOpen = $state(false);
	let sheetView = $state<SheetView>('tabs');

	function openSheet(view: SheetView) {
		sheetView = view;
		sheetOpen = true;
	}

	/** Which server-side sessions the tabs here hold (sessionStorage is not reactive, so it is
	 *  re-read whenever the list is refreshed). */
	let heldSessions = new SvelteMap<string, string>();
	let liveSessions = $derived(sessionsState.getSessions());
	let resumableCount = $derived(liveSessions.filter((s) => !heldSessions.has(s.id)).length);
	let sessionsVisible = $derived(isMobile() ? sheetOpen && sheetView === 'sessions' : sessionsOpen);

	function rereadHeldSessions() {
		const held = terminalState.heldSessions();
		for (const id of [...heldSessions.keys()]) if (!held.has(id)) heldSessions.delete(id);
		for (const [id, tabId] of held) heldSessions.set(id, tabId);
	}

	async function refreshSessions() {
		rereadHeldSessions();
		await sessionsState.loadSessions();
		rereadHeldSessions();
	}

	function openSessions() {
		if (isMobile()) openSheet('sessions');
		else {
			prefsOpen = false;
			filesOpen = false;
			sessionsOpen = true;
		}
	}

	/** Browse the active tab's server; the panel keeps the last directory per server */
	function openFiles() {
		const tab = terminalState.getActiveTab();
		if (!tab?.serverId) {
			toast.warning('Connect a tab to a server first');
			return;
		}
		filesState.browse(tab.serverId, tab.serverName);
		if (isMobile()) openSheet('files');
		else {
			prefsOpen = false;
			sessionsOpen = false;
			filesOpen = true;
		}
	}

	/** "Edit in terminal": a new tab on that server opens the file in its own editor */
	function editInTerminal(serverId: number, serverName: string | null, path: string) {
		const id = terminalState.openToEdit(serverId, serverName ?? `Server ${serverId}`, path);
		if (!id) {
			toast.error('Tab limit reached — close a terminal first');
			return;
		}
		sheetOpen = false;
		filesOpen = false;
	}

	// The panel follows the active tab: switching tabs switches servers
	$effect(() => {
		const tab = activeTab;
		if (!filesOpen || !tab?.serverId) return;
		if (untrack(() => filesState.getServerId()) !== tab.serverId) {
			filesState.browse(tab.serverId, tab.serverName);
		}
	});

	// One fetch when the terminal page first shows, so the badge can say a shell is waiting.
	// Once only: a failure must not turn into a retry loop.
	let sessionsFetchedOnce = false;
	$effect(() => {
		if (visible && !sessionsFetchedOnce) {
			sessionsFetchedOnce = true;
			untrack(() => refreshSessions());
		}
	});

	// Fresh data whenever the list is shown, and every so often while it stays open. The
	// refresh reads and writes `heldSessions`; untracked so it does not retrigger itself.
	$effect(() => {
		if (!sessionsVisible) return;
		untrack(() => refreshSessions());
		const t = setInterval(refreshSessions, 15_000);
		return () => clearInterval(t);
	});

	/** Open a tab attached to a shell that no tab here holds */
	function resumeSession(session: LiveSession) {
		const id = terminalState.resumeSession(session.id, session.server_id, session.server_name);
		if (!id) {
			toast.error('Too many terminals open — close one to free a slot');
			if (isMobile()) openSheet('tabs');
			return;
		}
		heldSessions.set(session.id, id);
		sessionsOpen = false;
		sheetOpen = false;
		focusTerminal(id);
	}

	/** End a shell for good, attached or not */
	async function endSession(session: LiveSession) {
		const ok = await confirmDialog({
			title: `End the shell on ${session.server_name}?`,
			message: 'Whatever is running in it is terminated. This cannot be undone.',
			confirmLabel: 'End session',
			destructive: true
		});
		if (!ok) return;
		try {
			await sessionsState.endSession(session.id);
			toast.success(`Ended the shell on ${session.server_name}`);
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to end the session'));
			refreshSessions();
		}
	}

	function goToSessionTab(tabId: string) {
		terminalState.setActiveTab(tabId);
		sessionsOpen = false;
		sheetOpen = false;
		focusTerminal(tabId);
	}

	/** Connect a (reused or new) tab to a server and focus it */
	function connectTo(server: { id: number; name: string }) {
		const id = terminalState.connectToServer(server.id, server.name);
		if (!id) {
			// Dead end otherwise: on a phone the only way to close a terminal is this list, so
			// take the user straight to it instead of a toast that points nowhere.
			toast.error('Too many terminals open — close one to free a slot');
			openSheet('tabs');
			return;
		}
		sheetOpen = false;
		focusTerminal(id);
	}
	/** Show the tab already on this server, or open one */
	function openShellOn(server: { id: number; name: string }) {
		const existing = tabs.find((t) => t.serverId === server.id);
		if (existing) {
			terminalState.setActiveTab(existing.id);
			sheetOpen = false;
			focusTerminal(existing.id);
		} else {
			connectTo(server);
		}
	}

	// Home-screen shortcuts and links (see routes/ui/manifest.rs on the server):
	// /?connect=<server id> shows a shell on that server, /?open=servers the server picker.
	// The parameter is dropped once handled, so a reload does not repeat it.
	$effect(() => {
		if (!visible) return;
		const params = page.url.searchParams;
		const connect = params.get('connect');
		const open = params.get('open');
		if (connect === null && open === null) return;
		if (connect !== null) {
			// Wait for the first server list; an empty one before that means "not yet"
			if (!serversState.hasLoaded()) return;
			const server = servers.find((s) => s.id === Number(connect) && s.enabled);
			if (server) untrack(() => openShellOn(server));
			else toast.error('That server is gone or disabled');
		} else if (open === 'servers') {
			untrack(() => {
				if (isMobile()) openSheet('servers');
				else paletteOpen = true;
			});
		}
		const query = [...params.entries()]
			.filter(([k]) => k !== 'connect' && k !== 'open')
			.map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(v)}`)
			.join('&');
		goto(`${resolve('/')}${query ? `?${query}` : ''}`, { replace: true, shallow: true });
	});

	let paletteOpen = $state(false);
	let profileSaveOpen = $state(false);
	let showGestureHint = $state(false);
	/** Leader armed: the next key is a command; the overlay lists them. */
	let leaderArmed = $state(false);
	let leaderHelpPinned = $state(false);
	let leaderTimer: ReturnType<typeof setTimeout> | null = null;
	const LEADER_TIMEOUT = 3000;
	/** Where `` ` `` goes back to. */
	let lastActiveTabId: string | null = null;
	let currentTabIdSeen: string | null = null;
	let mobileViewHeight = $state(0);
	let mobileKeyboardOpen = $state(false);

	let servers = $derived(serversState.getServers());
	let serversLoading = $derived(serversState.isLoading());
	let serversError = $derived(serversState.getError());
	let tabs = $derived(terminalState.getTabs());
	let activeTabId = $derived(terminalState.getActiveTabId());
	let activeTab = $derived(terminalState.getActiveTab());
	let layout = $derived(terminalState.getLayout());
	let visibleTabs = $derived(terminalState.getVisibleTabs());
	/** Nothing to show in the pane area: no tab, or one tab that has no server yet */
	let showEmptyState = $derived(
		!activeTab || (activeTab.serverId === null && visibleTabs.length <= 1)
	);
	const EMPTY_STATE_SERVERS = 8;
	let enabledServers = $derived(servers.filter((s) => s.enabled));
	let pendingAutoConnect = $derived(terminalState.getPendingAutoConnect());

	// Remember the previously active tab for the leader's "last tab" (`)
	$effect(() => {
		const id = activeTabId;
		if (id !== currentTabIdSeen) {
			lastActiveTabId = currentTabIdSeen;
			currentTabIdSeen = id;
		}
	});

	// Auto-connect when a pending tab's pane is ready
	$effect(() => {
		if (pendingAutoConnect && paneRefs[pendingAutoConnect]) {
			const tabId = pendingAutoConnect;
			terminalState.clearPendingAutoConnect();
			requestAnimationFrame(() => paneRefs[tabId]?.connect());
		}
	});

	// Toasts sit just above the extra-keys row on the phone (Toast.svelte reads the variable)
	let extraKeysHeight = $state(0);
	$effect(() => {
		if (!visible || !isMobile() || extraKeysHeight <= 0) return;
		const root = document.documentElement;
		root.style.setProperty('--toast-offset', `${extraKeysHeight}px`);
		return () => root.style.removeProperty('--toast-offset');
	});

	// Refit terminals when view becomes visible (returning from another page)
	$effect(() => {
		if (visible && activeTabId) {
			requestAnimationFrame(() => {
				for (const ref of Object.values(paneRefs)) {
					ref?.fit();
				}
			});
		}
	});

	onMount(() => {
		// Show gesture hint on first mobile visit
		if (isMobile() && !localStorage.getItem('svrctlrs-gesture-hint-seen')) {
			showGestureHint = true;
			setTimeout(() => {
				showGestureHint = false;
				localStorage.setItem('svrctlrs-gesture-hint-seen', 'true');
			}, 5000);
		}

		// Soft keyboard tracking. With `interactive-widget=resizes-content` (Chrome) the layout
		// viewport — and `h-dvh` — already shrink with the keyboard; the explicit pixel height is
		// only for browsers that merely shrink the *visual* viewport (iOS). The baseline is the
		// height while nothing is focused, so rotation and pinch-zoom do not read as a keyboard.
		let updateViewHeight: (() => void) | null = null;
		if (isMobile()) {
			let baseline = window.innerHeight;
			updateViewHeight = () => {
				const vv = window.visualViewport;
				if (vv && vv.scale > 1.01) return; // pinch-zoomed: leave the layout alone
				const visible = vv ? vv.height * vv.scale : window.innerHeight;
				const typing = isTextInput(document.activeElement);
				if (!typing) baseline = window.innerHeight;
				mobileKeyboardOpen = typing && baseline - visible > 150;
				// Layout viewport did not follow the visual one: size the view by hand
				mobileViewHeight = window.innerHeight - visible > 150 ? visible : 0;
			};
			updateViewHeight();
			window.addEventListener('resize', updateViewHeight);
			window.visualViewport?.addEventListener('resize', updateViewHeight);
			window.addEventListener('orientationchange', updateViewHeight);
		}

		// Create initial tab — either from default profile or empty
		if (tabs.length === 0) {
			const allProfiles = profilesState.getProfiles();
			const defaultProfile = allProfiles.find((p) => p.is_default);
			if (defaultProfile && defaultProfile.pane_configs) {
				const serverNames: Record<number, string> = {};
				for (const s of serversState.getServers()) {
					serverNames[s.id] = s.name;
				}
				terminalState.applyProfile(
					defaultProfile.layout as LayoutMode,
					defaultProfile.pane_configs,
					serverNames
				);
			} else {
				terminalState.createTab(null, null);
			}
		}

		// Focus the terminal pane for a given tab id

		function disarmLeader() {
			leaderArmed = false;
			leaderHelpPinned = false;
			if (leaderTimer) {
				clearTimeout(leaderTimer);
				leaderTimer = null;
			}
		}

		function armLeader() {
			leaderArmed = true;
			if (leaderTimer) clearTimeout(leaderTimer);
			leaderTimer = setTimeout(() => {
				if (!leaderHelpPinned) disarmLeader();
			}, LEADER_TIMEOUT);
		}

		function cycleLayout() {
			const modes: LayoutMode[] = ['single', 'split-h', 'split-v', 'quad'];
			const idx = modes.indexOf(layout);
			terminalState.setLayout(modes[(idx + 1) % modes.length]);
		}

		/** Run the key that followed the leader. Returns false when it was not bound. */
		function runLeaderAction(e: KeyboardEvent): boolean {
			const action = leaderAction(e);
			if (action === null) {
				// A bare modifier keeps the leader armed; anything else unbound cancels it
				if (!['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) disarmLeader();
				return false;
			}
			if (action.type !== 'help') disarmLeader();
			switch (action.type) {
				case 'new-tab':
					terminalState.createTab(null, null);
					break;
				case 'close-tab':
					if (activeTabId) closeTabWithConfirm(activeTabId);
					break;
				case 'next-tab':
					terminalState.nextTab();
					focusTerminal(terminalState.getActiveTabId());
					break;
				case 'prev-tab':
					terminalState.prevTab();
					focusTerminal(terminalState.getActiveTabId());
					break;
				case 'goto-tab': {
					const target = tabs[action.index];
					if (target) {
						terminalState.setActiveTab(target.id);
						focusTerminal(target.id);
					}
					break;
				}
				case 'last-tab':
					if (lastActiveTabId && tabs.some((t) => t.id === lastActiveTabId)) {
						terminalState.setActiveTab(lastActiveTabId);
						focusTerminal(lastActiveTabId);
					}
					break;
				case 'focus':
					focusTerminal(terminalState.focusDirection(action.direction));
					break;
				case 'next-pane':
					focusTerminal(terminalState.focusNextPane());
					break;
				case 'prev-pane':
					focusTerminal(terminalState.focusPrevPane());
					break;
				case 'cycle-layout':
					cycleLayout();
					break;
				case 'search':
					searchOpen = !searchOpen;
					break;
				case 'palette':
					paletteOpen = true;
					break;
				case 'help':
					leaderHelpPinned = !leaderHelpPinned;
					if (!leaderHelpPinned) disarmLeader();
					break;
				case 'cancel':
					break;
			}
			return true;
		}

		// Keyboard shortcuts — only active when terminal view is visible
		function handleKeydown(e: KeyboardEvent) {
			if (!visible) return;

			// Leader key: arm on the chord, run the next key as a command. Browsers reserve
			// Ctrl+Tab and Ctrl+Shift+T/W, so this is the dependable way to move around.
			const leaderKey = terminalPrefs.getPrefs().leaderKey;
			if (!paletteOpen && !profileSaveOpen) {
				if (leaderArmed) {
					if (isLeaderChord(e, leaderKey)) {
						// Pressed twice: let the shell have the chord itself
						disarmLeader();
						return;
					}
					e.preventDefault();
					e.stopPropagation();
					runLeaderAction(e);
					return;
				}
				if (isLeaderChord(e, leaderKey)) {
					e.preventDefault();
					e.stopPropagation();
					armLeader();
					return;
				}
			}

			// Ctrl+K / Cmd+K — command palette
			if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
				e.preventDefault();
				paletteOpen = !paletteOpen;
				return;
			}
			// Skip all other shortcuts when palette or modals are open
			if (paletteOpen || profileSaveOpen) return;

			// Ctrl+Shift+T — new tab
			if (e.ctrlKey && e.shiftKey && e.key === 'T') {
				e.preventDefault();
				terminalState.createTab(null, null);
			}
			// Ctrl+Shift+W — close tab
			if (e.ctrlKey && e.shiftKey && e.key === 'W') {
				e.preventDefault();
				if (activeTabId) closeTabWithConfirm(activeTabId);
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
					ArrowLeft: 'left',
					ArrowRight: 'right',
					ArrowUp: 'up',
					ArrowDown: 'down'
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
				cycleLayout();
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

		// Listen for save-profile events from sidebar
		const handleSaveProfile = () => {
			profileSaveOpen = true;
		};
		window.addEventListener('save-profile', handleSaveProfile);

		// The sidebar hit the tab cap: open the tab list so the user can close one.
		const handleOpenTabList = () => openSheet('tabs');
		window.addEventListener('open-tab-list', handleOpenTabList);

		window.addEventListener('keydown', handleKeydown, true);
		return () => {
			disarmLeader();
			window.removeEventListener('keydown', handleKeydown, true);
			window.removeEventListener('save-profile', handleSaveProfile);
			window.removeEventListener('open-tab-list', handleOpenTabList);
			if (updateViewHeight) {
				window.removeEventListener('resize', updateViewHeight);
				window.visualViewport?.removeEventListener('resize', updateViewHeight);
			}
			window.removeEventListener('orientationchange', updateViewHeight!);
		};
	});

	function handleConnect() {
		if (!activeTabId || selectedServerId === null) return;
		const server = servers.find((s) => s.id === selectedServerId);
		terminalState.updateTabServer(activeTabId, selectedServerId, server?.name ?? null);
		paneRefs[activeTabId]?.connect();
	}

	function handleDisconnect() {
		if (!activeTabId) return;
		paneRefs[activeTabId]?.disconnect();
	}

	function handleStatusChange(tabId: string, status: ConnectionStatus) {
		terminalState.updateTabStatus(tabId, status);
	}

	// Switching to a tab acknowledges its bell
	$effect(() => {
		terminalState.clearBell(terminalState.getActiveTabId());
	});

	function handleNewTab() {
		terminalState.createTab(null, null);
	}

	/** Put the keyboard into a pane's terminal (after the DOM has caught up) */
	/** A quick command waiting for its variables to be filled in */
	let snippetRequest = $state<SnippetRequest | null>(null);

	/** Type a quick command into the active tab: `{{host}}` and friends from its server, the
	 *  other variables asked for first (`#lib/utils/snippet.ts`). */
	function runQuickCommand(command: string) {
		const tabId = activeTabId;
		if (!tabId || !paneRefs[tabId] || activeTab?.status !== 'connected') {
			toast.warning('Connect to a server first');
			return;
		}
		const server = serversState.getServers().find((s) => s.id === activeTab?.serverId);
		const known = server ? serverVariables(server) : {};
		const variables = promptedVariables(command, known);
		if (variables.length === 0) {
			paneRefs[tabId].injectInput(fillSnippet(command, known) + '\n');
			return;
		}
		snippetRequest = { command, known, variables };
	}

	function focusTerminal(tabId: string | null) {
		if (tabId) {
			requestAnimationFrame(() => paneRefs[tabId]?.focus());
		}
	}

	async function closeTabWithConfirm(id: string) {
		const tab = tabs.find((t) => t.id === id);
		if (tab?.status === 'connected') {
			const ok = await confirmDialog({
				title: `Close ${tab.label}?`,
				message: 'The shell on the server is ended.',
				confirmLabel: 'Close terminal',
				destructive: true
			});
			if (!ok) return;
		}
		paneRefs[id]?.disconnect();
		delete paneRefs[id];
		terminalState.closeTab(id);
	}

	function isTextInput(el: Element | null): boolean {
		return (
			!!el &&
			(el.tagName === 'TEXTAREA' || el.tagName === 'INPUT' || (el as HTMLElement).isContentEditable)
		);
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

<div
	class="flex flex-col overflow-hidden transition-none
		{visible ? 'flex-1 min-w-0 relative' : 'absolute inset-0 invisible pointer-events-none'}"
	style:height={isMobile() && mobileViewHeight > 0 ? `${mobileViewHeight}px` : undefined}
>
	<!-- Mobile: minimal status bar -->
	{#if isMobile()}
		<MobileStatusBar
			serverName={activeTab?.serverName ?? null}
			connectionStatus={activeTab?.status ?? 'disconnected'}
			{tabs}
			{activeTabId}
			onTapDots={() => openSheet('tabs')}
			onNewTab={() => openSheet('servers')}
			onOpenPrefs={() => (prefsOpen = true)}
		/>
		<MobileSheet
			open={sheetOpen}
			view={sheetView}
			{tabs}
			{activeTabId}
			onClose={() => (sheetOpen = false)}
			onSelectTab={(id) => {
				terminalState.setActiveTab(id);
				sheetOpen = false;
				focusTerminal(id);
			}}
			onCloseTab={(id) => closeTabWithConfirm(id)}
			onPickServer={connectTo}
			onShowServers={() => (sheetView = 'servers')}
			onShowSessions={() => (sheetView = 'sessions')}
			onShowFiles={openFiles}
			filesAvailable={!!activeTab?.serverId}
			onBackToTabs={() => (sheetView = 'tabs')}
			sessions={liveSessions}
			held={heldSessions}
			sessionsLoading={sessionsState.isLoading()}
			sessionsError={sessionsState.getError()}
			sessionsLoaded={sessionsState.hasLoaded()}
			onRefreshSessions={refreshSessions}
			onResumeSession={resumeSession}
			onEndSession={endSession}
		/>
	{:else}
		<!-- Desktop: full toolbar -->
		<div
			class="flex flex-wrap items-center gap-2 md:gap-3 md:flex-nowrap px-2 md:px-4 py-2 bg-surface border-b border-border"
		>
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
					{#each servers as server (server.id)}
						<option value={server.id}>{server.name} ({server.hostname})</option>
					{/each}
				{/if}
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
				<button
					class="p-2 md:p-1 text-text-muted hover:text-text-primary"
					onclick={() => {
						if (activeTabId) paneRefs[activeTabId]?.clear();
					}}
					title="Clear"
					aria-label="Clear terminal"
				>
					<Trash2 class="w-4 h-4" />
				</button>
				<button
					class="p-2 md:p-1 text-text-muted hover:text-text-primary"
					onclick={() => {
						if (activeTabId) paneRefs[activeTabId]?.copyOutput();
					}}
					title="Copy"
					aria-label="Copy output"
				>
					<ClipboardCopy class="w-4 h-4" />
				</button>
				<button
					class="p-2 md:p-1 text-text-muted hover:text-text-primary"
					onclick={() => {
						if (activeTabId) paneRefs[activeTabId]?.downloadOutput();
					}}
					title="Download"
					aria-label="Download output"
				>
					<Download class="w-4 h-4" />
				</button>
				<button
					class="p-2 md:p-1 text-text-muted hover:text-text-primary disabled:opacity-40"
					class:text-accent={filesOpen}
					disabled={!activeTab?.serverId}
					onclick={() => {
						if (filesOpen) filesOpen = false;
						else openFiles();
					}}
					title="Files on this server (SFTP)"
					aria-label="Files on this server"
				>
					<FolderOpen class="w-4 h-4" />
				</button>
				<button
					class="relative p-2 md:p-1 text-text-muted hover:text-text-primary"
					class:text-accent={sessionsOpen}
					onclick={() => {
						if (sessionsOpen) sessionsOpen = false;
						else openSessions();
					}}
					title="Sessions on the server (resume a lost shell)"
					aria-label="Sessions on the server{resumableCount > 0
						? `, ${resumableCount} to resume`
						: ''}"
				>
					<History class="w-4 h-4" />
					{#if resumableCount > 0}
						<span
							class="absolute -top-0.5 -right-0.5 min-w-3.5 h-3.5 px-0.5 rounded-full bg-accent text-background text-[9px] font-semibold leading-3.5 text-center"
							aria-hidden="true"
						>
							{resumableCount}
						</span>
					{/if}
				</button>
				<button
					class="p-2 md:p-1 text-text-muted hover:text-text-primary"
					class:text-accent={prefsOpen}
					onclick={() => {
						prefsOpen = !prefsOpen;
						if (prefsOpen) {
							sessionsOpen = false;
							filesOpen = false;
						}
					}}
					title="Terminal Preferences"
					aria-label="Terminal preferences"
				>
					<Settings2 class="w-4 h-4" />
				</button>
			</div>
		</div>
	{/if}

	<!-- Search bar (desktop only) -->
	{#if searchOpen && !isMobile()}
		<div
			class="flex flex-wrap items-center gap-2 px-2 md:px-4 py-1.5 bg-surface-raised border-b border-border"
		>
			<input
				type="text"
				bind:value={searchTerm}
				oninput={handleSearch}
				class="px-2 py-1 text-sm bg-input border border-border rounded-sm text-text-primary placeholder:text-text-muted w-full md:w-64"
				placeholder="Search terminal output..."
			/>
			<div class="flex gap-1 mt-1 md:mt-0 md:gap-2">
				<button
					class="px-2 py-1 text-xs text-text-muted hover:text-text-primary"
					onclick={handleSearchPrev}>Prev</button
				>
				<button
					class="px-2 py-1 text-xs text-text-muted hover:text-text-primary"
					onclick={handleSearchNext}>Next</button
				>
				<button
					class="px-2 py-1 text-xs text-text-muted hover:text-text-primary"
					onclick={() => {
						searchOpen = false;
						searchTerm = '';
						if (activeTabId) paneRefs[activeTabId]?.clearSearch();
					}}>Close</button
				>
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
			onCloseTab={(id) => closeTabWithConfirm(id)}
			onNewTab={handleNewTab}
			onSetLayout={(mode) => terminalState.setLayout(mode)}
		/>
	{/if}

	<!-- Terminal panes -->
	<div class="flex-1 min-h-0 relative">
		<SplitView {layout} slotCount={visibleTabs.length}>
			{#each tabs as tab (tab.id)}
				<div
					class="min-h-0 min-w-0 relative"
					class:hidden={tab.slot === null}
					class:ring-1={tab.id === activeTabId && layout !== 'single' && visibleTabs.length > 1}
					class:ring-accent={tab.id === activeTabId &&
						layout !== 'single' &&
						visibleTabs.length > 1}
					onfocusin={() => {
						// Clicking or tapping into a pane makes its tab the active one, in place
						if (tab.slot !== null && tab.id !== activeTabId) terminalState.focusSlot(tab.slot);
					}}
				>
					{#if tab.slot !== null && layout !== 'single' && visibleTabs.length > 1}
						<div
							class="absolute top-0 left-1 z-10 px-1.5 py-0 text-[10px] font-mono rounded-b-sm {tab.id ===
							activeTabId
								? 'bg-accent text-background'
								: 'bg-surface-raised/80 text-text-muted'}"
						>
							{tab.slot + 1}
						</div>
					{/if}
					<TerminalPane
						bind:this={paneRefs[tab.id]}
						tabId={tab.id}
						serverId={tab.serverId}
						active={tab.slot !== null}
						onStatusChange={handleStatusChange}
						onTitleChange={terminalState.updateTabTitle}
						onBell={terminalState.markBell}
						onModifierConsumed={() => extraKeysRow?.clearModifiers()}
						onOpenPalette={() => (paletteOpen = true)}
						onSwipe={() => focusTerminal(terminalState.getActiveTabId())}
					/>
				</div>
			{/each}
		</SplitView>
		{#if showEmptyState}
			<!-- No server in front: offer the servers instead of a blank pane -->
			<div class="absolute inset-0 z-10 overflow-y-auto bg-background">
				{#if serversLoading && enabledServers.length === 0}
					<StateBlock kind="loading" title="Loading servers…" />
				{:else if serversError && enabledServers.length === 0}
					<StateBlock
						kind="error"
						title={serversError}
						onRetry={() => serversState.loadServers()}
					/>
				{:else if enabledServers.length === 0}
					<StateBlock
						kind="empty"
						icon={ServerIcon}
						title="No servers yet"
						hint="Add a server, then open a shell on it here."
					>
						<Button onclick={() => goto(resolve('/servers'))}>Add a server</Button>
					</StateBlock>
				{:else}
					<div class="max-w-md mx-auto px-4 py-8 md:py-12 flex flex-col gap-3">
						<div class="text-center">
							<h2 class="text-sm font-medium text-text-primary">Open a shell</h2>
							<p class="text-xs text-text-muted mt-1">
								{isMobile()
									? 'Pick a server, or + for the full list.'
									: 'Pick a server, or use the list above (Ctrl+K searches).'}
							</p>
						</div>
						<ul class="flex flex-col gap-1">
							{#each enabledServers.slice(0, EMPTY_STATE_SERVERS) as server (server.id)}
								{@const st = serverStatus(server)}
								<li>
									<button
										type="button"
										class="w-full flex items-center gap-3 px-3 min-h-12 rounded-md border border-border/60 bg-surface hover:bg-surface-raised active:bg-surface-raised text-left"
										onclick={() => connectTo(server)}
									>
										<span class="w-2 h-2 rounded-full flex-shrink-0 {st.dot}" title={st.label}
										></span>
										<span class="min-w-0 flex-1">
											<span class="block text-sm text-text-primary truncate">{server.name}</span>
											<span class="block text-[11px] font-mono text-text-muted truncate"
												>{server.username ?? 'root'}@{server.hostname}</span
											>
										</span>
									</button>
								</li>
							{/each}
						</ul>
						<div class="flex flex-wrap justify-center gap-2">
							{#if enabledServers.length > EMPTY_STATE_SERVERS}
								<Button
									variant="secondary"
									class="min-h-11 md:min-h-0"
									onclick={() => (isMobile() ? openSheet('servers') : (paletteOpen = true))}
									>All {enabledServers.length} servers</Button
								>
							{/if}
							{#if resumableCount > 0}
								<Button variant="secondary" class="min-h-11 md:min-h-0" onclick={openSessions}
									><History class="w-4 h-4" /> Resume a session ({resumableCount})</Button
								>
							{/if}
						</div>
					</div>
				{/if}
			</div>
		{/if}

		<!-- Gesture hint (mobile, first visit only) -->
		{#if showGestureHint}
			<div
				class="absolute bottom-2 left-0 right-0 text-center text-[10px] text-text-muted transition-opacity duration-1000 z-10 pointer-events-none"
				class:opacity-0={!showGestureHint}
			>
				double-tap for commands · swipe ← → tabs
			</div>
		{/if}

		<TerminalPrefsPanel open={prefsOpen} onClose={() => (prefsOpen = false)} />
		<FileViewer onEditInTerminal={editInTerminal} />
		{#if !isMobile()}
			<FilesPanel open={filesOpen} onClose={() => (filesOpen = false)} />
			<SessionsPanel
				open={sessionsOpen}
				sessions={liveSessions}
				held={heldSessions}
				loading={sessionsState.isLoading()}
				error={sessionsState.getError()}
				loaded={sessionsState.hasLoaded()}
				onClose={() => (sessionsOpen = false)}
				onRefresh={refreshSessions}
				onResume={resumeSession}
				onEnd={endSession}
				onGoToTab={goToSessionTab}
			/>
		{/if}
		<ProfileManager open={profileSaveOpen} onClose={() => (profileSaveOpen = false)} />
		<SnippetVarsDialog
			request={snippetRequest}
			onRun={(command) => {
				snippetRequest = null;
				const tabId = activeTabId;
				if (tabId && activeTab?.status === 'connected') {
					paneRefs[tabId]?.injectInput(command + '\n');
				} else {
					toast.warning('Connect to a server first');
				}
				focusTerminal(tabId);
			}}
			onClose={() => {
				snippetRequest = null;
				focusTerminal(activeTabId);
			}}
		/>
		<CommandPalette
			open={paletteOpen}
			onClose={() => {
				paletteOpen = false;
				// Back to typing; on a phone this also brings the soft keyboard back
				focusTerminal(activeTabId);
			}}
			onInjectCommand={runQuickCommand}
			onSaveProfile={() => (profileSaveOpen = true)}
			onClearTerminal={() => {
				if (activeTabId) paneRefs[activeTabId]?.clear();
			}}
			onCopyOutput={() => {
				if (activeTabId) paneRefs[activeTabId]?.copyOutput();
			}}
			onDownloadOutput={() => {
				if (activeTabId) paneRefs[activeTabId]?.downloadOutput();
			}}
			onToggleSearch={() => {
				searchOpen = !searchOpen;
			}}
			onConnect={handleConnect}
			onDisconnect={handleDisconnect}
			onConnectServer={(id, name) => connectTo({ id, name })}
			onNewTab={() => {
				if (isMobile()) openSheet('servers');
				else handleNewTab();
			}}
			onCloseTab={() => {
				if (activeTabId) closeTabWithConfirm(activeTabId);
			}}
			onOpenPrefs={() => (prefsOpen = true)}
			onOpenServers={() => goto(resolve('/servers'))}
			onOpenSessions={openSessions}
			onOpenFiles={openFiles}
		/>
	</div>

	{#if leaderArmed}
		<div
			class="absolute bottom-3 left-1/2 -translate-x-1/2 z-20 max-w-[95%] px-3 py-2 rounded-md bg-surface/95 border border-border shadow-lg text-xs text-text-secondary pointer-events-none"
			role="status"
			aria-live="polite"
		>
			<div class="flex flex-wrap gap-x-3 gap-y-1 justify-center">
				<span class="font-semibold text-accent">
					{LEADER_KEY_LABELS[terminalPrefs.getPrefs().leaderKey]} …
				</span>
				{#each LEADER_BINDINGS as b (b.keys)}
					<span><kbd class="font-mono text-text-primary">{b.keys}</kbd> {b.action}</span>
				{/each}
				{#if !leaderHelpPinned}
					<span><kbd class="font-mono text-text-primary">?</kbd> keep open</span>
				{/if}
			</div>
		</div>
	{/if}

	<!-- Extra keys row (mobile only, in flex flow so it stays above keyboard) -->
	{#if isMobile()}
		<div
			class="flex-shrink-0"
			style:padding-bottom={!mobileKeyboardOpen ? 'env(safe-area-inset-bottom, 0px)' : undefined}
			bind:offsetHeight={extraKeysHeight}
		>
			<ExtraKeysRow
				bind:this={extraKeysRow}
				onKeyPress={(seq) => {
					if (activeTabId && paneRefs[activeTabId]) {
						paneRefs[activeTabId].injectInput(seq);
					}
				}}
				onModifier={(mod) => {
					if (activeTabId) paneRefs[activeTabId]?.setPendingModifier(mod);
				}}
				onPalette={() => (paletteOpen = true)}
			/>
		</div>
	{/if}
</div>
