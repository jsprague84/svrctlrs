import { get, post, del } from './client.js';

/** A browser or desktop-app sign-in of the caller's account */
export interface Device {
	id: number;
	/** "Firefox on Android", "Desktop app on Linux" */
	label: string;
	/** password, password+totp, password+recovery_code, oidc, app or unknown */
	method: string;
	/** The device this page runs on */
	current: boolean;
	created_at: string;
	last_seen_at: string;
	last_ip: string | null;
}

/** This device first, then the most recently used */
export async function listDevices(): Promise<Device[]> {
	const res = await get<{ devices: Device[] }>('/users/me/devices');
	return res.devices;
}

export async function signOutDevice(id: number): Promise<void> {
	await del(`/users/me/devices/${id}`);
}

/** Every device but this one; the answer is how many */
export async function signOutOtherDevices(): Promise<number> {
	const res = await post<{ signed_out: number }>('/users/me/devices/sign-out-others');
	return res.signed_out;
}
