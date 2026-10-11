import { describe, expect, it } from 'vitest';
import {
	heldSessionIds,
	loadSessionRecord,
	saveSessionRecord,
	sessionRecordKey,
	type KeyValueStore
} from './sessionRecord.js';

function fakeStore(): KeyValueStore & { map: Map<string, string> } {
	const map = new Map<string, string>();
	return {
		map,
		getItem: (k) => map.get(k) ?? null,
		setItem: (k, v) => void map.set(k, v),
		removeItem: (k) => void map.delete(k)
	};
}

describe('session records', () => {
	it('round-trips a record and removes it on null', () => {
		const store = fakeStore();
		saveSessionRecord('term-1', { sessionId: 'abc', lastSeq: 42, serverId: 3 }, store);
		expect(loadSessionRecord('term-1', store)).toEqual({
			sessionId: 'abc',
			lastSeq: 42,
			serverId: 3
		});
		expect(store.map.has(sessionRecordKey('term-1'))).toBe(true);

		saveSessionRecord('term-1', null, store);
		expect(loadSessionRecord('term-1', store)).toBeNull();
		expect(store.map.size).toBe(0);
	});

	it('ignores records that are missing, malformed or without a session id', () => {
		const store = fakeStore();
		expect(loadSessionRecord('term-9', store)).toBeNull();
		store.setItem(sessionRecordKey('term-2'), 'not json');
		expect(loadSessionRecord('term-2', store)).toBeNull();
		store.setItem(sessionRecordKey('term-3'), JSON.stringify({ lastSeq: 5 }));
		expect(loadSessionRecord('term-3', store)).toBeNull();
		store.setItem(sessionRecordKey('term-4'), JSON.stringify({ sessionId: '' }));
		expect(loadSessionRecord('term-4', store)).toBeNull();
	});

	it('defaults a bad lastSeq to 0 (replay everything) and a bad serverId to null', () => {
		const store = fakeStore();
		store.setItem(
			sessionRecordKey('t'),
			JSON.stringify({ sessionId: 'x', lastSeq: -1, serverId: '3' })
		);
		expect(loadSessionRecord('t', store)).toEqual({ sessionId: 'x', lastSeq: 0, serverId: null });
	});

	it('maps held session ids back to the tabs that hold them', () => {
		const store = fakeStore();
		saveSessionRecord('term-1', { sessionId: 'a', lastSeq: 1, serverId: 1 }, store);
		saveSessionRecord('term-3', { sessionId: 'c', lastSeq: 0, serverId: 2 }, store);
		// term-7 is a tab from an earlier page life: not in the list, so not held
		saveSessionRecord('term-7', { sessionId: 'z', lastSeq: 0, serverId: 2 }, store);
		const held = heldSessionIds(['term-1', 'term-2', 'term-3'], store);
		expect([...held.entries()]).toEqual([
			['a', 'term-1'],
			['c', 'term-3']
		]);
	});

	it('is a no-op without any storage', () => {
		// node has no sessionStorage; the helpers must not throw
		expect(loadSessionRecord('term-1')).toBeNull();
		saveSessionRecord('term-1', { sessionId: 'a', lastSeq: 0, serverId: null });
		expect(heldSessionIds(['term-1']).size).toBe(0);
	});
});
