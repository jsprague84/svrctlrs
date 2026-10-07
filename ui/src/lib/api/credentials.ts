import { get, post, put, del } from './client.js';
import type { Credential, CreateCredential, UpdateCredential } from '#lib/types/index.js';

interface CredentialsListResponse {
	credentials: Credential[];
}

interface MutationResponse {
	id: number;
	message: string;
}

export async function listCredentials(): Promise<Credential[]> {
	const res = await get<CredentialsListResponse>('/credentials');
	return res.credentials;
}

export async function getCredential(id: number): Promise<Credential> {
	return get<Credential>(`/credentials/${id}`);
}

export async function createCredential(input: CreateCredential): Promise<number> {
	const res = await post<MutationResponse>('/credentials', input);
	return res.id;
}

export async function updateCredential(id: number, input: UpdateCredential): Promise<void> {
	await put<MutationResponse>(`/credentials/${id}`, input);
}

export async function deleteCredential(id: number): Promise<void> {
	await del<MutationResponse>(`/credentials/${id}`);
}

/** Generate an ed25519 key on the server; the private half is stored encrypted. */
export async function generateKey(input: {
	name: string;
	description?: string;
	comment?: string;
}): Promise<{ id: number; public_key: string }> {
	return post<{ id: number; public_key: string }>('/credentials/generate-key', input);
}
