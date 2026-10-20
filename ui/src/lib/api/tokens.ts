import { get, post, put, del } from './client.js';

export type TokenPreset = 'full_admin' | 'servers_keys' | 'read_only';
export type TokenStatus = 'active' | 'expired' | 'revoked';

export interface ApiToken {
	id: number;
	name: string;
	/** `svr_` and the secret's first characters, to tell tokens apart */
	prefix: string;
	preset: TokenPreset;
	status: TokenStatus;
	created_at: string;
	/** null: never expires */
	expires_at: string | null;
	last_used_at: string | null;
	last_used_ip: string | null;
	revoked_at: string | null;
	/** Deleting a server or credential, or forgetting host keys, waits for your approval */
	require_approval: boolean;
}

export interface CreatedToken {
	token: ApiToken;
	/** The whole token. The server returns it in this response only. */
	secret: string;
}

export const PRESETS: Array<{ id: TokenPreset; label: string; detail: string }> = [
	{
		id: 'full_admin',
		label: 'Full admin',
		detail: 'Everything you can do, terminals and files included. Cannot manage tokens or accounts.'
	},
	{
		id: 'servers_keys',
		label: 'Servers & keys',
		detail:
			'Read everything; add, change and delete servers, credentials and host keys. No terminals or files.'
	},
	{
		id: 'read_only',
		label: 'Read-only',
		detail: 'Read servers, keys, settings and the audit log. No changes, terminals or files.'
	}
];

/** Lifetimes the create dialog offers; null is "never" */
export const LIFETIMES: Array<{ days: number | null; label: string }> = [
	{ days: 30, label: '30 days' },
	{ days: 90, label: '90 days' },
	{ days: 365, label: '1 year' },
	{ days: null, label: 'Never' }
];

export function presetLabel(preset: string): string {
	return PRESETS.find((p) => p.id === preset)?.label ?? preset;
}

export async function listTokens(): Promise<ApiToken[]> {
	const res = await get<{ tokens: ApiToken[] }>('/tokens');
	return res.tokens;
}

export function createToken(
	name: string,
	preset: TokenPreset,
	expiresInDays: number | null,
	requireApproval: boolean
): Promise<CreatedToken> {
	return post<CreatedToken>('/tokens', {
		name,
		preset,
		expires_in_days: expiresInDays,
		require_approval: requireApproval
	});
}

export async function setRequireApproval(id: number, on: boolean): Promise<ApiToken> {
	const res = await put<{ token: ApiToken }>(`/tokens/${id}`, { require_approval: on });
	return res.token;
}

export async function revokeToken(id: number): Promise<ApiToken> {
	const res = await post<{ token: ApiToken }>(`/tokens/${id}/revoke`);
	return res.token;
}

export async function deleteToken(id: number): Promise<void> {
	await del(`/tokens/${id}`);
}
