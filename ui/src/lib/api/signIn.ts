import { get, post, del } from './client.js';

/** How the caller signs in, and how this server lets anyone sign in */
export interface SignInMethods {
	/** Whether this server accepts a password at all (PASSWORD_LOGIN) */
	password_login: boolean;
	totp: { enabled: boolean; recovery_codes_left: number };
	/** The OpenID Connect provider, when one is configured */
	provider: { name: string; issuer: string } | null;
	identities: LinkedIdentity[];
}

export interface LinkedIdentity {
	id: number;
	issuer: string;
	email: string | null;
	display_name: string | null;
	created_at: string;
	last_used_at: string | null;
}

/** What to load into the authenticator app; shown once */
export interface TotpEnrolment {
	secret: string;
	otpauth_uri: string;
	qr_svg: string;
}

export function getSignInMethods(): Promise<SignInMethods> {
	return get<SignInMethods>('/users/me/sign-in');
}

export function beginTotp(password: string): Promise<TotpEnrolment> {
	return post<TotpEnrolment>('/users/me/totp', { password });
}

/** Turns two-factor on; the answer is the ten recovery codes, shown once */
export async function confirmTotp(code: string): Promise<string[]> {
	const res = await post<{ codes: string[] }>('/users/me/totp/confirm', { code });
	return res.codes;
}

export async function disableTotp(password: string): Promise<void> {
	await post('/users/me/totp/disable', { password });
}

export async function renewRecoveryCodes(password: string): Promise<string[]> {
	const res = await post<{ codes: string[] }>('/users/me/totp/recovery-codes', { password });
	return res.codes;
}

export async function unlinkIdentity(id: number): Promise<void> {
	await del(`/users/me/identities/${id}`);
}

/** Linking goes to the provider and back: a full-page navigation from this origin */
export const LINK_URL = '/auth/oidc/link';
