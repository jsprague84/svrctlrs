/** Extract a human-readable error message from any error type */
export function extractErrorMessage(e: unknown, fallback = 'An unexpected error occurred'): string {
	if (e instanceof Error) return e.message;
	if (typeof e === 'string') return e;
	return fallback;
}
