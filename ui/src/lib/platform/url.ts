/**
 * Normalise a server URL typed by a user: trim, default to https, strip trailing slashes.
 * Returns an error message instead of a URL when the input cannot be used.
 */
export function normalizeServerUrl(
	input: string,
	platform: 'web' | 'tauri-desktop' | 'tauri-mobile'
): { url: string } | { error: string } {
	let value = input.trim();
	if (!value) return { error: 'Enter the server URL.' };
	if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(value)) value = `https://${value}`;

	let parsed: URL;
	try {
		parsed = new URL(value);
	} catch {
		return { error: 'That is not a valid URL.' };
	}
	if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') {
		return { error: 'The URL must start with https:// or http://.' };
	}
	if (parsed.protocol === 'http:' && platform === 'tauri-mobile') {
		return {
			error: 'The Android app only talks to https:// servers (release builds refuse plain http).'
		};
	}
	if (parsed.username || parsed.password) {
		return { error: 'Do not put credentials in the URL; you will log in on the next screen.' };
	}
	return { url: `${parsed.protocol}//${parsed.host}${parsed.pathname.replace(/\/+$/, '')}` };
}
