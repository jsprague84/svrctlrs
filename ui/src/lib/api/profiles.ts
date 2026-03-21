import { get, post, put, del } from './client.js';
import type { TerminalProfile, CreateProfile, UpdateProfile } from '$lib/types/index.js';

interface ProfilesListResponse {
	profiles: TerminalProfile[];
}

interface ProfileResponse {
	profile: TerminalProfile;
}

interface MutationResponse {
	id: number;
	message: string;
}

export async function listProfiles(): Promise<TerminalProfile[]> {
	const res = await get<ProfilesListResponse>('/profiles');
	return res.profiles;
}

export async function getProfile(id: number): Promise<TerminalProfile> {
	const res = await get<ProfileResponse>(`/profiles/${id}`);
	return res.profile;
}

export async function createProfile(input: CreateProfile): Promise<number> {
	const res = await post<MutationResponse>('/profiles', input);
	return res.id;
}

export async function updateProfile(id: number, input: UpdateProfile): Promise<void> {
	await put<MutationResponse>(`/profiles/${id}`, input);
}

export async function deleteProfile(id: number): Promise<void> {
	await del<MutationResponse>(`/profiles/${id}`);
}
