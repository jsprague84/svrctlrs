import { getServerUrl } from '$lib/platform/index.js';

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

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
	const url = `${getApiBase()}${path}`;
	const res = await fetch(url, {
		credentials: 'include',
		...options,
		headers: {
			'Content-Type': 'application/json',
			...getAuthHeaders(),
			...(options.headers as Record<string, string> | undefined)
		}
	});

	if (res.status === 401) {
		// Use SvelteKit goto if available, fall back to window.location for non-SPA contexts
		try {
			const { goto } = await import('$app/navigation');
			goto('/auth/login');
		} catch {
			window.location.href = '/auth/login';
		}
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

	if (res.status === 204) return undefined as T;
	return res.json();
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
	try {
		const { goto } = await import('$app/navigation');
		goto('/auth/login');
	} catch {
		window.location.href = '/auth/login';
	}
}
