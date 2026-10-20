import { get, post } from './client.js';

export type ApprovalStatus = 'pending' | 'expired' | 'denied' | 'approved' | 'done' | 'failed';

export interface Approval {
	id: string;
	status: ApprovalStatus;
	/** server.delete | credential.delete | server.host_keys_forget */
	action: string;
	/** "Delete server web-01" */
	summary: string;
	method: string;
	path: string;
	target_type: string;
	target_id: number;
	target_name: string | null;
	token_id: number | null;
	token_name: string;
	client_ip: string | null;
	url: string;
	created_at: string;
	expires_at: string;
	decided_at: string | null;
	/** What the approved call answered */
	result_status: number | null;
	result: unknown;
}

export async function listApprovals(): Promise<{ approvals: Approval[]; waiting: number }> {
	return get<{ approvals: Approval[]; waiting: number }>('/approvals');
}

export function getApproval(id: string): Promise<Approval> {
	return get<Approval>(`/approvals/${encodeURIComponent(id)}`);
}

export function approve(id: string): Promise<Approval> {
	return post<Approval>(`/approvals/${encodeURIComponent(id)}/approve`);
}

export function deny(id: string): Promise<Approval> {
	return post<Approval>(`/approvals/${encodeURIComponent(id)}/deny`);
}

/** The message a failed approved call came back with, if any */
export function resultMessage(a: Approval): string | null {
	const r = a.result as { error?: { message?: string } | string; message?: string } | null;
	if (!r || typeof r !== 'object') return null;
	if (typeof r.error === 'string') return r.error;
	return r.error?.message ?? r.message ?? null;
}
