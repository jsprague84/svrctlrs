/**
 * Predictive local echo: printable characters appear the moment they are typed, drawn in a
 * provisional style, and are withdrawn once the shell's own echo has drawn them (or has drawn
 * something else). On a phone over a lossy link that turns "type, wait, see" into "type, see".
 *
 * Predictions are checked against the **screen**, not the byte stream: a prediction is
 * confirmed when its cell holds the typed character and the cursor has moved past it. Prompt
 * colours, zsh redraws and syntax highlighting therefore confirm just as well as a plain
 * echo, and nothing is ever written into the terminal itself — the pane draws predictions on
 * top of it, so a wrong guess cannot corrupt the shell's screen or its attributes.
 *
 * Nothing is shown unless the shell is known to be echoing. As in mosh, predictions belong to
 * an *epoch*; an epoch is shown only after one of its predictions has been confirmed. Every
 * key that is not a printable character (Enter, Ctrl-C, arrows, Tab, Escape), every cursor
 * movement the predictions do not explain (a prompt, command output), a mismatch and a
 * timeout start a new epoch. A password prompt does not echo, so its epoch is never confirmed
 * and nothing typed there is drawn. On top of that: never in the alternate screen (vim, less,
 * htop), never on a line that ends in a password/passphrase/PIN prompt, never in the last
 * column, and (in `auto`) only once the measured echo latency makes it worth it.
 */

export type EchoMode = 'auto' | 'always' | 'off';

/** One predicted character, at an absolute buffer line (scrollback included) and column. */
export interface EchoCell {
	line: number;
	col: number;
	char: string;
}

/** The terminal as the predictor sees it; the pane implements it over xterm's buffer. */
export interface EchoScreen {
	readonly cols: number;
	/** The cursor as an absolute buffer line and column; null in the alternate screen */
	cursor(): { line: number; col: number } | null;
	/** The character in a cell, `' '` for a blank one; null when the line does not exist */
	cell(line: number, col: number): string | null;
	/** The cursor line's text left of the cursor (the prompt check) */
	textBeforeCursor(): string;
	/** Draw these provisional characters, replacing what was drawn before; `[]` clears */
	show(cells: EchoCell[]): void;
}

export interface EchoTimers {
	setTimeout(fn: () => void, ms: number): unknown;
	clearTimeout(handle: unknown): void;
}

export interface LocalEchoOptions {
	screen: EchoScreen;
	/** Read on every keystroke, so a preference change applies at once */
	mode: () => EchoMode;
	/** True while server output is queued in the terminal but not yet drawn: the cursor the
	 *  screen reports is stale, so no new run of predictions starts from it */
	busy?: () => boolean;
	now?: () => number;
	timers?: EchoTimers;
}

/** A prediction the shell has not confirmed in this long is withdrawn and its epoch ended. */
export const PREDICTION_TIMEOUT = 2000;
/** `auto` starts drawing predictions when the smoothed echo latency rises above this (ms)… */
export const LATENCY_SHOW = 30;
/** …and stops again below this, so a link near the threshold does not flicker. */
export const LATENCY_HIDE = 20;
/** One keystroke event longer than this is a paste, not typing */
export const MAX_TYPED_RUN = 32;

/**
 * Text before the cursor that asks for a secret: `Password:`, `[sudo] password for x:`,
 * `Enter passphrase for key '…':`, `PIN:`, `Verification code:`. Such a prompt does not echo
 * anyway; this keeps even a guess from being drawn there.
 */
const SECRET_PROMPT =
	/\b(password|passwort|passphrase|passcode|pin|otp|one-time|verification code|token|secret)\b[^\n]{0,60}[:?]\s*$/i;

export function looksLikeSecretPrompt(text: string): boolean {
	return SECRET_PROMPT.test(text);
}

/** A character that takes exactly one cell and that a line editor echoes as itself. */
function predictable(cp: number): boolean {
	return (
		(cp >= 0x20 && cp <= 0x7e) || // ASCII
		(cp >= 0xa1 && cp <= 0x24f && cp !== 0xad) || // Latin-1 and Latin Extended, no soft hyphen
		(cp >= 0x386 && cp <= 0x3ce) || // Greek
		(cp >= 0x400 && cp <= 0x481) || // Cyrillic, without its combining marks
		(cp >= 0x48a && cp <= 0x52f)
	);
}

