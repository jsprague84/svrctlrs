import { get, post, put, del } from './client.js';

export interface UserAccount {
	id: number;
	username: string;
	is_me: boolean;
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
