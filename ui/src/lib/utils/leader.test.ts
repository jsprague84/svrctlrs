import { describe, expect, it } from 'vitest';
import { isLeaderChord, leaderAction, LEADER_BINDINGS, type KeyChord } from './leader.js';

const chord = (over: Partial<KeyChord>): KeyChord => ({
	key: '',
	ctrlKey: false,
	shiftKey: false,
	altKey: false,
	metaKey: false,
	...over
});

describe('isLeaderChord', () => {
	it('matches exactly the configured chord', () => {
		expect(isLeaderChord(chord({ key: ' ', ctrlKey: true }), 'ctrl-space')).toBe(true);
		expect(isLeaderChord(chord({ key: ' ', ctrlKey: true, shiftKey: true }), 'ctrl-space')).toBe(
			false
		);
		expect(
			isLeaderChord(chord({ key: ' ', ctrlKey: true, shiftKey: true }), 'ctrl-shift-space')
		).toBe(true);
		expect(isLeaderChord(chord({ key: ' ', altKey: true }), 'alt-space')).toBe(true);
		expect(isLeaderChord(chord({ key: ' ', altKey: true, ctrlKey: true }), 'alt-space')).toBe(
			false
		);
	});

	it('accepts the Space code when the key is reported differently', () => {
		expect(
			isLeaderChord(chord({ key: 'Spacebar', code: 'Space', ctrlKey: true }), 'ctrl-space')
		).toBe(true);
	});

	it('never arms when off, on Cmd, or on other keys', () => {
		expect(isLeaderChord(chord({ key: ' ', ctrlKey: true }), 'off')).toBe(false);
		expect(isLeaderChord(chord({ key: ' ', ctrlKey: true, metaKey: true }), 'ctrl-space')).toBe(
			false
		);
		expect(isLeaderChord(chord({ key: 'b', ctrlKey: true }), 'ctrl-space')).toBe(false);
	});
});

describe('leaderAction', () => {
	it('ignores bare modifier presses so Ctrl can be released after the chord', () => {
		expect(leaderAction(chord({ key: 'Control', ctrlKey: true }))).toBeNull();
		expect(leaderAction(chord({ key: 'Shift', shiftKey: true }))).toBeNull();
	});

	it('maps the tmux-style keys', () => {
		expect(leaderAction(chord({ key: 'c' }))).toEqual({ type: 'new-tab' });
		expect(leaderAction(chord({ key: 'x' }))).toEqual({ type: 'close-tab' });
		expect(leaderAction(chord({ key: 'n' }))).toEqual({ type: 'next-tab' });
		expect(leaderAction(chord({ key: 'n', shiftKey: true }))).toEqual({ type: 'prev-tab' });
		expect(leaderAction(chord({ key: 'p' }))).toEqual({ type: 'prev-tab' });
		expect(leaderAction(chord({ key: '3' }))).toEqual({ type: 'goto-tab', index: 2 });
		expect(leaderAction(chord({ key: '`' }))).toEqual({ type: 'last-tab' });
		expect(leaderAction(chord({ key: 'h' }))).toEqual({ type: 'focus', direction: 'left' });
		expect(leaderAction(chord({ key: 'ArrowDown' }))).toEqual({ type: 'focus', direction: 'down' });
		expect(leaderAction(chord({ key: ']' }))).toEqual({ type: 'next-pane' });
		expect(leaderAction(chord({ key: 's' }))).toEqual({ type: 'cycle-layout' });
		expect(leaderAction(chord({ key: '/' }))).toEqual({ type: 'search' });
		expect(leaderAction(chord({ key: ':' }))).toEqual({ type: 'palette' });
		expect(leaderAction(chord({ key: 'Escape' }))).toEqual({ type: 'cancel' });
	});

	it('cancels on another chord and reports unbound keys as null', () => {
		expect(leaderAction(chord({ key: 'c', ctrlKey: true }))).toEqual({ type: 'cancel' });
		expect(leaderAction(chord({ key: 'z' }))).toBeNull();
	});

	it('documents every bound key in the overlay table', () => {
		const shown = LEADER_BINDINGS.map((b) => b.keys).join(' ');
		for (const k of ['c', 'x', 'n', 'p', 'h', 's', '/', ':', '[', ']', '`']) {
			expect(shown).toContain(k);
		}
	});
});
