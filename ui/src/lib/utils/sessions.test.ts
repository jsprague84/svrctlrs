import { describe, expect, it } from 'vitest';
import type { LiveSession } from '#lib/types/index.js';
import { classifySessions, describeSession, formatTtl } from './sessions.js';

const now = new Date('2026-10-11T12:00:00Z');

function session(id: string, minutesAgo: number, attachments = 0): LiveSession {
	return {
		id,
		server_id: 1,
		server_name: 'web',
		created_at: new Date(now.getTime() - minutesAgo * 60_000).toISOString(),
		attachments,
		latest_seq: 10
	};
}

describe('classifySessions', () => {
	it('separates sessions held by a tab here from the rest, newest first', () => {
		const held = new Map([['b', 'term-2']]);
		const { resumable, open } = classifySessions(
			[session('a', 30), session('b', 5), session('c', 1), session('d', 10)],
			held
		);
		expect(resumable.map((r) => r.session.id)).toEqual(['c', 'd', 'a']);
		expect(resumable.every((r) => r.tabId === null)).toBe(true);
		expect(open).toEqual([{ session: session('b', 5), tabId: 'term-2' }]);
	});

	it('handles an empty list', () => {
		expect(classifySessions([], new Map())).toEqual({ resumable: [], open: [] });
	});
});

describe('describeSession', () => {
	it('says how old the shell is and who holds it', () => {
		expect(describeSession({ session: session('a', 5), tabId: null }, now)).toBe(
			'started 5 min ago · detached'
		);
		expect(describeSession({ session: session('a', 5, 1), tabId: null }, now)).toBe(
			'started 5 min ago · attached elsewhere'
		);
		expect(describeSession({ session: session('a', 90, 2), tabId: null }, now)).toBe(
			'started 2 h ago · attached elsewhere ×2'
		);
		expect(describeSession({ session: session('a', 0, 1), tabId: 'term-1' }, now)).toBe(
			'started just now · open here'
		);
	});

	it('says when a kept shell is waiting in tmux after a restart', () => {
		const kept = { ...session('a', 5), persistent: true, waiting: true };
		expect(describeSession({ session: kept, tabId: null }, now)).toBe(
			'started 5 min ago · waiting in tmux'
		);
	});
});

describe('formatTtl', () => {
	it('picks the largest whole unit', () => {
		expect(formatTtl(900)).toBe('15 min');
		expect(formatTtl(3600)).toBe('1 h');
		expect(formatTtl(28800)).toBe('8 h');
		expect(formatTtl(90)).toBe('90 s');
		expect(formatTtl(0)).toBe('');
	});
});