/** The characters of a typed chunk, or null when any of it cannot be predicted. */
export function typedChars(data: string): string[] | null {
	if (data.length === 0 || data.length > MAX_TYPED_RUN) return null;
	const chars: string[] = [];
	for (const ch of data) {
		const cp = ch.codePointAt(0) ?? 0;
		if (!predictable(cp)) return null;
		chars.push(ch);
	}
	return chars;
}

interface Prediction extends EchoCell {
	/** What the cell held when the prediction was made */
	original: string;
	epoch: number;
	at: number;
}

const BACKSPACE = new Set(['\x7f', '\b']);

export class LocalEcho {
	private readonly screen: EchoScreen;
	private readonly mode: () => EchoMode;
	private readonly busy: () => boolean;
	private readonly now: () => number;
	private readonly timers: EchoTimers;

	/** Unconfirmed predictions, in typing order: one contiguous run on one line */
	private pending: Prediction[] = [];
	/** New keystrokes may extend the run (false after a key we could not place) */
	private open = false;
	private epoch = 1;
	/** The newest epoch in which the shell has echoed a prediction */
	private confirmedEpoch = 0;
	/** Where the cursor was after the last check; movement from here must be explained */
	private lastCursor: { line: number; col: number } | null = null;
	/** Backspaces typed but not yet seen on screen: the cursor may step back this far */
	private erases = 0;
	/** Smoothed echo latency in ms, from confirmed predictions; null before the first */
	private srtt: number | null = null;
	/** `auto`: the latency is high enough to draw predictions */
	private latencyGate = false;
	private expiry: unknown = null;
	/** What was last handed to `show`, so an unchanged set is not redrawn */
	private drawn = '';

