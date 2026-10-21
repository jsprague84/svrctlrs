import { get, del, put } from './client.js';
import type { LiveSession } from '#lib/types/index.js';

export interface SessionsListResponse {
	sessions: LiveSession[];
	/** How long the caller's shells are kept with nothing attached */
	detach_ttl_secs: number;
	/** The longest the server allows */
	detach_ttl_max_secs: number;
	/** The keep times the caller can choose from */
	detach_ttl_choices: number[];
	/** The caller chose `detach_ttl_secs`; false means the server's default */
	detach_ttl_chosen: boolean;
}

export interface SessionKeep {
	detach_ttl_secs: number;
	detach_ttl_chosen: boolean;
}

/** Live interactive sessions of the calling user, oldest first (the server's order). */
export function listSessions(): Promise<SessionsListResponse> {
	return get<SessionsListResponse>('/sessions');
}

/** End the shell behind a session, whether or not anything is attached to it. */
export async function endSession(id: string): Promise<void> {
	await del<{ id: string; message: string }>(`/sessions/${encodeURIComponent(id)}`);
}

/** Choose how long shells are kept with nothing attached (one of the choices, or null for the
 *  server's default). Shells already waiting are kept that long from now. */
export function setKeepTime(secs: number | null): Promise<SessionKeep> {
	return put<SessionKeep>('/sessions/keep', { detach_ttl_secs: secs });
}
