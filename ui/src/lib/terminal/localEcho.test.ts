import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeEchoScreen } from './fakeEchoScreen.js';
import {
	LATENCY_SHOW,
	LocalEcho,
	PREDICTION_TIMEOUT,
	looksLikeSecretPrompt,
	typedChars,
	type EchoMode
} from './localEcho.js';

interface Rig {
	screen: FakeEchoScreen;
	echo: LocalEcho;
	mode: { value: EchoMode };
	busy: { value: boolean };
	/** The user typed this (one keystroke event) */
	type(data: string): void;
	/** The shell printed this and the terminal drew it */
	shell(text: string): void;
	/** Every keystroke of `text` typed and echoed one at a time: confirms the epoch */
	typeEchoed(text: string): void;
}

function rig(opts: { mode?: EchoMode; cols?: number; prompt?: string } = {}): Rig {
	const screen = new FakeEchoScreen(opts.cols ?? 80);
	const mode = { value: opts.mode ?? 'always' };
	const busy = { value: false };
	const echo = new LocalEcho({
		screen,
		mode: () => mode.value,
		busy: () => busy.value,
		now: () => Date.now()
	});
	const r: Rig = {
		screen,
		echo,
		mode,
		busy,
		type: (data) => echo.input(data),
		shell: (text) => {
			screen.print(text);
			echo.settled();
		},
		typeEchoed: (text) => {
			for (const ch of text) {
				r.type(ch);
				r.shell(ch);
			}
		}
	};
	r.shell(opts.prompt ?? '$ ');
	return r;
}

/** Every prediction drawn since `from` (index into `shown`) */
function drawnSince(screen: FakeEchoScreen, from: number): string[] {
	return screen.shown.slice(from).map((cells) => cells.map((c) => c.char).join(''));
}

beforeEach(() => {
	vi.useFakeTimers();
});
afterEach(() => {
	vi.useRealTimers();
});

describe('typing at a prompt', () => {
	it('draws nothing until the shell has echoed once, then draws ahead of the echo', () => {
		const r = rig();
		r.type('l');
		expect(r.echo.pendingCount).toBe(1);
		expect(r.echo.visible).toEqual([]); // a new epoch: the shell has not echoed yet

		r.shell('l');
		expect(r.echo.pendingCount).toBe(0);

		r.type('s');
		expect(r.echo.visible).toEqual([{ line: 0, col: 3, char: 's' }]);
		expect(r.screen.drawn).toBe('s');

		r.shell('s');
		expect(r.screen.drawn).toBe('');
		expect(r.screen.text()).toBe('$ ls');
	});

	it('draws the rest of a fast burst as soon as its first character comes back', () => {
		const r = rig();
		for (const ch of 'ls -la') r.type(ch);
		expect(r.echo.visible).toEqual([]);

		r.shell('l');
		expect(r.screen.drawn).toBe('s -la');
		r.shell('s -');
		expect(r.screen.drawn).toBe('la');
		r.shell('la');
		expect(r.screen.drawn).toBe('');
		expect(r.echo.pendingCount).toBe(0);
	});

	it('keeps drawing ahead while each character is confirmed before the next is typed', () => {
		const r = rig();
		r.typeEchoed('e');
		for (const ch of 'cho') {
			r.type(ch);
			expect(r.screen.drawn).toBe(ch);
			r.shell(ch);
			expect(r.screen.drawn).toBe('');
		}
	});

	it('confirms an echo that redraws the whole line (syntax highlighting, zsh)', () => {
		const r = rig();
		r.typeEchoed('l');
		r.type('s');
		expect(r.screen.drawn).toBe('s');
		// The shell repaints the prompt and the line instead of printing one character
		r.shell('\r$ ls');
		expect(r.screen.drawn).toBe('');
		expect(r.echo.pendingCount).toBe(0);
		r.type(' ');
		expect(r.screen.drawn).toBe(' '); // the epoch is still confirmed
	});

	it('waits over a suggestion drawn after the cursor until the cursor moves past it', () => {
		const r = rig();
		r.type('l');
		// zsh-autosuggestions: the echo, then a grey suggestion, then the cursor back
		r.shell('ls -la\b\b\b\b\b');
		r.type('s');
		expect(r.echo.pendingCount).toBe(1); // the cell already shows 's', but the cursor is before it
		expect(r.screen.drawn).toBe('s');
		r.shell('s');
		expect(r.echo.pendingCount).toBe(0);
	});

	it('accepts Latin and Cyrillic letters, but not wide or combining characters', () => {
		expect(typedChars('é')).toEqual(['é']);
		expect(typedChars('Жж')).toEqual(['Ж', 'ж']);
		expect(typedChars('日')).toBeNull();
		expect(typedChars('é')).toBeNull();
		expect(typedChars('🙂')).toBeNull();
		expect(typedChars('')).toBeNull();
		expect(typedChars('x'.repeat(33))).toBeNull(); // a paste
	});

	it('predicts every character of a chunk the phone keyboard sends at once', () => {
		const r = rig();
		r.typeEchoed('g');
		r.type('it ');
		expect(r.screen.drawn).toBe('it ');
		expect(r.echo.visible.map((c) => c.col)).toEqual([3, 4, 5]);
	});
});

