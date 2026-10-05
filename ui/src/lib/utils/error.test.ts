import { describe, expect, it } from 'vitest';
import { ApiError } from '$lib/api/client.js';
import { extractErrorMessage } from './error.js';

describe('extractErrorMessage', () => {
	it('prefers the API envelope message', () => {
		const e = new ApiError(404, 'Not Found', {
			success: false,
			error: { code: 'NOT_FOUND', message: 'Server not found' }
		});
		expect(extractErrorMessage(e)).toBe('Server not found');
	});

	it('accepts a flat message body', () => {
		expect(extractErrorMessage(new ApiError(400, 'Bad', { message: 'nope' }))).toBe('nope');
	});

	it('falls back through Error, string, and the default', () => {
		expect(extractErrorMessage(new Error('boom'))).toBe('boom');
		expect(extractErrorMessage('plain')).toBe('plain');
		expect(extractErrorMessage(42, 'fallback')).toBe('fallback');
		expect(extractErrorMessage(new ApiError(500, 'Oops'), 'fallback')).toBe('500 Oops');
	});
});
