import { describe, expect, it } from 'vitest';
import { wrapFocus } from './overlay.js';

describe('wrapFocus', () => {
	const items = ['a', 'b', 'c'];

	it('wraps only at the ends', () => {
		expect(wrapFocus(items, 'c', false)).toBe('a');
		expect(wrapFocus(items, 'a', true)).toBe('c');
		expect(wrapFocus(items, 'a', false)).toBeNull();
		expect(wrapFocus(items, 'b', true)).toBeNull();
	});

	it('pulls focus that escaped back in', () => {
		expect(wrapFocus(items, 'elsewhere', false)).toBe('a');
		expect(wrapFocus(items, null, true)).toBe('c');
	});

	it('has nothing to do without focusable items', () => {
		expect(wrapFocus([], null, false)).toBeNull();
	});
});