describe('where nothing is drawn', () => {
	it('never draws anything typed at a prompt that does not echo', () => {
		const r = rig();
		r.typeEchoed('sudo -k ls');
		r.type('\r');
		r.shell('\r\n');
		r.shell('Key for /dev/sdb: '); // a secret prompt the pattern does not know
		const from = r.screen.shown.length;
		for (const ch of 'hunter2 with spaces') r.type(ch);
		vi.advanceTimersByTime(PREDICTION_TIMEOUT + 1);
		for (const ch of 'more') r.type(ch);
		expect(drawnSince(r.screen, from).every((s) => s === '')).toBe(true);
		expect(r.echo.visible).toEqual([]);
	});

	it('never confirms a typed space over a blank cell when the cursor does not move', () => {
		const r = rig();
		r.typeEchoed('cat');
		r.type('\r');
		r.shell('\r\n> ');
		r.type(' '); // the cell already "holds" a space
		r.shell(''); // any output batch
		r.type('x');
		expect(r.echo.visible).toEqual([]);
	});

	it('does not even guess on a line that asks for a password', () => {
		const r = rig({ prompt: '[sudo] password for ubuntu: ' });
		r.type('a');
		expect(r.echo.pendingCount).toBe(0);
		expect(looksLikeSecretPrompt('Password: ')).toBe(true);
		expect(looksLikeSecretPrompt("Enter passphrase for key '/home/u/.ssh/id_ed25519': ")).toBe(
			true
		);
		expect(looksLikeSecretPrompt('Enter PIN for token: ')).toBe(true);
		expect(looksLikeSecretPrompt('Verification code: ')).toBe(true);
		expect(looksLikeSecretPrompt('ubuntu@hermes:~$ ')).toBe(false);
		expect(looksLikeSecretPrompt('ubuntu@hermes:~$ echo password ')).toBe(false);
	});

	it('withdraws everything in the alternate screen and predicts nothing there', () => {
		const r = rig();
		r.typeEchoed('v');
		r.type('i');
		expect(r.screen.drawn).toBe('i');
		r.screen.alternate = true; // vim started
		r.echo.settled();
		expect(r.screen.drawn).toBe('');
		r.type('j');
		expect(r.echo.pendingCount).toBe(0);
	});

	it('does not predict keys that are not text, and starts a new epoch on them', () => {
		const r = rig();
		r.typeEchoed('ls');
		for (const key of ['\t', '\x1b[A', '\x03', '\x1b[200~paste\x1b[201~']) {
			r.type(key);
			expect(r.echo.pendingCount).toBe(0);
		}
		r.type('x');
		expect(r.echo.visible).toEqual([]); // waits for its own echo
		r.shell('x');
		r.type('y');
		expect(r.screen.drawn).toBe('y');
	});

	it('stops short of the last column', () => {
		const r = rig({ cols: 10 });
		r.typeEchoed('a');
		r.type('bcdefghij'); // cols 3..11; only 3..8 fit before the last column
		expect(r.echo.visible.map((c) => c.col)).toEqual([3, 4, 5, 6, 7, 8]);
	});

	it('draws nothing when switched off, and withdraws what was drawn', () => {
		const r = rig();
		r.typeEchoed('l');
		r.type('s');
		expect(r.screen.drawn).toBe('s');
		r.mode.value = 'off';
		r.type('x');
		expect(r.screen.drawn).toBe('');
		expect(r.echo.pendingCount).toBe(0);
	});

	it('does not start a run while output is still being drawn', () => {
		const r = rig();
		r.typeEchoed('l');
		r.busy.value = true;
		r.type('s');
		expect(r.echo.pendingCount).toBe(0);
		r.busy.value = false;
		r.shell('s');
		r.type(' ');
		expect(r.echo.visible).toEqual([]); // a new epoch, until it echoes
	});
});

