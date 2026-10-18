import type { Server } from '#lib/types/index.js';

export type ServerStatusKind = 'online' | 'unreachable' | 'unknown' | 'disabled';

export interface ServerStatus {
	kind: ServerStatusKind;
	/** Short label for badges and tooltips */
	label: string;
	/** Tailwind background class for the status dot */
	dot: string;
}

/** How long a successful contact counts as "online" when nothing newer has been recorded. */
export const ONLINE_WINDOW_MS = 15 * 60 * 1000;

/**
 * What the sidebar dot and the Servers page say about a server. Derived from what the
 * health monitor and connections record: `last_error` means the last attempt failed,
 * a recent `last_seen_at` means it answered, anything else is simply unknown — never a
 * green dot for a host nobody has heard from.
 */
export function serverStatus(server: Server, now: Date = new Date()): ServerStatus {
	if (!server.enabled) return { kind: 'disabled', label: 'Disabled', dot: 'bg-text-muted' };
	if (server.last_error) {
		return { kind: 'unreachable', label: `Unreachable: ${server.last_error}`, dot: 'bg-error' };
	}
	if (server.last_seen_at) {
		const age = now.getTime() - new Date(server.last_seen_at).getTime();
		if (age <= ONLINE_WINDOW_MS) {
			return {
				kind: 'online',
				label: `Online (seen ${Math.round(age / 60000)} min ago)`,
				dot: 'bg-success'
			};
		}
		return {
			kind: 'unknown',
			label: `Last seen ${new Date(server.last_seen_at).toLocaleString()}`,
			dot: 'bg-warning'
		};
	}
	return { kind: 'unknown', label: 'Not checked yet', dot: 'bg-text-muted' };
}
