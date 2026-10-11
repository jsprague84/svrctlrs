import type { LiveSession } from '#lib/types/index.js';
import * as api from '#lib/api/sessions.js';
import { extractErrorMessage } from '#lib/utils/error.js';

/** The user's live server-side sessions, for the resume list. Refreshed on demand. */
let sessions = $state<LiveSession[]>([]);
let detachTtlSecs = $state(900);
let loading = $state(false);
let error = $state<string | null>(null);
let loadedOnce = $state(false);

export function getSessions() {
	return sessions;
}

export function getDetachTtlSecs() {
	return detachTtlSecs;
}

export function isLoading() {
	return loading;
}

export function getError() {
	return error;
}

export function hasLoaded() {
	return loadedOnce;
}

export async function loadSessions() {
	loading = true;
	error = null;
	try {
		const res = await api.listSessions();
		sessions = res.sessions;
		detachTtlSecs = res.detach_ttl_secs;
		loadedOnce = true;
	} catch (e) {
		error = extractErrorMessage(e, 'Failed to load sessions');
	} finally {
		loading = false;
	}
}

/** End a session for good and drop it from the list. Throws on failure. */
export async function endSession(id: string): Promise<void> {
	await api.endSession(id);
	sessions = sessions.filter((s) => s.id !== id);
}

/** Forget a session locally (it ended on its own, or a tab here just adopted it). */
export function dropSession(id: string) {
	sessions = sessions.filter((s) => s.id !== id);
}
