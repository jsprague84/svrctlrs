/**
 * Platform detection layer for SvrCtlRS.
 * Detects whether the app is running in a browser, Tauri desktop, or Tauri mobile.
 */

/** Check if running inside Tauri (any platform) */
export function isTauri(): boolean {
	if (typeof window === 'undefined') return false;
	return '__TAURI_INTERNALS__' in window;
}

/** Check if running in a plain web browser */
export function isWeb(): boolean {
	return !isTauri();
}

/** Check if running in Tauri on a mobile device (iOS or Android) */
export function isTauriMobile(): boolean {
	if (!isTauri()) return false;
	const ua = navigator.userAgent;
	return /Android|iPhone|iPad|iPod/i.test(ua);
}

/** Check if running in Tauri on a desktop (macOS, Windows, Linux) */
export function isTauriDesktop(): boolean {
	return isTauri() && !isTauriMobile();
}

/** Get the current platform */
export function getPlatform(): 'web' | 'tauri-desktop' | 'tauri-mobile' {
	if (!isTauri()) return 'web';
	return isTauriMobile() ? 'tauri-mobile' : 'tauri-desktop';
}

/**
 * Get the configured backend server URL.
 * - Web mode: returns '' (empty string = same-origin, relative URLs)
 * - Tauri mode: returns the stored server URL from localStorage
 */
export function getServerUrl(): string {
	if (isWeb()) return '';
	if (typeof localStorage === 'undefined') return '';
	return localStorage.getItem('svrctlrs-server-url') ?? '';
}

/** Set the backend server URL (Tauri mode only) */
export function setServerUrl(url: string) {
	localStorage.setItem('svrctlrs-server-url', url.replace(/\/+$/, '')); // trim trailing slashes
}

/** Check if a server URL is configured (Tauri mode) */
export function hasServerUrl(): boolean {
	return getServerUrl() !== '';
}
