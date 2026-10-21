import type { LiveSession } from '#lib/types/index.js';
import { timeAgo } from './timeAgo.js';

/** A live session as the resume list shows it: with the tab in this window that holds it, if any. */
export interface SessionRow {
	session: LiveSession;
	/** Tab id when a pane in this window is attached to (or resuming) the session */
	tabId: string | null;
}

export interface ClassifiedSessions {
	/** Sessions no tab here holds — a closed or lost tab, another device, another browser */
	resumable: SessionRow[];
	/** Sessions already open in this window */
	open: SessionRow[];
}

/** Split the user's live sessions into those this window holds and those it can resume. */
export function classifySessions(
	sessions: LiveSession[],
	held: Map<string, string>
): ClassifiedSessions {
	const resumable: SessionRow[] = [];
	const open: SessionRow[] = [];
	for (const session of sessions) {
		const tabId = held.get(session.id) ?? null;
		(tabId ? open : resumable).push({ session, tabId });
	}
	// Newest first: the shell you just lost is the one you want back
	const newest = (a: SessionRow, b: SessionRow) =>
		Date.parse(b.session.created_at) - Date.parse(a.session.created_at);
	resumable.sort(newest);
	open.sort(newest);
	return { resumable, open };
}

/** One line under the server name: age, and whether something else is attached. */
export function describeSession(row: SessionRow, now: Date = new Date()): string {
	const parts = [`started ${timeAgo(row.session.created_at, now)}`];
	if (row.tabId) {
		parts.push('open here');
	} else if (row.session.attachments > 0) {
		parts.push(
			row.session.attachments === 1
				? 'attached elsewhere'
				: `attached elsewhere ×${row.session.attachments}`
		);
	} else if (row.session.waiting) {
		// Kept in tmux across a restart; resuming reconnects it
		parts.push('waiting in tmux');
	} else {
		parts.push('detached');
	}
	return parts.join(' · ');
}

/** "15 min", "2 h", "90 s" for the footer's grace-period note. */
export function formatTtl(secs: number): string {
	if (!Number.isFinite(secs) || secs <= 0) return '';
	if (secs % 3600 === 0) return `${secs / 3600} h`;
	if (secs % 60 === 0) return `${secs / 60} min`;
	return `${secs} s`;
}
