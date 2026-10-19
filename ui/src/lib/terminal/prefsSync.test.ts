import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TerminalPreferences } from '#lib/types/index.js';
import { DEFAULT_PREFS, PrefsSync, SAVE_DELAY, mergePrefs } from './prefsSync.js';

/** An account on a fake server, and one device's state in front of it */
function rig(opts: { server?: unknown; hadLocal?: boolean; local?: Partial<TerminalPreferences> }) {
	const server = {
		value: opts.server ?? null,
		saves: 0,
		failing: false,
		slowLoad: null as null | (() => void)
	};
	let prefs: TerminalPreferences = { ...DEFAULT_PREFS, ...opts.local };
	const sync = new PrefsSync({
		remote: {
			load: async () => {
				if (server.failing) throw new Error('offline');
				if (server.slowLoad) await new Promise<void>((r) => (server.slowLoad = r));
				return server.value;
			},
			save: async (p) => {
				if (server.failing) throw new Error('offline');
				server.saves++;
				server.value = { ...p };
			}
		},
		current: () => prefs,
		apply: (p) => (prefs = p),
		hadLocal: opts.hadLocal ?? false
	});
	return {
		server,
		sync,
		get prefs() {
			return prefs;
		},
		change<K extends keyof TerminalPreferences>(key: K, value: TerminalPreferences[K]) {
			prefs = { ...prefs, [key]: value };
			sync.changed();
		}
	};
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('mergePrefs', () => {
	it('keeps known keys of the right type and known choices, and defaults the rest', () => {
		const merged = mergePrefs({
			fontSize: 12,
			cursorStyle: 'bar',
			predictiveEcho: 'off',
			leaderKey: 'alt-space',
			scrollback: '9000', // wrong type
			lineHeight: Number.NaN,
			cursorBlink: 'yes',
			tabStopWidth: Infinity,
			fromTheFuture: true
		});
		expect(merged.fontSize).toBe(12);
		expect(merged.cursorStyle).toBe('bar');
		expect(merged.predictiveEcho).toBe('off');
		expect(merged.leaderKey).toBe('alt-space');
		expect(merged.scrollback).toBe(DEFAULT_PREFS.scrollback);
		expect(merged.lineHeight).toBe(DEFAULT_PREFS.lineHeight);
		expect(merged.cursorBlink).toBe(DEFAULT_PREFS.cursorBlink);
		expect(merged.tabStopWidth).toBe(DEFAULT_PREFS.tabStopWidth);
		expect('fromTheFuture' in merged).toBe(false);
	});

	it('refuses unknown choices and anything that is not an object', () => {
		expect(mergePrefs({ cursorStyle: 'blinky', predictiveEcho: 'maybe' })).toEqual(DEFAULT_PREFS);
		for (const junk of [null, undefined, 3, 'x', [1, 2]]) {
			expect(mergePrefs(junk)).toEqual(DEFAULT_PREFS);
		}
	});
});

describe('PrefsSync', () => {
	it('takes what the account has', async () => {
		const r = rig({
			server: { fontSize: 18, cursorStyle: 'underline' },
			hadLocal: true,
			local: { fontSize: 11 }
		});
		await r.sync.pull();
		expect(r.prefs.fontSize).toBe(18);
		expect(r.prefs.cursorStyle).toBe('underline');
		expect(r.server.saves).toBe(0);
	});

	it("uploads this browser's own preferences when the account has none yet", async () => {
		const r = rig({ server: null, hadLocal: true, local: { fontSize: 11 } });
		await r.sync.pull();
		expect(r.server.saves).toBe(1);
		expect((r.server.value as TerminalPreferences).fontSize).toBe(11);
	});

	it('uploads nothing when neither side has preferences', async () => {
		const r = rig({ server: null, hadLocal: false });
		await r.sync.pull();
		expect(r.server.saves).toBe(0);
		expect(r.prefs).toEqual(DEFAULT_PREFS);
	});

	it('saves once after a burst of changes', async () => {
		const r = rig({});
		r.change('fontSize', 15);
		r.change('fontSize', 16);
		r.change('cursorBlink', false);
		expect(r.server.saves).toBe(0);
		await vi.advanceTimersByTimeAsync(SAVE_DELAY);
		expect(r.server.saves).toBe(1);
		expect(r.server.value).toMatchObject({ fontSize: 16, cursorBlink: false });
	});

	it('never lets the server overwrite a change that is not saved yet', async () => {
		const r = rig({ server: { fontSize: 18 } });
		r.change('fontSize', 10);
		await r.sync.pull(); // the page was shown again before the save went out
		expect(r.prefs.fontSize).toBe(10);
		expect((r.server.value as TerminalPreferences).fontSize).toBe(10);
	});

	it('keeps a change made while the account was being read', async () => {
		const r = rig({ server: { fontSize: 18 } });
		r.server.slowLoad = () => {};
		const pulling = r.sync.pull();
		await Promise.resolve();
		r.change('fontSize', 9);
		r.server.slowLoad?.();
		await pulling;
		expect(r.prefs.fontSize).toBe(9);
		await vi.advanceTimersByTimeAsync(SAVE_DELAY);
		expect((r.server.value as TerminalPreferences).fontSize).toBe(9);
	});

	it('keeps working offline and saves the change once the server answers again', async () => {
		const r = rig({ server: { fontSize: 18 } });
		r.server.failing = true;
		await r.sync.pull();
		expect(r.prefs.fontSize).toBe(DEFAULT_PREFS.fontSize);
		r.change('fontSize', 20);
		await vi.advanceTimersByTimeAsync(SAVE_DELAY);
		expect(r.sync.pending).toBe(true);

		r.server.failing = false;
		await r.sync.pull(); // the page shown again: the waiting change goes up first
		expect(r.sync.pending).toBe(false);
		expect((r.server.value as TerminalPreferences).fontSize).toBe(20);
	});

	it('flush saves at once, and only when something changed', async () => {
		const r = rig({});
		await r.sync.flush();
		expect(r.server.saves).toBe(0);
		r.change('scrollback', 9000);
		await r.sync.flush();
		expect(r.server.saves).toBe(1);
		await vi.advanceTimersByTimeAsync(SAVE_DELAY);
		expect(r.server.saves).toBe(1); // the debounced save was cancelled
	});
});
