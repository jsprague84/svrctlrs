import { get, post, put, del } from './client.js';
import type {
	Server,
	CreateServer,
	UpdateServer,
	TestConnectionResult,
	HostKey,
	ImportResult
} from '#lib/types/index.js';

interface ServersListResponse {
	servers: Server[];
}

interface ServerDetailResponse {
	server: Server;
}

interface MutationResponse {
	id: number;
	message: string;
}

export async function listServers(): Promise<Server[]> {
	const res = await get<ServersListResponse>('/servers');
	return res.servers;
}

export async function getServer(id: number): Promise<Server> {
	const res = await get<ServerDetailResponse>(`/servers/${id}`);
	return res.server;
}

export async function createServer(input: CreateServer): Promise<number> {
	const res = await post<MutationResponse>('/servers', input);
	return res.id;
}

export async function updateServer(id: number, input: UpdateServer): Promise<void> {
	await put<MutationResponse>(`/servers/${id}`, input);
}

export async function deleteServer(id: number): Promise<void> {
	await del<MutationResponse>(`/servers/${id}`);
}

/** Create servers from the Host blocks of a pasted ssh_config. */
export async function importServers(input: {
	config: string;
	credential_id?: number;
	tags?: string[];
}): Promise<ImportResult> {
	return post<ImportResult>('/servers/import', input);
}

export async function testConnection(id: number): Promise<TestConnectionResult> {
	return post<TestConnectionResult>(`/servers/${id}/test`);
}

export async function listHostKeys(id: number): Promise<HostKey[]> {
	const res = await get<{ server_id: number; host_keys: HostKey[] }>(`/servers/${id}/host-keys`);
	return res.host_keys;
}

/** Trust the key recorded on first contact, by the fingerprint the user compared. */
export async function trustHostKey(id: number, fingerprint: string): Promise<HostKey> {
	const res = await post<{ server_id: number; host_key: HostKey }>(
		`/servers/${id}/host-keys/trust`,
		{ fingerprint }
	);
	return res.host_key;
}

/** Forget every recorded host key; the next connection asks for confirmation again. */
export async function forgetHostKeys(id: number): Promise<number> {
	const res = await del<{ server_id: number; removed: number }>(`/servers/${id}/host-keys`);
	return res.removed;
}

/** Install a key credential's public half on the server with one-time bootstrap access. */
export async function authorizeKey(
	id: number,
	input: { credential_id: number; password?: string; bootstrap_credential_id?: number }
): Promise<{ success: boolean; message: string }> {
	return post<{ success: boolean; message: string }>(`/servers/${id}/authorize-key`, input);
}

export interface DiagnosisStep {
	/** config, dns, tcp, jump_host, tunnel, handshake, host_key or login */
	step: string;
	status: 'ok' | 'failed' | 'skipped';
	detail: string;
	fix: string | null;
	duration_ms: number;
}

export interface DiagnosisHop {
	server_id: number;
	server_name: string;
	address: string;
	via_server_id: number | null;
	steps: DiagnosisStep[];
}

export interface Diagnosis {
	server_id: number;
	server_name: string;
	ok: boolean;
	verdict: string;
	/** Jump hosts first, the server last */
	hops: DiagnosisHop[];
	duration_ms: number;
}

/** Why svrctlrs can or cannot reach the server, step by step, from where it runs. */
export function diagnose(id: number): Promise<Diagnosis> {
	return post<Diagnosis>(`/servers/${id}/diagnose`);
}

export interface OpStep {
	step: string;
	status: 'ok' | 'failed' | 'skipped' | 'waiting' | 'planned';
	detail: string;
}

export interface RotateKeyResult {
	server_id: number;
	done: boolean;
	dry_run: boolean;
	old_credential_id: number | null;
	new_credential_id: number | null;
	steps: OpStep[];
	next: string | null;
}

/** New key generated here, installed with the current one, proved, switched to; the old
 *  public key removed from the server. */
export function rotateKey(id: number): Promise<RotateKeyResult> {
	return post<RotateKeyResult>(`/servers/${id}/rotate-key`, {});
}
