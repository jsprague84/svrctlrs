import { get, del } from './client.js';
import type { LiveSession } from '#lib/types/index.js';

export interface SessionsListResponse {
	sessions: LiveSession[];
	/** How long a session with nothing attached is kept before the shell is closed */
	detach_ttl_secs: number;
}

/** Live interactive sessions of the calling user, oldest first (the server's order). */
export function listSessions(): Promise<SessionsListResponse> {
	return get<SessionsListResponse>('/sessions');
}

/** End the shell behind a session, whether or not anything is attached to it. */
export async function endSession(id: string): Promise<void> {
	await del<{ id: string; message: string }>(`/sessions/${encodeURIComponent(id)}`);
}
