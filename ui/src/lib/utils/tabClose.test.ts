import { describe, expect, it } from 'vitest';
import type { TerminalTab, ConnectionStatus } from '#lib/types/index.js';
import { closeManyPrompt, closeTargets } from './tabClose.js';

function tab(id: string, status: ConnectionStatus = 'connected'): TerminalTab {
	return { id, label: `web-${id}`, serverId: 1, serverName: 'web', status, slot: null };
}

const tabs = ['a', 'b', 'c', 'd'].map((id) => tab(id));

describe('closeTargets', () => {
	it('takes every tab for all', () => {
		expect(closeTargets(tabs, 'b', 'all')).toEqual(['a', 'b', 'c', 'd']);
		expect(closeTargets(tabs, null, 'all')).toEqual(['a', 'b', 'c', 'd']);
	});

	it('keeps the active tab for others', () => {
		expect(closeTargets(tabs, 'b', 'others')).toEqual(['a', 'c', 'd']);
	});

	it('takes the tabs after the active one for right', () => {
		expect(closeTargets(tabs, 'b', 'right')).toEqual(['c', 'd']);
		expect(closeTargets(tabs, 'd', 'right')).toEqual([]);
	});

	it('closes nothing relative to a missing active tab', () => {
		expect(closeTargets(tabs, null, 'others')).toEqual([]);
		expect(closeTargets(tabs, 'zz', 'right')).toEqual([]);
	});
});

describe('closeManyPrompt', () => {
	it('asks once, counting the live shells', () => {
		const p = closeManyPrompt([tab('a'), tab('b', 'disconnected'), tab('c')]);
		expect(p?.title).toBe('Close 3 terminals?');
		expect(p?.message).toBe('2 shells on the servers are ended.');
		expect(p?.confirmLabel).toBe('Close 3 terminals');
		expect(p?.destructive).toBe(true);
	});

	it('words a single tab like the one-tab close', () => {
		const p = closeManyPrompt([tab('a')]);
		expect(p?.title).toBe('Close web-a?');
		expect(p?.message).toBe('The shell on the server is ended.');
	});

	it('does not ask when no shell is live', () => {
		expect(closeManyPrompt([tab('a', 'disconnected'), tab('b', 'error')])).toBeNull();
		expect(closeManyPrompt([])).toBeNull();
	});
});
