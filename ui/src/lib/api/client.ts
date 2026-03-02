const API_BASE = '/api/v1';

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
	const url = `${API_BASE}${path}`;
	const res = await fetch(url, {
		credentials: 'same-origin',
		headers: {
			'Content-Type': 'application/json',
			...options.headers
		},
		...options
	});

	if (res.status === 401) {
		window.location.href = '/auth/login';
		throw new ApiError(401, 'Unauthorized');
	}

	if (!res.ok) {
		let body: unknown;
		try {
			body = await res.json();
		} catch {
			// ignore parse errors
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
		await fetch('/auth/logout', { method: 'POST', credentials: 'same-origin' });
	} catch {
		// ignore errors
	}
	window.location.href = '/auth/login';
}
