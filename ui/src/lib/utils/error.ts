import { ApiError } from '$lib/api/client.js';

/** Extract a human-readable error message from any error type */
export function extractErrorMessage(e: unknown, fallback = 'An unexpected error occurred'): string {
	if (e instanceof ApiError && e.body && typeof e.body === 'object') {
		const body = e.body as Record<string, unknown>;
		// Handle { error: { message: "..." } } from backend
		if (body.error && typeof body.error === 'object') {
			const err = body.error as Record<string, unknown>;
			if (typeof err.message === 'string') return err.message;
		}
		// Handle { message: "..." } from backend
		if (typeof body.message === 'string') return body.message;
	}
	if (e instanceof Error) return e.message;
	if (typeof e === 'string') return e;
	return fallback;
}
