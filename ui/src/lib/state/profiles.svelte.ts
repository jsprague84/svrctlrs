import type { TerminalProfile, CreateProfile, UpdateProfile } from '$lib/types/index.js';
import * as api from '$lib/api/profiles.js';
import { extractErrorMessage } from '$lib/utils/error.js';

let profiles = $state<TerminalProfile[]>([]);
let loading = $state(false);
let error = $state<string | null>(null);

export function getProfiles() {
	return profiles;
}

export function isLoading() {
	return loading;
}

export function getError() {
	return error;
}

export function getDefaultProfile(): TerminalProfile | undefined {
	return profiles.find((p) => p.is_default);
}

export async function loadProfiles() {
	loading = true;
	error = null;
	try {
		profiles = await api.listProfiles();
	} catch (e) {
		error = extractErrorMessage(e, 'Failed to load profiles');
	} finally {
		loading = false;
	}
}

export async function createProfile(input: CreateProfile): Promise<number> {
	const id = await api.createProfile(input);
	await loadProfiles();
	return id;
}

export async function updateProfile(id: number, input: UpdateProfile): Promise<void> {
	await api.updateProfile(id, input);
	await loadProfiles();
}

export async function deleteProfile(id: number): Promise<void> {
	await api.deleteProfile(id);
	await loadProfiles();
}
