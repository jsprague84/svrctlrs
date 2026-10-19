import { describe, expect, it } from 'vitest';
import {
	formatBytes,
	formatClock,
	indexAt,
	limitIdle,
	markers,
	parseCast,
	parseSize,
	steps,
	type CastEvent
} from './cast.js';

const header = '{"version":2,"width":80,"height":24,"timestamp":1760000000,"title":"web"}';

function cast(...events: unknown[]): string {
	return [header, ...events.map((e) => JSON.stringify(e))].join('\n') + '\n';
}

describe('parseCast', () => {
	it('reads the header and the events in order', () => {
		const c = parseCast(cast([0.1, 'o', '$ '], [1.5, 'r', '100x30'], [2, 'o', 'ls\r\n']));
		expect(c.header).toMatchObject({ version: 2, width: 80, height: 24, title: 'web' });
		expect(c.events.map((e) => e.kind)).toEqual(['o', 'r', 'o']);
		expect(c.duration).toBe(2);
	});

	it('refuses something that is not asciicast v2', () => {
		expect(() => parseCast('')).toThrow();
		expect(() => parseCast('hello')).toThrow();
		expect(() => parseCast('{"version":1,"width":80,"height":24}')).toThrow(/v2/);
		expect(() => parseCast('{"version":2,"width":0,"height":24}')).toThrow();
	});

	it('skips a cut-off last line and lines that are not events', () => {
		const text = cast([0.5, 'o', 'a'], { not: 'an event' }, [1, 'o', 'b']) + '[1.2, "o", "unfinish';
		const c = parseCast(text);
		expect(c.events.map((e) => e.data)).toEqual(['a', 'b']);
		expect(c.duration).toBe(1);
	});

	it('never lets time go backwards', () => {
		const c = parseCast(cast([2, 'o', 'a'], [1, 'o', 'b'], [3, 'o', 'c']));
		expect(c.events.map((e) => e.time)).toEqual([2, 2, 3]);
	});

	it('handles a header only (a recording that has just started)', () => {
		const c = parseCast(header + '\n');
		expect(c.events).toEqual([]);
		expect(c.duration).toBe(0);
	});
});

const ev = (time: number, data = 'x', kind = 'o'): CastEvent => ({ time, kind, data });

describe('limitIdle', () => {
	it('shortens long pauses and keeps short ones', () => {
		const events = [ev(1), ev(1.5), ev(61.5), ev(62)];
		expect(limitIdle(events, 2).map((e) => e.time)).toEqual([1, 1.5, 3.5, 4]);
		expect(limitIdle(events, null)).toBe(events);
	});

	it('counts the pause before the first event too', () => {
		expect(limitIdle([ev(30)], 2).map((e) => e.time)).toEqual([2]);
	});
});

describe('indexAt', () => {
	const events = [ev(0), ev(1), ev(1), ev(2.5)];
	it('is the number of events that have happened by then', () => {
		expect(indexAt(events, -1)).toBe(0);
		expect(indexAt(events, 0)).toBe(1);
		expect(indexAt(events, 0.99)).toBe(1);
		expect(indexAt(events, 1)).toBe(3);
		expect(indexAt(events, 2.5)).toBe(4);
		expect(indexAt(events, 99)).toBe(4);
		expect(indexAt([], 5)).toBe(0);
	});
});

describe('steps', () => {
	it('joins output and keeps resizes in their place', () => {
		const events = [ev(0, 'a'), ev(1, 'b'), ev(2, '120x40', 'r'), ev(3, 'c'), ev(4, 'lost', 'm')];
		expect(steps(events, 0, events.length)).toEqual([
			{ write: 'ab' },
			{ resize: { cols: 120, rows: 40 } },
			{ write: 'c' }
		]);
		expect(steps(events, 3, 4)).toEqual([{ write: 'c' }]);
		expect(steps(events, 4, 2)).toEqual([]);
	});

	it('ignores a malformed resize', () => {
		expect(steps([ev(0, 'a'), ev(1, 'wide', 'r'), ev(2, 'b')], 0, 3)).toEqual([{ write: 'ab' }]);
	});
});

describe('helpers', () => {
	it('parses sizes', () => {
		expect(parseSize('120x40')).toEqual({ cols: 120, rows: 40 });
		expect(parseSize(' 80x24 ')).toEqual({ cols: 80, rows: 24 });
		expect(parseSize('0x24')).toBeNull();
		expect(parseSize('80 by 24')).toBeNull();
	});

	it('lists markers', () => {
		expect(markers([ev(1), ev(2, 'Recording stopped at the 100 MB limit', 'm')])).toEqual([
			{ time: 2, label: 'Recording stopped at the 100 MB limit' }
		]);
	});

	it('formats clocks and sizes', () => {
		expect(formatClock(0)).toBe('0:00');
		expect(formatClock(5.9)).toBe('0:05');
		expect(formatClock(125)).toBe('2:05');
		expect(formatClock(3725)).toBe('1:02:05');
		expect(formatBytes(512)).toBe('512 B');
		expect(formatBytes(2150)).toBe('2.1 KB');
		expect(formatBytes(3_500_000)).toBe('3.3 MB');
		expect(formatBytes(150 * 1024 * 1024)).toBe('150 MB');
	});
});
