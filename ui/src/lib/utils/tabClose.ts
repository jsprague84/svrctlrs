import type { TerminalTab } from '#lib/types/index.js';
import type { ConfirmOptions } from '#lib/state/confirm.svelte.js';

/** Which tabs a close action takes: all of them, all but the active one, or those after it. */
export type CloseScope = 'all' | 'others' | 'right';

export function closeTargets(
	tabs: TerminalTab[],
	activeTabId: string | null,
	scope: CloseScope
): string[] {
	if (scope === 'all') return tabs.map((t) => t.id);
	const idx = tabs.findIndex((t) => t.id === activeTabId);
	if (idx === -1) return [];
	if (scope === 'others') return tabs.filter((t) => t.id !== activeTabId).map((t) => t.id);
	return tabs.slice(idx + 1).map((t) => t.id);
}

/**
 * One question for closing several tabs at once, or null when none of them holds a live shell
 * (closing those loses nothing, so it is not asked).
 */
export function closeManyPrompt(closing: TerminalTab[]): ConfirmOptions | null {
	const live = closing.filter((t) => t.status === 'connected').length;
	if (live === 0) return null;
	const n = closing.length;
	return {
		title: n === 1 ? `Close ${closing[0].label}?` : `Close ${n} terminals?`,
		message:
			live === 1 ? 'The shell on the server is ended.' : `${live} shells on the servers are ended.`,
		confirmLabel: n === 1 ? 'Close terminal' : `Close ${n} terminals`,
		destructive: true
	};
}
