import { post } from '#lib/api/client.js';
import { getServerUrl } from '#lib/platform/index.js';

export const PTY_PATH = '/ws/terminal/pty';

export interface SocketUrlInputs {
	/** The configured backend for the native apps; '' means same origin */
	serverUrl: string;
	location: { protocol: string; host: string };
	/** The native apps' session token; the browser has none and uses its cookie */
	token: string | null;
	/** `POST /auth/ws-ticket` */
	fetchTicket: () => Promise<string>;
}

/**
 * The PTY WebSocket URL. The browser relies on the session cookie. Native apps cannot set
 * headers on a WebSocket handshake, so they exchange their token for a one-time ticket that is
 * bound to them and expires in seconds — nothing durable ends up in the URL.
 */
export async function ptySocketUrl(inputs: SocketUrlInputs): Promise<string> {
	let url: string;
	if (inputs.serverUrl) {
		url = `${inputs.serverUrl.replace(/^http/, 'ws')}${PTY_PATH}`;
	} else {
		const protocol = inputs.location.protocol === 'https:' ? 'wss:' : 'ws:';
		url = `${protocol}//${inputs.location.host}${PTY_PATH}`;
	}
	if (inputs.token) {
		try {
			const ticket = await inputs.fetchTicket();
			url += `?ticket=${encodeURIComponent(ticket)}`;
		} catch {
			// Server without ticket support: fall back to the token (deprecated).
			url += `?token=${encodeURIComponent(inputs.token)}`;
		}
	}
	return url;
}

/** `ptySocketUrl` for this page: its location, the stored token, the real ticket call. */
export function browserPtySocketUrl(): Promise<string> {
	return ptySocketUrl({
		serverUrl: getServerUrl(),
		location: window.location,
		token: localStorage.getItem('svrctlrs-session-token'),
		fetchTicket: async () => (await post<{ ticket: string }>('/auth/ws-ticket')).ticket
	});
}
