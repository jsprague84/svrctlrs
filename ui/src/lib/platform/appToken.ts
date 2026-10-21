/**
 * The desktop app's session token (ROADMAP 22). It lives in the OS keyring, through the Tauri
 * commands in `src-tauri/src/auth.rs`, and in memory while the app runs; never in
 * localStorage, where any script on the page could read it. The browser has no token: its
 * HttpOnly session cookie is the credential.
 */
import { getServerUrl, isTauri } from './index.js';

const LEGACY_KEY = 'svrctlrs-session-token';

let token: string | null = null;

async function invoke<T>(command: string, args: Record<string, unknown>): Promise<T> {
	const core = await import('@tauri-apps/api/core');
	return core.invoke<T>(command, args);
}

/** The token for API calls and WebSocket tickets, or null (the browser, or not signed in). */
export function getAppToken(): string | null {
	return token;
}

/** Load the token from the keyring at start-up. A token an older build left in localStorage
 *  moves to the keyring (or stays only in memory if there is no keyring) and is removed. */
export async function loadAppToken(): Promise<void> {
	if (!isTauri()) return;
	const serverUrl = getServerUrl();
	if (!serverUrl) return;
	const legacy = localStorage.getItem(LEGACY_KEY);
	if (legacy) {
		localStorage.removeItem(LEGACY_KEY);
		token = legacy;
		await invoke('store_token', { serverUrl, token: legacy }).catch(() => {});
		return;
	}
	token = (await invoke<string | null>('stored_token', { serverUrl }).catch(() => null)) ?? null;
}

/** Sign in through the system browser; resolves once the browser has handed the app its
 *  session (the Rust side keeps it in the keyring). */
export async function signInWithBrowser(): Promise<void> {
	token = await invoke<string>('sign_in_with_browser', { serverUrl: getServerUrl() });
}

/** Forget the token here and in the keyring (sign-out, or a change of server). */
export async function forgetAppToken(serverUrl = getServerUrl()): Promise<void> {
	token = null;
	localStorage.removeItem(LEGACY_KEY);
	if (isTauri() && serverUrl) await invoke('forget_token', { serverUrl }).catch(() => {});
}
