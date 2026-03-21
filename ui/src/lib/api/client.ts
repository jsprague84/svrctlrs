import { getServerUrl, isTauri } from '$lib/platform/index.js';

function getApiBase(): string {
	return `${getServerUrl()}/api/v1`;
}

/** Get auth headers — includes Bearer token + user ID if available.
 * In web mode with cookies, server checks cookies first — these are ignored.
 * In Tauri mode without cookies, server uses token + user_id for auth. */
function getAuthHeaders(): Record<string, string> {
	if (typeof localStorage === 'undefined') return {};
	const token = localStorage.getItem('svrctlrs-session-token');
	if (!token) return {};
	const headers: Record<string, string> = { Authorization: `Bearer ${token}` };
	const userId = localStorage.getItem('svrctlrs-user-id');
	if (userId) headers['X-User-Id'] = userId;
	return headers;
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
		headers: {
			'Content-Type': 'application/json',
			...getAuthHeaders(),
			...options.headers
		},
		...options
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
