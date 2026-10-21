import { getServerUrl } from '#lib/platform/index.js';

function getApiBase(): string {
	return `${getServerUrl()}/api/v1`;
}

/** Auth headers for the native apps: a Bearer token (the session id) stored at login.
 * The browser has no token and authenticates with the session cookie instead.
 * The server derives the user from the session; nothing else identifies the caller. */
function getAuthHeaders(): Record<string, string> {
	if (typeof localStorage === 'undefined') return {};
	const token = localStorage.getItem('svrctlrs-session-token');
	return token ? { Authorization: `Bearer ${token}` } : {};
}

export class ApiError extends Error {
	constructor(
		public status: number,
		public statusText: string,
		public body?: unknown
	) {
		super(`${status} ${statusText}`);
		this.name = 'ApiError';
	}
}

/** Where a 401 or signing out sends the user. In the browser that is the server's own login
 *  page (password, two-factor, "Sign in with …" for an identity provider), which only a full
 *  page load reaches: a client-side `goto` would render the SPA's `/auth/login` route, the
 *  native apps' password-only form. After signing out, `?signed_out=1` keeps the page from
 *  sending you straight back to the provider. */
async function toLogin(signedOut = false): Promise<void> {
	const path = signedOut ? '/auth/login?signed_out=1' : '/auth/login';
	if (!getServerUrl()) {
		window.location.href = path;
		return;
	}
	try {
		const { goto } = await import('$app/navigation');
		goto('/auth/login');
	} catch {
		window.location.href = '/auth/login';
	}
}

const EDGE_RELOAD_KEY = 'svrctlrs-edge-reload';

/** Behind a login proxy (the dev-space edge), an expired sign-in there turns every API call
 *  into a redirect to the proxy's login page. The API itself never redirects, so a redirect
 *  means: reload the page and let the proxy sign the user in again. At most once in 30 s, so a
 *  proxy that keeps redirecting cannot put the page in a loop. */
function reloadForEdgeSignIn(): void {
	try {
		const last = Number(sessionStorage.getItem(EDGE_RELOAD_KEY) ?? 0);
		if (Date.now() - last < 30_000) return;
		sessionStorage.setItem(EDGE_RELOAD_KEY, String(Date.now()));
	} catch {
		// Storage blocked: reload anyway
	}
	window.location.reload();
}

/** One API call with the app's credentials. Throws `ApiError` on anything but a 2xx. */
async function send(path: string, options: RequestInit, headers: Record<string, string>) {
	// Only the web app sits behind the proxy; the native apps talk to the server directly
	const web = !getServerUrl();
	const res = await fetch(`${getApiBase()}${path}`, {
		credentials: 'include',
		...(web ? { redirect: 'manual' as const } : {}),
		...options,
		headers
	});

	if (web && res.type === 'opaqueredirect') {
		reloadForEdgeSignIn();
		throw new ApiError(401, 'Sign-in expired');
	}
	if (res.status === 401) {
		await toLogin();
		throw new ApiError(401, 'Unauthorized');
	}
	if (!res.ok) {
		let body: unknown;
		try {
			body = await res.json();
		} catch {
			body = { error: res.statusText };
		}
		throw new ApiError(res.status, res.statusText, body);
	}
	return res;
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
	const res = await send(path, options, {
		'Content-Type': 'application/json',
		...getAuthHeaders(),
		...(options.headers as Record<string, string> | undefined)
	});
	if (res.status === 204) return undefined as T;
	return res.json();
}

/** A raw fetch with the app's credentials and 401 handling, for bodies that are not JSON
 *  (file downloads and uploads). Throws `ApiError` on a non-2xx status. */
export function authorizedFetch(path: string, options: RequestInit = {}): Promise<Response> {
	return send(path, options, {
		...getAuthHeaders(),
		...(options.headers as Record<string, string> | undefined)
	});
}

/** Ask the server whether this browser is still signed in, for a caller that only saw a
 *  failure it cannot read (a WebSocket refused before it opened). A sign-in that has run out,
 *  in the app or at the proxy, is handled as for any API call; anything else is ignored. */
export async function checkSignIn(): Promise<void> {
	try {
		await send('/sessions', {}, getAuthHeaders());
	} catch {
		// Handled in send(), or the server is just unreachable
	}
}

export function get<T>(path: string): Promise<T> {
	return request<T>(path);
}

export function post<T>(path: string, body?: unknown): Promise<T> {
	return request<T>(path, {
		method: 'POST',
		body: body ? JSON.stringify(body) : undefined
	});
}

export function put<T>(path: string, body?: unknown): Promise<T> {
	return request<T>(path, {
		method: 'PUT',
		body: body ? JSON.stringify(body) : undefined
	});
}

export function del<T>(path: string): Promise<T> {
	return request<T>(path, { method: 'DELETE' });
}

export async function logout(): Promise<void> {
	try {
		await fetch(`${getServerUrl()}/auth/logout`, {
			method: 'POST',
			credentials: 'include',
			headers: { ...getAuthHeaders() }
		});
	} catch (e) {
		console.warn('Logout request failed:', e);
	}
	localStorage.removeItem('svrctlrs-session-token');
	localStorage.removeItem('svrctlrs-user-id');
	await toLogin(true);
}
