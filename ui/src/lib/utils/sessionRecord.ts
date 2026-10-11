/**
 * The per-tab record of the server-side session a terminal pane is (or was last) attached to.
 * It lives in sessionStorage so a reload resumes the shell, and it is written by two places:
 * the pane itself as it connects, and the terminal state when a lost session is resumed into
 * a fresh tab (the pane then finds the record on mount and attaches).
 */

export interface SessionRecord {
	sessionId: string;
	/** Last output `seq` shown; 0 asks the server for everything its ring still holds */
	lastSeq: number;
	/** The server the session was opened on; a tab pointed elsewhere must not resume it */
	serverId: number | null;
}

/** The subset of `Storage` these helpers use, so tests can pass a plain fake. */
export interface KeyValueStore {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
	removeItem(key: string): void;
}

const PREFIX = 'svrctlrs-term-session-';

export function sessionRecordKey(tabId: string): string {
	return `${PREFIX}${tabId}`;
}

function pick(store?: KeyValueStore): KeyValueStore | null {
	if (store) return store;
	return typeof sessionStorage === 'undefined' ? null : sessionStorage;
}

export function loadSessionRecord(tabId: string, store?: KeyValueStore): SessionRecord | null {
	const s = pick(store);
	if (!s) return null;
	try {
		const raw = s.getItem(sessionRecordKey(tabId));
		if (!raw) return null;
		const saved = JSON.parse(raw) as Partial<SessionRecord>;
		if (typeof saved.sessionId !== 'string' || !saved.sessionId) return null;
		return {
			sessionId: saved.sessionId,
			lastSeq: typeof saved.lastSeq === 'number' && saved.lastSeq >= 0 ? saved.lastSeq : 0,
			serverId: typeof saved.serverId === 'number' ? saved.serverId : null
		};
	} catch {
		return null;
	}
}

/** Write the record, or remove it when `record` is null. */
export function saveSessionRecord(
	tabId: string,
	record: SessionRecord | null,
	store?: KeyValueStore
): void {
	const s = pick(store);
	if (!s) return;
	try {
		if (record) s.setItem(sessionRecordKey(tabId), JSON.stringify(record));
		else s.removeItem(sessionRecordKey(tabId));
	} catch {
		// storage full or unavailable
	}
}

/** Which server-side sessions the given tabs hold: session id → tab id. */
export function heldSessionIds(tabIds: string[], store?: KeyValueStore): Map<string, string> {
	const held = new Map<string, string>();
	for (const tabId of tabIds) {
		const rec = loadSessionRecord(tabId, store);
		if (rec) held.set(rec.sessionId, tabId);
	}
	return held;
}
