import { get, post, put, del } from './client.js';
import type { Credential, CreateCredential, UpdateCredential } from '$lib/types/index.js';

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
