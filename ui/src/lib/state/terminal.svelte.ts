import type { TerminalTab, TerminalMode, ConnectionStatus, LayoutMode } from '$lib/types/index.js';

let tabs = $state<TerminalTab[]>([]);
let activeTabId = $state<string | null>(null);
let layout = $state<LayoutMode>('single');
let tabCounter = $state(0);
let pendingAutoConnect = $state<string | null>(null);

const MAX_TABS = 10;

export function getTabs() {
	return tabs;
}

export function getActiveTabId() {
	return activeTabId;
}

export function getActiveTab(): TerminalTab | undefined {
	return tabs.find((t) => t.id === activeTabId);
}

export function getLayout() {
	return layout;
}

export function setLayout(mode: LayoutMode) {
	layout = mode;
	assignSlots();
}

export function getVisibleTabs(): TerminalTab[] {
	return tabs.filter((t) => t.slot !== null);
}

export function createTab(
	serverId: number | null = null,
	serverName: string | null = null,
	mode: TerminalMode = 'pty'
): TerminalTab | null {
	if (tabs.length >= MAX_TABS) return null;

	tabCounter++;
	const tab: TerminalTab = {
		id: `term-${tabCounter}`,
		label: serverName ?? `Terminal ${tabCounter}`,
		serverId,
		serverName,
		mode,
		status: 'disconnected',
		slot: null
	};

	tabs.push(tab);
	activeTabId = tab.id;
	assignSlots();
	return tab;
}

export function closeTab(id: string) {
	const idx = tabs.findIndex((t) => t.id === id);
	if (idx === -1) return;

	tabs.splice(idx, 1);

	if (activeTabId === id) {
		activeTabId = tabs.length > 0 ? tabs[Math.min(idx, tabs.length - 1)].id : null;
	}

	assignSlots();
}

export function setActiveTab(id: string) {
	if (tabs.find((t) => t.id === id)) {
		activeTabId = id;
		assignSlots();
	}
}

export function nextTab() {
	if (tabs.length <= 1) return;
	const idx = tabs.findIndex((t) => t.id === activeTabId);
	const next = (idx + 1) % tabs.length;
	activeTabId = tabs[next].id;
	assignSlots();
}

export function prevTab() {
	if (tabs.length <= 1) return;
	const idx = tabs.findIndex((t) => t.id === activeTabId);
	const prev = (idx - 1 + tabs.length) % tabs.length;
	activeTabId = tabs[prev].id;
	assignSlots();
}

/** Focus the tab at a specific visible slot (0-3) without rearranging slots */
export function focusSlot(slot: number): string | null {
	const tab = tabs.find((t) => t.slot === slot);
	if (tab) {
		activeTabId = tab.id;
		return tab.id;
	}
	return null;
}

/** Get the tab at a given slot */
export function getTabBySlot(slot: number): TerminalTab | undefined {
	return tabs.find((t) => t.slot === slot);
}

/** Get the slot of the currently active tab */
export function getActiveSlot(): number | null {
	const tab = tabs.find((t) => t.id === activeTabId);
	return tab?.slot ?? null;
}

/** Navigate to an adjacent slot based on direction and layout.
 *  Returns the tab id that was focused, or null if no movement possible. */
export function focusDirection(direction: 'left' | 'right' | 'up' | 'down'): string | null {
	const currentSlot = getActiveSlot();
	if (currentSlot === null) return null;

	const maxSlots = layout === 'single' ? 1
		: layout === 'split-h' || layout === 'split-v' ? 2
		: 4;

	if (maxSlots <= 1) return null;

	let targetSlot: number | null = null;

	if (layout === 'split-h') {
		// Two columns: slot 0 = left, slot 1 = right
		if (direction === 'left' && currentSlot === 1) targetSlot = 0;
		if (direction === 'right' && currentSlot === 0) targetSlot = 1;
	} else if (layout === 'split-v') {
		// Two rows: slot 0 = top, slot 1 = bottom
		if (direction === 'up' && currentSlot === 1) targetSlot = 0;
		if (direction === 'down' && currentSlot === 0) targetSlot = 1;
	} else if (layout === 'quad') {
		// 2x2 grid: slot 0=TL, 1=TR, 2=BL, 3=BR
		const grid: Record<number, Record<string, number>> = {
			0: { right: 1, down: 2 },
			1: { left: 0, down: 3 },
			2: { right: 3, up: 0 },
			3: { left: 2, up: 1 }
		};
		targetSlot = grid[currentSlot]?.[direction] ?? null;
	}

	if (targetSlot !== null) {
		return focusSlot(targetSlot);
	}
	return null;
}

