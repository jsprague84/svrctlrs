import type { Credential, CreateCredential, UpdateCredential } from '$lib/types/index.js';
import * as api from '$lib/api/credentials.js';
import { extractErrorMessage } from '$lib/utils/error.js';

let credentials = $state<Credential[]>([]);
let loading = $state(false);
let error = $state<string | null>(null);

export function getCredentials() {
	return credentials;
}

export function isLoading() {
	return loading;
}

export function getError() {
	return error;
}

export async function loadCredentials() {
	loading = true;
	error = null;
	try {
		credentials = await api.listCredentials();
	} catch (e) {
		error = extractErrorMessage(e, 'Failed to load credentials');
	} finally {
		loading = false;
	}
}

export async function createCredential(input: CreateCredential): Promise<number> {
	const id = await api.createCredential(input);
	await loadCredentials();
	return id;
}

export async function updateCredential(id: number, input: UpdateCredential): Promise<void> {
	await api.updateCredential(id, input);
	await loadCredentials();
}

export async function deleteCredential(id: number): Promise<void> {
	await api.deleteCredential(id);
	await loadCredentials();
}