	constructor(opts: LocalEchoOptions) {
		this.screen = opts.screen;
		this.mode = opts.mode;
		this.busy = opts.busy ?? (() => false);
		this.now = opts.now ?? (() => performance.now());
		this.timers = opts.timers ?? {
			setTimeout: (fn, ms) => setTimeout(fn, ms),
			clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>)
		};
	}

	// ── Read-outs (tests, diagnostics) ───────────────────────────────────

	/** Predictions the shell has not confirmed yet, drawn or not */
	get pendingCount(): number {
		return this.pending.length;
	}

	/** Smoothed echo latency in ms, or null before anything was confirmed */
	get latency(): number | null {
		return this.srtt;
	}

	/** The predictions that are drawn now */
	get visible(): EchoCell[] {
		if (!this.drawing()) return [];
		return this.pending
			.filter((p) => p.epoch <= this.confirmedEpoch)
			.map(({ line, col, char }) => ({ line, col, char }));
	}

	// ── Input ────────────────────────────────────────────────────────────

	/** A keystroke (or a chunk the keyboard sent at once), just before it goes to the shell. */
	input(data: string): void {
		if (this.mode() === 'off') {
			this.reset();
			return;
		}
		if (BACKSPACE.has(data)) {
			this.backspace();
			return;
		}
		const chars = typedChars(data);
		if (!chars) {
			// Enter, Ctrl-C, arrows, Tab, a paste: where the cursor goes next is the shell's call
			this.boundary();
			return;
		}
		const cursor = this.screen.cursor();
		if (!cursor) {
			this.kill(); // alternate screen
			return;
		}
		if (looksLikeSecretPrompt(this.screen.textBeforeCursor())) {
			this.kill();
			return;
		}

		let line: number;
		let col: number;
		const last = this.pending[this.pending.length - 1];
		if (last) {
			if (!this.open) {
				this.boundary();
				return;
			}
			line = last.line;
			col = last.col + 1;
		} else {
			const stale = this.busy();
			if (stale || this.erases > 0 || !this.cursorExplained(cursor)) {
				// Output still being drawn, a backspace not yet on screen, or the screen moved
				// since we last looked: the cursor is not where this character will land
				this.boundary();
				this.lastCursor = stale ? null : cursor;
				return;
			}
			line = cursor.line;
			col = cursor.col;
		}

		const at = this.now();
		for (const char of chars) {
			// Never the last column: what the shell does at the margin (wrap, pending wrap,
			// redraw on the next line) is not ours to guess
			if (col >= this.screen.cols - 1) {
				this.boundary();
				break;
			}
			const original = this.screen.cell(line, col) ?? ' ';
			this.pending.push({ line, col, char, original, epoch: this.epoch, at });
			col++;
			this.open = true;
		}
		this.armExpiry();
		this.render();
	}

	/**
	 * Backspace. With predictions drawn, the newest is withdrawn at once (that character still
	 * reaches the shell, is echoed and then erased, so the screen moves in ways the run cannot
	 * follow: new epoch). Without, the cursor may step back one cell when the echo arrives.
	 */
	private backspace() {
		if (this.pending.length > 0) {
			this.pending.pop();
			this.boundary();
			this.armExpiry();
			return;
		}
		this.erases++;
	}

	// ── Output ───────────────────────────────────────────────────────────

	/** The terminal has drawn everything the server sent so far: check the predictions. */
	settled(): void {
		if (this.mode() === 'off') return;
		const cursor = this.screen.cursor();
		if (!cursor) {
			this.kill();
			this.lastCursor = null;
			return;
		}

		let confirmed: Prediction | null = null;
		while (this.pending.length > 0) {
			const p = this.pending[0];
			const past = cursor.line > p.line || (cursor.line === p.line && cursor.col > p.col);
			const cell = this.screen.cell(p.line, p.col);
			if (cell === p.char && past) {
				this.pending.shift();
				this.confirm(p);
				confirmed = p;
				continue;
			}
			if (!past && (cell === p.char || cell === p.original)) break; // still on its way
			this.kill(); // the shell drew something else there
			break;
		}

		if (this.pending.length > 0) {
			// The cursor waits where the next echo lands; anywhere else, our run is stale
			const next = this.pending[0];
			if (cursor.line !== next.line || cursor.col !== next.col) this.kill();
		} else {
			const expected = confirmed
				? { line: confirmed.line, col: confirmed.col + 1 }
				: this.lastCursor;
			if (expected && !this.near(cursor, expected)) {
				// Output we did not predict (a prompt, a command's output): whatever comes next
				// waits for its own echo before it is drawn
				this.boundary();
			} else if (expected) {
				this.erases -= expected.col - cursor.col;
			}
		}
		this.lastCursor = cursor;
		this.armExpiry();
		this.render();
	}

	/** Forget everything (new connection, replay, the shell ended). */
	reset(): void {
		this.pending = [];
		this.open = false;
		this.epoch++;
		this.erases = 0;
		this.lastCursor = null;
		this.disarmExpiry();
		this.render();
	}

	dispose(): void {
		this.reset();
	}

	// ── Internals ────────────────────────────────────────────────────────

	/** The cursor is where the last check left it. */
	private cursorExplained(cursor: { line: number; col: number }): boolean {
		const last = this.lastCursor;
		return last === null || (cursor.line === last.line && cursor.col === last.col);
	}

	/** Where we expected the cursor, or up to `erases` cells left of it on the same line. */
	private near(cursor: { line: number; col: number }, expected: { line: number; col: number }) {
		return (
			cursor.line === expected.line &&
			cursor.col <= expected.col &&
			cursor.col >= expected.col - this.erases
		);
	}

	private confirm(p: Prediction) {
		this.confirmedEpoch = Math.max(this.confirmedEpoch, p.epoch);
		// A cell that already held the character says nothing about the round trip
		if (p.original === p.char) return;
		const sample = Math.max(0, this.now() - p.at);
		this.srtt = this.srtt === null ? sample : this.srtt + (sample - this.srtt) / 8;
		if (this.srtt > LATENCY_SHOW) this.latencyGate = true;
		else if (this.srtt < LATENCY_HIDE) this.latencyGate = false;
	}

	/** The next keystroke is not where we think: stop extending, and start a new epoch. */
	private boundary() {
		this.open = false;
		this.epoch++;
		this.erases = 0;
		this.render();
	}

	/** The predictions are wrong or stale: withdraw them all. */
	private kill() {
		this.pending = [];
		this.boundary();
		this.disarmExpiry();
	}

	private drawing(): boolean {
		const mode = this.mode();
		return mode === 'always' || (mode === 'auto' && this.latencyGate);
	}

	private render() {
		const cells = this.visible;
		const key = cells.map((c) => `${c.line}:${c.col}:${c.char}`).join('|');
		if (key === this.drawn) return;
		this.drawn = key;
		this.screen.show(cells);
	}

	private armExpiry() {
		this.disarmExpiry();
		const oldest = this.pending[0];
		if (!oldest) return;
		const wait = Math.max(0, oldest.at + PREDICTION_TIMEOUT - this.now());
		this.expiry = this.timers.setTimeout(() => {
			this.expiry = null;
			this.kill();
		}, wait);
	}

	private disarmExpiry() {
		if (this.expiry !== null) {
			this.timers.clearTimeout(this.expiry);
			this.expiry = null;
		}
	}
}
