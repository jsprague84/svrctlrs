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
}

export interface AuditPage {
	entries: AuditEntry[];
	next_before: number | null;
}

export async function listAudit(limit = 100, before?: number): Promise<AuditPage> {
	const params = new URLSearchParams({ limit: String(limit) });
	if (before != null) params.set('before', String(before));
	return get<AuditPage>(`/audit?${params}`);
}
