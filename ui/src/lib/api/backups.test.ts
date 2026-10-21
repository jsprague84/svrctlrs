import { describe, expect, it } from 'vitest';
import { utcHourLabel } from './backups.js';

describe('utcHourLabel', () => {
	it('names the UTC hour, and the local time when it differs', () => {
		const on = new Date('2026-10-22T12:00:00Z');
		for (const h of [0, 3, 23]) {
			const label = utcHourLabel(h, on);
			const utc = `${String(h).padStart(2, '0')}:00 UTC`;
			expect(label.startsWith(utc)).toBe(true);
			expect(label === utc || /^\d\d:00 UTC \(\d\d:\d\d here\)$/.test(label)).toBe(true);
		}
	});
});
