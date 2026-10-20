import { get } from './client.js';

export interface AuditEntry {
	id: number;
	at: string;
	user_id: number | null;
	username: string | null;
	client_ip: string | null;
	action: string;
	target_type: string | null;
	target_id: number | null;
	target_name: string | null;
	/** JSON object string */
	details: string | null;
	/** The API token the action came in with, if any (its name at the time) */
	token_id: number | null;
	token_name: string | null;
}

export interface AuditPage {
	entries: AuditEntry[];
	next_before: number | null;
}

export interface AuditFilter {
	/** Comma-separated action prefixes, e.g. `login,session.` */
	action?: string;
	user_id?: number;
	/** Free text over action, target, details, user, address and token name */
	q?: string;
}

export async function listAudit(
	limit = 100,
	before?: number,
	filter: AuditFilter = {}
): Promise<AuditPage> {
	const params = new URLSearchParams({ limit: String(limit) });
	if (before != null) params.set('before', String(before));
	if (filter.action) params.set('action', filter.action);
	if (filter.user_id != null) params.set('user_id', String(filter.user_id));
	if (filter.q) params.set('q', filter.q);
	return get<AuditPage>(`/audit?${params}`);
}
