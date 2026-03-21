/**
 * Biometric authentication wrapper.
 * Uses @tauri-apps/plugin-biometric on Tauri, no-op on web.
 */

import { isTauri } from './index.js';

export interface BiometricResult {
	success: boolean;
	error?: string;
}

/** Check if biometric authentication is available on this device */
export async function checkBiometricAvailability(): Promise<boolean> {
	if (!isTauri()) return false;

	try {
		const { checkStatus } = await import('@tauri-apps/plugin-biometric');
		const status = await checkStatus();
		return status.isAvailable;
	} catch {
		return false;
	}
}

/** Prompt for biometric authentication (fingerprint, face, or device PIN fallback) */
export async function authenticate(reason = 'Unlock SvrCtlRS'): Promise<BiometricResult> {
	if (!isTauri()) {
		return { success: false, error: 'Biometrics not available on web' };
	}

	try {
		const { authenticate: bioAuth } = await import('@tauri-apps/plugin-biometric');
		await bioAuth(reason, { allowDeviceCredential: true });
		return { success: true };
	} catch (e) {
		return {
			success: false,
			error: e instanceof Error ? e.message : 'Authentication failed'
		};
	}
}