/** Cycle focus between visible panes only (without rearranging slots) */
export function focusNextPane(): string | null {
	const visible = tabs.filter((t) => t.slot !== null).sort((a, b) => a.slot! - b.slot!);
	if (visible.length <= 1) return null;
	const idx = visible.findIndex((t) => t.id === activeTabId);
	const next = (idx + 1) % visible.length;
	activeTabId = visible[next].id;
	return visible[next].id;
}

export function focusPrevPane(): string | null {
	const visible = tabs.filter((t) => t.slot !== null).sort((a, b) => a.slot! - b.slot!);
	if (visible.length <= 1) return null;
	const idx = visible.findIndex((t) => t.id === activeTabId);
	const prev = (idx - 1 + visible.length) % visible.length;
	activeTabId = visible[prev].id;
	return visible[prev].id;
}

export function updateTabStatus(id: string, status: ConnectionStatus) {
	const tab = tabs.find((t) => t.id === id);
	if (tab) tab.status = status;
}

export function updateTabMode(id: string, mode: TerminalMode) {
	const tab = tabs.find((t) => t.id === id);
	if (tab) tab.mode = mode;
}

export function getPendingAutoConnect(): string | null {
	return pendingAutoConnect;
}

export function setPendingAutoConnect(tabId: string) {
	pendingAutoConnect = tabId;
}

export function clearPendingAutoConnect() {
	pendingAutoConnect = null;
}

export function updateTabServer(id: string, serverId: number | null, serverName: string | null) {
	const tab = tabs.find((t) => t.id === id);
	if (tab) {
		tab.serverId = serverId;
		tab.serverName = serverName;
		tab.label = serverName ?? tab.label;
	}
}

/** Capture the current layout as a profile-compatible state */
export function captureProfileState(): { layout: LayoutMode; panes: Array<{ server_id: number | null; mode: string }> } {
	return {
		layout,
		panes: tabs.map((t) => ({
			server_id: t.serverId,
			mode: t.mode
		}))
	};
}

/** Apply a profile: close all tabs and create new ones matching the profile */
export function applyProfile(profileLayout: LayoutMode, panes: Array<{ server_id: number | null; mode: string | null }>, serverNames: Record<number, string>) {
	// Close all existing tabs
	while (tabs.length > 0) {
		tabs.pop();
	}
	tabCounter = 0;
	activeTabId = null;

	// Set layout
	layout = profileLayout;

	// Create tabs for each pane
	for (const pane of panes) {
		const name = pane.server_id ? (serverNames[pane.server_id] ?? `Server ${pane.server_id}`) : null;
		const mode = (pane.mode === 'cmd' ? 'cmd' : 'pty') as TerminalMode;
		const tab = createTab(pane.server_id, name, mode);
		if (tab && pane.server_id) {
			pendingAutoConnect = tab.id;
		}
	}
}

/** Assign visible slot indices based on layout and active tab */
function assignSlots() {
	const maxSlots =
		layout === 'single' ? 1 : layout === 'split-h' || layout === 'split-v' ? 2 : 4;

	// Reset all slots
	for (const tab of tabs) tab.slot = null;

	if (tabs.length === 0) return;

	// Active tab always gets slot 0
	const active = tabs.find((t) => t.id === activeTabId);
	if (active) active.slot = 0;

	// Fill remaining slots with adjacent tabs
	if (maxSlots > 1) {
		const activeIdx = tabs.findIndex((t) => t.id === activeTabId);
		let slotNum = 1;
		for (let i = 1; i < tabs.length && slotNum < maxSlots; i++) {
			const idx = (activeIdx + i) % tabs.length;
			tabs[idx].slot = slotNum++;
		}
	}
}
