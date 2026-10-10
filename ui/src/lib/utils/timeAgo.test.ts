import { describe, expect, it } from 'vitest';
import { timeAgo } from './timeAgo.js';

const now = new Date('2026-10-10T12:00:00Z');

describe('timeAgo', () => {
	it('rounds to the coarsest useful unit', () => {
		expect(timeAgo('2026-10-10T11:59:50Z', now)).toBe('just now');
		expect(timeAgo('2026-10-10T11:55:00Z', now)).toBe('5 min ago');
		expect(timeAgo('2026-10-10T09:00:00Z', now)).toBe('3 h ago');
		expect(timeAgo('2026-10-08T12:00:00Z', now)).toBe('2 d ago');
	});
	it('falls back to a date for anything older than two weeks', () => {
		expect(timeAgo('2026-09-01T12:00:00Z', now)).toBe(
			new Date('2026-09-01T12:00:00Z').toLocaleDateString()
		);
	});
	it('is empty for garbage', () => {
		expect(timeAgo('not a date', now)).toBe('');
	});
});
