import { get, post, put, del } from './client.js';
import type { Server, ServerWithDetails, CreateServer, UpdateServer, TestConnectionResult } from '$lib/types/index.js';

interface ServersListResponse {
	servers: Server[];
}

interface ServerDetailResponse {
	server: Server;
	tags: string[];
	capabilities: Array<{
		id: number;
		server_id: number;
		capability: string;
		available: boolean;
		version: string | null;
		detected_at: string;
	}>;
}

interface MutationResponse {
	id: number;
	message: string;
}

export async function listServers(): Promise<Server[]> {
	const res = await get<ServersListResponse>('/servers');
	return res.servers;
}

export async function getServer(id: number): Promise<ServerWithDetails> {
	const res = await get<ServerDetailResponse>(`/servers/${id}`);
	return {
		server: res.server,
		tags: res.tags,
		capabilities: res.capabilities
	};
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

export async function testConnection(id: number): Promise<TestConnectionResult> {
	return post<TestConnectionResult>(`/servers/${id}/test`);
}
