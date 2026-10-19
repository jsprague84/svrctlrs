/**
 * Terminal preferences that follow the account: the server keeps them
 * (`/api/v1/users/me/preferences/terminal`), the browser keeps a copy so the terminal starts
 * with the right font before the server has answered, and works offline.
 *
 * Rules: a change is saved shortly after it is made (several quick changes become one save);
 * a change not yet saved is never overwritten by what the server has; when the server has
 * nothing yet, this browser's own preferences (from before they were synced) are uploaded.
 */

import type { CursorStyle, LeaderKeyOption, TerminalPreferences } from '#lib/types/index.js';
import type { EchoMode } from './localEcho.js';

export const DEFAULT_PREFS: TerminalPreferences = {
	fontSize: 14,
	fontFamily: '"JetBrains Mono", "Fira Code", "Cascadia Code", monospace',
	fontWeight: 400,
	fontWeightBold: 700,
	lineHeight: 1.4,
	letterSpacing: 0,
	cursorStyle: 'block',
	cursorBlink: true,
	scrollback: 5000,
	scrollSensitivity: 1,
	fastScrollSensitivity: 5,
	smoothScrollDuration: 0,
	scrollOnUserInput: true,
	tabStopWidth: 4,
	drawBoldTextInBrightColors: true,
	minimumContrastRatio: 1,
	leaderKey: 'ctrl-space',
	predictiveEcho: 'auto'
};

const CHOICES: Partial<Record<keyof TerminalPreferences, readonly string[]>> = {
	cursorStyle: ['block', 'underline', 'bar'] satisfies CursorStyle[],
	leaderKey: ['ctrl-space', 'ctrl-shift-space', 'alt-space', 'off'] satisfies LeaderKeyOption[],
	predictiveEcho: ['auto', 'always', 'off'] satisfies EchoMode[]
};

/**
 * Saved preferences (from the server or localStorage, so untrusted in shape) over the
 * defaults: only known keys, each of its default's type, enums only with a known value.
 * Anything else falls back to the default, so an older or newer client cannot break this one.
 */
export function mergePrefs(saved: unknown): TerminalPreferences {
	const out: TerminalPreferences = { ...DEFAULT_PREFS };
	if (!saved || typeof saved !== 'object' || Array.isArray(saved)) return out;
	const record = saved as Record<string, unknown>;
	for (const key of Object.keys(DEFAULT_PREFS) as (keyof TerminalPreferences)[]) {
		const value = record[key];
		if (typeof value !== typeof DEFAULT_PREFS[key]) continue;
		if (typeof value === 'number' && !Number.isFinite(value)) continue;
		const choices = CHOICES[key];
		if (choices && !choices.includes(value as string)) continue;
		(out as unknown as Record<string, unknown>)[key] = value;
	}
	return out;
}

export interface PrefsRemote {
	/** The saved document, or null when the account has none yet */
	load(): Promise<unknown>;
	save(prefs: TerminalPreferences): Promise<void>;
}

export interface PrefsSyncOptions {
	remote: PrefsRemote;
	/** The preferences in use now */
	current: () => TerminalPreferences;
	/** Use these (they came from the server) */
	apply: (prefs: TerminalPreferences) => void;
	/** This browser had preferences of its own before the first sync */
	hadLocal: boolean;
	/** Quiet time after a change before it is saved (ms) */
	delay?: number;
	timers?: {
		setTimeout(fn: () => void, ms: number): unknown;
		clearTimeout(handle: unknown): void;
	};
}

export const SAVE_DELAY = 800;

export class PrefsSync {
	private readonly opts: PrefsSyncOptions;
	private readonly timers: NonNullable<PrefsSyncOptions['timers']>;
	/** A change made here and not yet saved */
	private dirty = false;
	private timer: unknown = null;
	private hadLocal: boolean;

	constructor(opts: PrefsSyncOptions) {
		this.opts = opts;
		this.hadLocal = opts.hadLocal;
		this.timers = opts.timers ?? {
			setTimeout: (fn, ms) => setTimeout(fn, ms),
			clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>)
		};
	}

	/** A change waits to be saved */
	get pending(): boolean {
		return this.dirty;
	}

	/** Pick up what the account has (app start, the page shown again). A change waiting to be
	 *  saved goes up instead; failures leave the local copy in use. */
	async pull(): Promise<void> {
		if (this.dirty) return this.flush();
		let saved: unknown;
		try {
			saved = await this.opts.remote.load();
		} catch {
			return; // offline or signed out: keep using the local copy
		}
		if (this.dirty) return; // changed while we were asking; that change wins
		if (saved === null || saved === undefined) {
			// Nothing on the account yet: this browser's own preferences become the account's
			if (this.hadLocal) {
				this.hadLocal = false;
				this.dirty = true;
				await this.flush();
			}
			return;
		}
		this.hadLocal = false;
		this.opts.apply(mergePrefs(saved));
	}

	/** The user changed something: save it once they pause. */
	changed(): void {
		this.dirty = true;
		if (this.timer !== null) this.timers.clearTimeout(this.timer);
		this.timer = this.timers.setTimeout(() => {
			this.timer = null;
			void this.flush();
		}, this.opts.delay ?? SAVE_DELAY);
	}

	/** Save now if a change is waiting (the page is being hidden). */
	async flush(): Promise<void> {
		if (this.timer !== null) {
			this.timers.clearTimeout(this.timer);
			this.timer = null;
		}
		if (!this.dirty) return;
		this.dirty = false;
		try {
			await this.opts.remote.save(this.opts.current());
		} catch {
			// Kept for the next change or the next time the page is shown
			this.dirty = true;
		}
	}
}