describe('when the shell does something else', () => {
	it('withdraws a prediction the shell drew differently, and the next waits for its echo', () => {
		const r = rig();
		r.typeEchoed('l');
		r.type('x');
		expect(r.screen.drawn).toBe('x');
		r.shell('y'); // e.g. a key binding that inserts something else
		expect(r.screen.drawn).toBe('');
		expect(r.echo.pendingCount).toBe(0);
		r.type('z');
		expect(r.echo.visible).toEqual([]);
	});

	it('treats output it did not predict as a new context', () => {
		const r = rig();
		r.typeEchoed('top');
		r.shell('\r\nYou have new mail\r\n$ '); // something printed without a keystroke
		r.type('q');
		expect(r.echo.visible).toEqual([]);
		r.shell('q');
		r.type('w');
		expect(r.screen.drawn).toBe('w');
	});

	it('keeps drawn predictions through Enter, then waits for the new prompt to echo', () => {
		const r = rig();
		r.typeEchoed('l');
		r.type('s');
		r.type('\r');
		expect(r.screen.drawn).toBe('s'); // still on its way; it is not withdrawn
		r.type('p'); // typed ahead: the next prompt's position is not known
		expect(r.echo.pendingCount).toBe(1);
		r.shell('s\r\nfile\r\n$ ');
		expect(r.screen.drawn).toBe('');
		r.type('w');
		expect(r.echo.visible).toEqual([]);
		r.shell('w');
		r.type('d');
		expect(r.screen.drawn).toBe('d');
	});

	it('withdraws predictions the shell never confirms, and ends their epoch', () => {
		const r = rig();
		r.typeEchoed('l');
		r.type('s');
		expect(r.screen.drawn).toBe('s');
		vi.advanceTimersByTime(PREDICTION_TIMEOUT - 1);
		expect(r.screen.drawn).toBe('s');
		vi.advanceTimersByTime(2);
		expect(r.screen.drawn).toBe('');
		expect(r.echo.pendingCount).toBe(0);
		r.shell('s'); // the echo was only late
		r.type(' ');
		expect(r.echo.visible).toEqual([]);
	});

	it('withdraws the run when the cursor leaves the place the next echo lands', () => {
		const r = rig();
		r.typeEchoed('l');
		r.type('s');
		r.shell('\r\n'); // a newline instead of the echo
		expect(r.screen.drawn).toBe('');
		expect(r.echo.pendingCount).toBe(0);
	});

	it('forgets everything on reset', () => {
		const r = rig();
		r.typeEchoed('l');
		r.type('s');
		r.echo.reset();
		expect(r.screen.drawn).toBe('');
		r.shell('s');
		r.type('x');
		expect(r.echo.visible).toEqual([]); // a reset is a new epoch too
	});
});

describe('backspace', () => {
	it('withdraws the newest prediction at once', () => {
		const r = rig();
		r.typeEchoed('l');
		r.type('s');
		r.type('x');
		expect(r.screen.drawn).toBe('sx');
		r.type('\x7f');
		expect(r.screen.drawn).toBe('s');
		r.type('y'); // the screen will move in ways the run cannot follow
		expect(r.screen.drawn).toBe('s');
		expect(r.echo.pendingCount).toBe(1);
	});

	it('keeps the epoch across an erase the shell has already drawn', () => {
		const r = rig();
		r.typeEchoed('lsx');
		r.type('\x7f');
		r.shell('\b\x1b[K');
		r.type(' ');
		expect(r.screen.drawn).toBe(' ');
		expect(r.echo.visible[0].col).toBe(4);
	});

	it('does not guess where a character typed before the erase is drawn lands', () => {
		const r = rig();
		r.typeEchoed('lsx');
		r.type('\x7f');
		r.type(' '); // the erase has not come back yet
		expect(r.echo.pendingCount).toBe(0);
	});
});

describe('auto mode', () => {
	it('draws only once the echo is measurably slow, and stops again when it is fast', () => {
		const r = rig({ mode: 'auto' });
		// A fast link: echoes come back at once
		r.typeEchoed('ls');
		r.type(' ');
		expect(r.echo.visible).toEqual([]);
		r.shell(' ');

		// A slow link: every echo takes 120 ms
		for (const ch of '-la /var/log') {
			r.type(ch);
			vi.advanceTimersByTime(120);
			r.shell(ch);
		}
		expect(r.echo.latency).toBeGreaterThan(LATENCY_SHOW);
		r.type('x');
		expect(r.screen.drawn).toBe('x');
		r.shell('x');

		// Fast again: the smoothed latency falls and drawing stops
		for (let i = 0; i < 40; i++) r.typeEchoed('y');
		r.type('z');
		expect(r.echo.visible).toEqual([]);
	});
});
