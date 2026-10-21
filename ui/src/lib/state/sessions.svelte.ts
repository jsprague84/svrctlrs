import type { LiveSession } from '#lib/types/index.js';
import * as api from '#lib/api/sessions.js';
import { extractErrorMessage } from '#lib/utils/error.js';

/** The user's live server-side sessions, for the resume list. Refreshed on demand. */
let sessions = $state<LiveSession[]>([]);
let detachTtlSecs = $state(900);
let ttlChoices = $state<number[]>([]);
let ttlChosen = $state(false);
let loading = $state(false);
let error = $state<string | null>(null);
let loadedOnce = $state(false);

export function getSessions() {
	return sessions;
}

export function getDetachTtlSecs() {
	return detachTtlSecs;
}

/** The keep times this server offers, in seconds */
export function getTtlChoices() {
	return ttlChoices;
}

/** Whether the keep time is the user's own choice (else the server's default) */
export function isTtlChosen() {
	return ttlChosen;
}

/** Choose how long shells are kept with nothing attached; null goes back to the default.
 *  Throws on failure. */
export async function setKeepTime(secs: number | null): Promise<void> {
	const res = await api.setKeepTime(secs);
	detachTtlSecs = res.detach_ttl_secs;
	ttlChosen = res.detach_ttl_chosen;
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
		ttlChoices = res.detach_ttl_choices ?? [];
		ttlChosen = res.detach_ttl_chosen ?? false;
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
