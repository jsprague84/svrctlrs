import { get, post, put, del } from './client.js';
import type { TerminalPreferences } from '#lib/types/index.js';

export interface UserAccount {
	id: number;
	username: string;
	is_me: boolean;
	/** Most recent successful login from the audit log, if any */
	last_login_at: string | null;
	created_at: string;
	updated_at: string;
}

export async function listUsers(): Promise<UserAccount[]> {
	const res = await get<{ users: UserAccount[] }>('/users');
	return res.users;
}

export async function createUser(username: string, password: string): Promise<number> {
	const res = await post<{ id: number }>('/users', { username, password });
	return res.id;
}

export async function deleteUser(id: number): Promise<void> {
	await del(`/users/${id}`);
}

export async function changePassword(currentPassword: string, newPassword: string): Promise<void> {
	await put('/users/me/password', { current_password: currentPassword, new_password: newPassword });
}

/** Preference documents an account keeps on the server */
export interface PreferenceDocuments {
	terminal: TerminalPreferences;
}

/** The caller's own preference document, or null when they never saved one. Untrusted in
 *  shape (another client version wrote it): merge before use. */
export async function getPreference(key: keyof PreferenceDocuments): Promise<unknown> {
	const res = await get<{ value: unknown }>(`/users/me/preferences/${key}`);
	return res.value ?? null;
}

export async function putPreference<K extends keyof PreferenceDocuments>(
	key: K,
	value: PreferenceDocuments[K]
): Promise<void> {
	await put(`/users/me/preferences/${key}`, { value });
}
