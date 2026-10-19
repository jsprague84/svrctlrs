/**
 * asciicast v2 for the in-app player: read a recording, shorten long pauses, find where a
 * moment falls, and turn a stretch of events into what to apply to a terminal.
 * Format: https://docs.asciinema.org/manual/asciicast/v2/
 */

export interface CastHeader {
	version: 2;
	width: number;
	height: number;
	/** Unix seconds when the recording started */
	timestamp?: number;
	title?: string;
}

/** `o` output, `r` resize (`COLSxROWS`), `m` marker; others are kept but not shown. */
export interface CastEvent {
	/** Seconds from the start, never decreasing */
	time: number;
	kind: string;
	data: string;
}

export interface Cast {
	header: CastHeader;
	events: CastEvent[];
	/** Time of the last event */
	duration: number;
}

/** One thing to do to the terminal: write output, or change its size. */
export type Step = { write: string } | { resize: { cols: number; rows: number } };

function isSize(n: unknown): n is number {
	return typeof n === 'number' && Number.isInteger(n) && n > 0 && n < 10_000;
}

/**
 * Parse a recording. Throws when the first line is not an asciicast v2 header. Event lines
 * that do not parse are skipped: the last line of a recording still being written can be cut
 * off. Times are kept from going backwards.
 */
export function parseCast(text: string): Cast {
	const lines = text.split('\n');
	let i = 0;
	while (i < lines.length && lines[i].trim() === '') i++;
	let header: CastHeader;
	try {
		header = JSON.parse(lines[i] ?? '');
	} catch {
		throw new Error('Not an asciicast recording');
	}
	if (!header || header.version !== 2 || !isSize(header.width) || !isSize(header.height)) {
		throw new Error('Not an asciicast v2 recording');
	}
	const events: CastEvent[] = [];
	let last = 0;
	for (const line of lines.slice(i + 1)) {
		if (line.trim() === '') continue;
		let raw: unknown;
		try {
			raw = JSON.parse(line);
		} catch {
			continue;
		}
		if (!Array.isArray(raw) || raw.length < 3) continue;
		const [t, kind, data] = raw;
		if (typeof t !== 'number' || !Number.isFinite(t) || typeof kind !== 'string') continue;
		if (typeof data !== 'string') continue;
		last = Math.max(last, t);
		events.push({ time: last, kind, data });
	}
	return { header, events, duration: last };
}

/**
 * The same events with every pause longer than `maxIdle` seconds shortened to it (what
 * asciinema calls the idle time limit), so a shell left open over lunch plays in seconds.
 * `null` keeps the real timing.
 */
export function limitIdle(events: CastEvent[], maxIdle: number | null): CastEvent[] {
	if (maxIdle === null) return events;
	let shift = 0;
	let previous = 0;
	return events.map((e) => {
		const gap = e.time - previous;
		if (gap > maxIdle) shift += gap - maxIdle;
		previous = e.time;
		return { ...e, time: e.time - shift };
	});
}

/** How many events have happened by `time`: the index of the first event after it. */
export function indexAt(events: CastEvent[], time: number): number {
	let lo = 0;
	let hi = events.length;
	while (lo < hi) {
		const mid = (lo + hi) >>> 1;
		if (events[mid].time <= time) lo = mid + 1;
		else hi = mid;
	}
	return lo;
}

/** `"120x40"` → `{ cols: 120, rows: 40 }`; anything else → null. */
export function parseSize(data: string): { cols: number; rows: number } | null {
	const m = /^(\d+)x(\d+)$/.exec(data.trim());
	if (!m) return null;
	const cols = Number(m[1]);
	const rows = Number(m[2]);
	return isSize(cols) && isSize(rows) ? { cols, rows } : null;
}

/**
 * What to apply to show `events[from..to)`: consecutive output joined into one write (a seek
 * replays thousands of events), resizes in their place, markers and unknown kinds skipped.
 */
export function steps(events: CastEvent[], from: number, to: number): Step[] {
	const out: Step[] = [];
	let text = '';
	for (let i = Math.max(0, from); i < Math.min(to, events.length); i++) {
		const e = events[i];
		if (e.kind === 'o') {
			text += e.data;
		} else if (e.kind === 'r') {
			const size = parseSize(e.data);
			if (!size) continue;
			if (text) out.push({ write: text });
			text = '';
			out.push({ resize: size });
		}
	}
	if (text) out.push({ write: text });
	return out;
}

/** Marker events (where output was lost, where the size limit stopped the recording). */
export function markers(events: CastEvent[]): { time: number; label: string }[] {
	return events.filter((e) => e.kind === 'm').map((e) => ({ time: e.time, label: e.data }));
}

/** 5 → "0:05", 125 → "2:05", 3725 → "1:02:05". */
export function formatClock(seconds: number): string {
	const total = Math.max(0, Math.floor(seconds));
	const h = Math.floor(total / 3600);
	const m = Math.floor((total % 3600) / 60);
	const s = total % 60;
	const ss = String(s).padStart(2, '0');
	return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** 512 → "512 B", 2150 → "2.1 KB", 3_500_000 → "3.3 MB". */
export function formatBytes(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	const units = ['KB', 'MB', 'GB'];
	let value = bytes / 1024;
	let unit = 0;
	while (value >= 1024 && unit < units.length - 1) {
		value /= 1024;
		unit++;
	}
	return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}
