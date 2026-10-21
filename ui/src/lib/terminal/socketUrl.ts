import { post } from '#lib/api/client.js';
import { getServerUrl } from '#lib/platform/index.js';
import { getAppToken } from '#lib/platform/appToken.js';

export const PTY_PATH = '/ws/terminal/pty';

export interface SocketUrlInputs {
	/** The configured backend for the native apps; '' means same origin */
	serverUrl: string;
	location: { protocol: string; host: string };
	/** The desktop app's session token; the browser has none and uses its cookie */
	token: string | null;
	/** `POST /auth/ws-ticket` */
	fetchTicket: () => Promise<string>;
}

/**
 * The PTY WebSocket URL. The browser relies on the session cookie. The desktop app cannot set
 * headers on a WebSocket handshake, so it exchanges its token for a one-time ticket that is
 * bound to it and expires in seconds — nothing durable ends up in the URL (the server refuses
 * a token there).
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
		const ticket = await inputs.fetchTicket();
		url += `?ticket=${encodeURIComponent(ticket)}`;
	}
	return url;
}

/** `ptySocketUrl` for this page: its location, the stored token, the real ticket call. */
export function browserPtySocketUrl(): Promise<string> {
	return ptySocketUrl({
		serverUrl: getServerUrl(),
		location: window.location,
		token: getAppToken(),
		fetchTicket: async () => (await post<{ ticket: string }>('/auth/ws-ticket')).ticket
	});
}
