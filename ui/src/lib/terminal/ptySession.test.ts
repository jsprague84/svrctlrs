import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest';
import type { ConnectionStatus, PtyResponse } from '#lib/types/index.js';
import {
	loadSessionRecord,
	saveSessionRecord,
	type KeyValueStore
} from '#lib/utils/sessionRecord.js';
import {
	MAX_RECONNECT_ATTEMPTS,
	MAX_SAVED_BUFFER,
	PING_INTERVAL,
	PONG_TIMEOUT,
	PtySession,
	RECONNECT_DELAYS,
	reconnectDelay,
	savedBufferKey,
	type SessionView,
	type SocketLike,
	type TerminalSurface
} from './ptySession.js';
import { FakeEchoScreen } from './fakeEchoScreen.js';
import type { EchoMode } from './localEcho.js';

// ── Fakes ────────────────────────────────────────────────────────────────

class FakeSocket implements SocketLike {
	readyState = 0;
	binaryType: BinaryType = 'blob';
	onopen: SocketLike['onopen'] = null;
	onmessage: SocketLike['onmessage'] = null;
	onerror: SocketLike['onerror'] = null;
	onclose: SocketLike['onclose'] = null;
	sent: Record<string, unknown>[] = [];
	closed = false;
	constructor(readonly url: string) {}

	send(data: string) {
		this.sent.push(JSON.parse(data));
	}
	close() {
		this.closed = true;
		this.readyState = 3;
	}

	// Server side
	open() {
		this.readyState = 1;
		this.onopen?.({} as Event);
	}
	reply(res: Partial<PtyResponse> & { type: PtyResponse['type'] }) {
		this.onmessage?.({ data: JSON.stringify({ data: '', ...res }) } as MessageEvent);
	}
	output(seq: number, bytes: Uint8Array) {
		const buf = new ArrayBuffer(8 + bytes.length);
		new DataView(buf).setBigUint64(0, BigInt(seq));
		new Uint8Array(buf, 8).set(bytes);
		this.onmessage?.({ data: buf } as MessageEvent);
	}
	/** The connection died: error, then close, as browsers report it */
	drop() {
		this.readyState = 3;
		this.onerror?.({} as Event);
		this.onclose?.({} as CloseEvent);
	}
	/** A close the session asked for (watchdog) arriving */
	closeEvent() {
		this.readyState = 3;
		this.onclose?.({} as CloseEvent);
	}
	types() {
		return this.sent.map((m) => m.type);
	}
}

class FakeTerminal implements TerminalSurface {
	written: (string | Uint8Array)[] = [];
	lines: string[] = [];
	clears = 0;
	resets = 0;
	focuses = 0;
	fits = 0;
	cols = 80;
	rows = 24;
	screen: string | null = 'SCREEN';
	/** When set, output is drawn into this model and `done` reports it */
	echoScreen: FakeEchoScreen | null = null;
	/** Hold parsed-callbacks back until `drain()`, as xterm does between frames */
	deferParse = false;
	private parsing: (() => void)[] = [];
	private decoder = new TextDecoder();
	write(data: string | Uint8Array, done?: () => void) {
		this.written.push(data);
		const draw = () => {
			this.echoScreen?.print(
				typeof data === 'string' ? data : this.decoder.decode(data, { stream: true })
			);
			done?.();
		};
		if (this.deferParse) this.parsing.push(draw);
		else draw();
	}
	drain() {
		const queued = this.parsing;
		this.parsing = [];
		for (const draw of queued) draw();
	}
	writeln(line: string) {
		this.lines.push(line);
	}
	clear() {
		this.clears++;
	}
	reset() {
		this.resets++;
	}
	focus() {
		this.focuses++;
	}
	fit() {
		this.fits++;
	}
	serialize() {
		return this.screen;
	}
}

function fakeStore(): KeyValueStore & { map: Map<string, string> } {
	const map = new Map<string, string>();
	return {
		map,
		getItem: (k) => map.get(k) ?? null,
		setItem: (k, v) => void map.set(k, v),
		removeItem: (k) => void map.delete(k)
	};
}

interface Harness {
	session: PtySession;
	term: FakeTerminal;
	store: ReturnType<typeof fakeStore>;
	sockets: FakeSocket[];
	statuses: ConnectionStatus[];
	views: SessionView[];
	trust: Mock<(serverId: number, fingerprint: string) => Promise<unknown>>;
	server: { id: number | null };
	/** The predictive echo's screen, with `echo: true` */
	screen: FakeEchoScreen;
	echoMode: { value: EchoMode };
	/** How often a socket closed before it ever opened */
	refused: { count: number };
	/** The tab's file to edit, cleared when a shell starts (as the tab state does) */
	pendingEdit: { path: string | null };
	/** The newest socket */
	ws(): FakeSocket;
}

function harness(
	opts: { serverId?: number | null; trustFails?: boolean; echo?: boolean; edit?: string } = {}
): Harness {
	const pendingEdit = { path: opts.edit ?? null };
	const term = new FakeTerminal();
	const screen = new FakeEchoScreen();
	const echoMode = { value: 'always' as EchoMode };
	if (opts.echo) term.echoScreen = screen;
	const store = fakeStore();
	const sockets: FakeSocket[] = [];
	const statuses: ConnectionStatus[] = [];
	const views: SessionView[] = [];
	const server = { id: opts.serverId === undefined ? 7 : opts.serverId };
	const refused = { count: 0 };
	const trust = vi.fn(async (_serverId: number, _fingerprint: string): Promise<unknown> => {
		if (opts.trustFails) throw new Error('nope');
		return {};
	});
	const session = new PtySession({
		tabId: 'tab-1',
		serverId: () => server.id,
		terminal: term,
		socketUrl: async () => 'ws://test/ws/terminal/pty',
		trustHostKey: trust,
		edit: () => pendingEdit.path,
		onShellStarted: () => (pendingEdit.path = null),
		onStatus: (s) => statuses.push(s),
		onView: (v) => views.push(v),
		onRefused: () => refused.count++,
		createSocket: (url) => {
			const s = new FakeSocket(url);
			sockets.push(s);
			return s;
		},
		storage: store,
		echo: opts.echo ? { screen, mode: () => echoMode.value } : undefined,
		timers: {
			setTimeout: (fn, ms) => setTimeout(fn, ms),
			clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
			setInterval: (fn, ms) => setInterval(fn, ms),
			clearInterval: (h) => clearInterval(h as ReturnType<typeof setInterval>)
		},
		// No jitter: every delay is the backoff step itself
		random: () => 0.5
	});
	return {
		session,
		term,
		store,
		sockets,
		statuses,
		views,
		trust,
		server,
		screen,
		echoMode,
		refused,
		pendingEdit,
		ws: () => sockets[sockets.length - 1]
	};
}

/** Connect and bring a new shell up, as the server does */
async function connected(h: Harness, sessionId = 'sess-1') {
	await h.session.connect();
	h.ws().open();
	h.ws().reply({ type: 'connected', session_id: sessionId, data: 'Connected\r\n' });
}

beforeEach(() => {
	vi.useFakeTimers();
});
afterEach(() => {
	vi.useRealTimers();
});

// ── Tests ────────────────────────────────────────────────────────────────

describe('a new shell', () => {
	it('asks for a shell with binary frames and records the session it gets', async () => {
		const h = harness();
		await h.session.connect();
		expect(h.statuses).toEqual(['connecting']);
		expect(h.session.currentView.note).toBe('Connecting…');
		expect(h.ws().url).toBe('ws://test/ws/terminal/pty');
		expect(h.ws().binaryType).toBe('arraybuffer');
		expect(h.term.clears).toBe(1);

		h.ws().open();
		expect(h.ws().sent[0]).toEqual({
			type: 'shell',
			server_id: 7,
			cols: 80,
			rows: 24,
			binary: true
		});
		// Open, but the tab is not "connected" until the shell is up
		expect(h.session.status).toBe('connecting');
		expect(h.session.isOpen).toBe(true);

		h.ws().reply({ type: 'connected', session_id: 'sess-1', data: 'Connected\r\n' });
		expect(h.session.status).toBe('connected');
		expect(h.session.currentView.note).toBeNull();
		expect(h.term.written).toContain('Connected\r\n');
		expect(loadSessionRecord('tab-1', h.store)).toEqual({
			sessionId: 'sess-1',
			lastSeq: 0,
			serverId: 7
		});
	});

	it('shows whether the server records the session, until it ends', async () => {
		const h = harness();
		await connected(h);
		expect(h.session.currentView.recording).toBe(false);

		const r = harness();
		await r.session.connect();
		r.ws().open();
		r.ws().reply({ type: 'connected', session_id: 'sess-2', data: '', recording: true });
		expect(r.session.currentView.recording).toBe(true);
		r.ws().reply({ type: 'closed', data: 'Shell exited with status 0' });
		expect(r.session.currentView.recording).toBe(false);
	});

	it("opens the tab's file in the editor once, even when the first try is refused", async () => {
		const h = harness({ edit: "/etc/it's.conf" });
		await h.session.connect();
		h.ws().open();
		expect(h.ws().sent[0]).toMatchObject({ type: 'shell', edit: "/etc/it's.conf" });
		// The host key waits for confirmation: no shell yet, so the edit is still owed
		h.ws().reply({
			type: 'error',
			code: 'host_key_unconfirmed',
			data: '',
			details: {
				server_id: 7,
				server_name: 'web',
				key_type: 'ssh-ed25519',
				fingerprint: 'SHA256:x'
			}
		});
		expect(h.pendingEdit.path).toBe("/etc/it's.conf");
		await h.session.trustHostKey();
		h.ws().open();
		expect(h.ws().sent[0]).toMatchObject({ type: 'shell', edit: "/etc/it's.conf" });
		h.ws().reply({ type: 'connected', session_id: 'sess-1', data: '' });
		expect(h.pendingEdit.path).toBeNull();

		// The shell exits; Reconnect starts a plain shell
		h.ws().reply({ type: 'closed', data: 'Shell exited' });
		await h.session.connect();
		h.ws().open();
		expect(h.ws().sent[0]).toEqual({
			type: 'shell',
			server_id: 7,
			cols: 80,
			rows: 24,
			binary: true
		});
	});

	it('does nothing without a server', async () => {
		const h = harness({ serverId: null });
		await h.session.connect();
		expect(h.sockets).toHaveLength(0);
		expect(h.statuses).toEqual([]);
	});

	it('reports a socket that cannot be created', async () => {
		const h = harness();
		const session = new PtySession({
			tabId: 'tab-1',
			serverId: () => 7,
			terminal: h.term,
			socketUrl: async () => 'bad',
			trustHostKey: h.trust,
			onStatus: (s) => h.statuses.push(s),
			createSocket: () => {
				throw new Error('SyntaxError');
			},
			storage: h.store
		});
		await session.connect();
		expect(session.status).toBe('error');
		expect(h.term.lines.join()).toContain('Failed to create WebSocket');
	});
});

describe('output', () => {
	it('writes binary frames raw, tracks seq, and joins split characters in the history', async () => {
		const h = harness();
		await connected(h);
		const bytes = new TextEncoder().encode('héllo\r\n');
		// "é" is two bytes; split it across frames
		const cut = 2;
		h.ws().output(1, bytes.slice(0, cut));
		h.ws().output(2, bytes.slice(cut));
		expect(h.term.written.filter((w) => w instanceof Uint8Array)).toHaveLength(2);
		expect(h.session.getOutput()).toBe('héllo\r\n');

		h.session.persist();
		expect(loadSessionRecord('tab-1', h.store)?.lastSeq).toBe(2);
	});

	it('ignores a binary frame shorter than its header', async () => {
		const h = harness();
		await connected(h);
		const before = h.term.written.length;
		h.ws().onmessage?.({ data: new ArrayBuffer(4) } as MessageEvent);
		expect(h.term.written.length).toBe(before);
	});

	it('accepts JSON output frames (older servers) and strips CSI codes from the history', async () => {
		const h = harness();
		await connected(h);
		h.ws().reply({ type: 'output', data: '\x1b[31mred\x1b[0m', seq: 5 });
		expect(h.session.getOutput()).toBe('red');
		h.session.clearOutput();
		expect(h.session.getOutput()).toBe('');
	});

	it('reports a frame it cannot parse in the terminal', async () => {
		const h = harness();
		await connected(h);
		h.ws().onmessage?.({ data: '{not json' } as MessageEvent);
		expect(h.term.lines.join()).toContain('Error parsing server response');
	});
});

describe('input', () => {
	it('sends keystrokes and sizes only while the socket is open', async () => {
		const h = harness();
		h.session.sendInput('x');
		await h.session.connect();
		h.session.sendInput('y');
		expect(h.ws().sent).toEqual([]);
		h.ws().open();
		h.session.sendInput('ls\r');
		h.session.sendResize(100, 30);
		expect(h.ws().sent.slice(1)).toEqual([
			{ type: 'input', data: 'ls\r' },
			{ type: 'resize', cols: 100, rows: 30 }
		]);
		h.ws().drop();
		expect(h.session.isOpen).toBe(false);
		h.session.sendInput('z');
		expect(h.ws().sent).toHaveLength(3);
	});
});

describe('resume', () => {
	it('attaches to the saved session with the saved screen and seq', async () => {
		const h = harness();
		saveSessionRecord('tab-1', { sessionId: 'sess-9', lastSeq: 41, serverId: 7 }, h.store);
		h.store.setItem(savedBufferKey('tab-1'), 'OLD SCREEN');
		h.session.loadSession();
		expect(h.session.heldSessionId).toBe('sess-9');

		await h.session.connect();
		expect(h.session.currentView.note).toBe('Resuming session…');
		expect(h.term.written).toEqual(['OLD SCREEN']);
		expect(h.store.map.has(savedBufferKey('tab-1'))).toBe(false);
		h.ws().open();
		expect(h.ws().sent[0]).toEqual({
			type: 'attach',
			session_id: 'sess-9',
			last_seq: 41,
			cols: 80,
			rows: 24,
			binary: true
		});

		// Attach answers "connected" without data; the banner stays until the replay ends
		h.ws().reply({ type: 'connected', session_id: 'sess-9' });
		expect(h.session.currentView.note).toBe('Resuming session…');
		h.ws().reply({ type: 'replay_start' });
		expect(h.term.resets).toBe(0); // screen restored, so no redraw from the top
		h.ws().reply({ type: 'replay_end' });
		expect(h.session.currentView.note).toBeNull();
		// Two window-size changes so every program repaints
		expect(h.ws().sent.slice(-1)).toEqual([{ type: 'resize', cols: 80, rows: 23 }]);
		vi.advanceTimersByTime(60);
		expect(h.ws().sent.slice(-1)).toEqual([{ type: 'resize', cols: 80, rows: 24 }]);
	});

	it('asks for the whole ring when the saved screen is missing', async () => {
		const h = harness();
		saveSessionRecord('tab-1', { sessionId: 'sess-9', lastSeq: 41, serverId: 7 }, h.store);
		h.session.loadSession();
		await h.session.connect();
		h.ws().open();
		expect(h.ws().sent[0]).toMatchObject({ type: 'attach', last_seq: 0 });
		h.ws().reply({ type: 'replay_start' });
		expect(h.term.resets).toBe(1);
	});

	it('starts over when the ring no longer reaches back far enough', async () => {
		const h = harness();
		saveSessionRecord('tab-1', { sessionId: 'sess-9', lastSeq: 41, serverId: 7 }, h.store);
		h.store.setItem(savedBufferKey('tab-1'), 'OLD');
		h.session.loadSession();
		await h.session.connect();
		h.ws().open();
		h.ws().reply({ type: 'replay_window_lost' });
		expect(h.term.resets).toBe(1);
		h.session.persist();
		expect(loadSessionRecord('tab-1', h.store)?.lastSeq).toBe(0);
	});

	it('does not resume a session opened on a different server', async () => {
		const h = harness();
		saveSessionRecord('tab-1', { sessionId: 'sess-9', lastSeq: 41, serverId: 3 }, h.store);
		h.session.loadSession();
		await h.session.connect();
		h.ws().open();
		expect(h.ws().sent[0]).toMatchObject({ type: 'shell', server_id: 7 });
		expect(h.session.heldSessionId).toBeNull();
	});

	it('falls back to a new shell when the session is gone', async () => {
		const h = harness();
		saveSessionRecord('tab-1', { sessionId: 'sess-9', lastSeq: 41, serverId: 7 }, h.store);
		h.session.loadSession();
		await h.session.connect();
		h.ws().open();
		h.ws().reply({ type: 'error', code: 'session_gone', data: 'gone' });
		expect(h.session.heldSessionId).toBeNull();
		expect(h.term.resets).toBe(1);
		expect(h.session.currentView.note).toBe('Previous session ended — starting a new shell');
		// Binary frames like every other shell, so split multibyte characters stay intact
		expect(h.ws().sent[1]).toEqual({
			type: 'shell',
			server_id: 7,
			cols: 80,
			rows: 24,
			binary: true
		});
	});
});

describe('kept shells', () => {
	it('keeps the session when a kept shell cannot be reconnected, so Reconnect tries it again', async () => {
		const h = harness();
		saveSessionRecord('tab-1', { sessionId: 'kept-1', lastSeq: 41, serverId: 7 }, h.store);
		h.session.loadSession();
		await h.session.connect();
		h.ws().open();
		expect(h.ws().sent[0]).toMatchObject({ type: 'attach', session_id: 'kept-1' });
		h.ws().reply({ type: 'error', code: 'resume_failed', data: 'server down' });
		expect(h.session.heldSessionId).toBe('kept-1');
		expect(h.session.currentView.status).toBe('error');
		expect(h.session.currentView.showReconnect).toBe(true);
		expect(h.term.resets).toBe(0);
	});
});

describe('keep-alive', () => {
	it('pings every interval and drops a socket whose pong never comes', async () => {
		const h = harness();
		await connected(h);
		vi.advanceTimersByTime(PING_INTERVAL);
		expect(h.ws().types()).toContain('ping');

		// Answered: the watchdog is disarmed
		h.ws().reply({ type: 'pong' });
		vi.advanceTimersByTime(PONG_TIMEOUT);
		expect(h.ws().closed).toBe(false);

		// Unanswered: the socket is closed and the close reconnects
		vi.advanceTimersByTime(PING_INTERVAL - PONG_TIMEOUT);
		vi.advanceTimersByTime(PONG_TIMEOUT);
		expect(h.ws().closed).toBe(true);
		expect(h.session.currentView.note).toBe('Connection unresponsive — reconnecting');
		h.ws().closeEvent();
		expect(h.session.currentView.note).toBe(`Reconnecting… (1/${MAX_RECONNECT_ATTEMPTS})`);
		await vi.advanceTimersByTimeAsync(RECONNECT_DELAYS[0]);
		expect(h.sockets).toHaveLength(2);
	});

	it('checks the socket when the page becomes visible again', async () => {
		const h = harness();
		await connected(h);
		h.session.visibilityChanged(false);
		expect(h.ws().types()).toEqual(['shell', 'ping']);

		// Died in the background without a close event
		h.ws().readyState = 3;
		h.session.visibilityChanged(false);
		expect(h.session.currentView.note).toBe(`Reconnecting… (1/${MAX_RECONNECT_ATTEMPTS})`);
		await vi.advanceTimersByTimeAsync(RECONNECT_DELAYS[0]);
		expect(h.sockets).toHaveLength(2);
	});

	it('saves the screen when the page is hidden', async () => {
		const h = harness();
		await connected(h);
		h.session.visibilityChanged(true);
		expect(h.store.getItem(savedBufferKey('tab-1'))).toBe('SCREEN');
	});
});

describe('reconnect', () => {
	it('backs off through every step, then offers the overlay', async () => {
		const h = harness();
		await connected(h);
		h.ws().drop();
		expect(h.session.status).toBe('disconnected');
		expect(h.session.currentView.note).toBe(`Reconnecting… (1/${MAX_RECONNECT_ATTEMPTS})`);

		for (let i = 0; i < MAX_RECONNECT_ATTEMPTS; i++) {
			await vi.advanceTimersByTimeAsync(RECONNECT_DELAYS[i] - 1);
			expect(h.sockets).toHaveLength(i + 1);
			await vi.advanceTimersByTimeAsync(1);
			expect(h.sockets).toHaveLength(i + 2);
			// A reconnect keeps the screen: no clear, no restore
			expect(h.term.clears).toBe(1);
			h.ws().drop(); // refused before it opened
		}
		expect(h.session.currentView.showReconnect).toBe(true);
		expect(h.session.currentView.note).toBeNull();
		await vi.advanceTimersByTimeAsync(60_000);
		expect(h.sockets).toHaveLength(MAX_RECONNECT_ATTEMPTS + 1);
	});

	it('resumes the same session after a drop and resets the backoff once open', async () => {
		const h = harness();
		await connected(h);
		h.ws().output(12, new TextEncoder().encode('x'));
		h.ws().drop();
		await vi.advanceTimersByTimeAsync(RECONNECT_DELAYS[0]);
		h.ws().open();
		expect(h.ws().sent[0]).toMatchObject({ type: 'attach', session_id: 'sess-1', last_seq: 12 });
		h.ws().reply({ type: 'connected', session_id: 'sess-1' });
		h.ws().drop();
		// Counter started over: the first step again
		expect(h.session.currentView.note).toBe(`Reconnecting… (1/${MAX_RECONNECT_ATTEMPTS})`);
	});

	it('does not retry a connection refused before it ever opened', async () => {
		const h = harness();
		await h.session.connect();
		h.ws().drop();
		expect(h.session.status).toBe('error');
		expect(h.session.currentView.note).toBe('Could not connect');
		await vi.advanceTimersByTimeAsync(60_000);
		expect(h.sockets).toHaveLength(1);
	});

	it('asks whether the sign-in still holds when a socket is refused, never when one drops', async () => {
		const h = harness();
		await connected(h);
		// An established connection dropping says nothing about the sign-in
		h.ws().drop();
		expect(h.refused.count).toBe(0);
		// The reconnect is refused before it opens (a proxy's login redirect looks like this)
		await vi.advanceTimersByTimeAsync(RECONNECT_DELAYS[0]);
		h.ws().drop();
		expect(h.refused.count).toBe(1);

		// A first connect refused outright asks too
		const fresh = harness();
		await fresh.session.connect();
		fresh.ws().drop();
		expect(fresh.refused.count).toBe(1);
	});

	it('ignores events from a socket it has replaced', async () => {
		const h = harness();
		await connected(h);
		const old = h.ws();
		const lateMessage = old.onmessage!;
		const lateClose = old.onclose!;
		await h.session.connect();
		expect(old.closed).toBe(true);
		expect(old.types()).toContain('detach');
		expect(old.onmessage).toBeNull();
		const statuses = h.statuses.length;
		// An event already queued for the old socket changes nothing
		lateMessage({ data: JSON.stringify({ type: 'closed', data: '' }) } as MessageEvent);
		lateClose({} as CloseEvent);
		expect(h.statuses.length).toBe(statuses);
		expect(h.session.currentView.showReconnect).toBe(false);
		await vi.advanceTimersByTimeAsync(60_000);
		expect(h.sockets).toHaveLength(2);
	});

	it('jitters each step by at most 15 %', () => {
		expect(reconnectDelay(0, () => 0)).toBe(850);
		expect(reconnectDelay(0, () => 1)).toBe(1150);
		expect(reconnectDelay(99, () => 0.5)).toBe(RECONNECT_DELAYS[RECONNECT_DELAYS.length - 1]);
	});
});

describe('host key on first contact', () => {
	const details = {
		server_id: 4,
		server_name: 'jump',
		key_type: 'ssh-ed25519',
		fingerprint: 'SHA256:abc'
	};

	it('shows the challenge, trusts the key and connects again', async () => {
		const h = harness();
		await h.session.connect();
		h.ws().open();
		h.ws().reply({
			type: 'error',
			code: 'host_key_unconfirmed',
			data: 'unknown host key',
			details
		});
		expect(h.session.status).toBe('error');
		expect(h.session.currentView.hostKey).toEqual(details);
		expect(h.session.currentView.note).toBeNull();

		await h.session.trustHostKey();
		expect(h.trust).toHaveBeenCalledWith(4, 'SHA256:abc');
		expect(h.session.currentView.hostKey).toBeNull();
		expect(h.term.lines.join()).toContain('Host key trusted');
		expect(h.sockets).toHaveLength(2);
	});

	it('keeps the challenge when trusting fails', async () => {
		const h = harness({ trustFails: true });
		await h.session.connect();
		h.ws().open();
		h.ws().reply({ type: 'error', code: 'host_key_unconfirmed', data: '', details });
		await expect(h.session.trustHostKey()).rejects.toThrow('nope');
		expect(h.session.currentView.hostKey).toEqual(details);
		expect(h.sockets).toHaveLength(1);
	});

	it('can be dismissed', async () => {
		const h = harness();
		await h.session.connect();
		h.ws().open();
		h.ws().reply({ type: 'error', code: 'host_key_unconfirmed', data: '', details });
		h.session.dismissHostKey();
		expect(h.session.currentView.hostKey).toBeNull();
	});
});

describe('ending', () => {
	it('stops and offers the overlay when the shell exits', async () => {
		const h = harness();
		await connected(h);
		h.ws().reply({ type: 'closed', data: 'Shell exited' });
		expect(h.term.lines.join()).toContain('[Shell exited]');
		expect(h.session.status).toBe('disconnected');
		expect(h.session.currentView.showReconnect).toBe(true);
		expect(loadSessionRecord('tab-1', h.store)).toBeNull();
		await vi.advanceTimersByTimeAsync(60_000);
		expect(h.sockets).toHaveLength(1);
	});

	it('disconnect ends the shell and forgets it', async () => {
		const h = harness();
		await connected(h);
		const ws = h.ws();
		h.session.disconnect();
		expect(ws.types()).toContain('close');
		expect(ws.closed).toBe(true);
		expect(h.session.status).toBe('disconnected');
		expect(loadSessionRecord('tab-1', h.store)).toBeNull();
	});

	it('detach keeps the shell and the record for a reattach', async () => {
		const h = harness();
		await connected(h);
		const ws = h.ws();
		h.session.detach();
		expect(ws.types()).toContain('detach');
		expect(loadSessionRecord('tab-1', h.store)?.sessionId).toBe('sess-1');
		expect(h.store.getItem(savedBufferKey('tab-1'))).toBe('SCREEN');
	});

	it('refuses to connect after dispose, even from a pending reconnect', async () => {
		const h = harness();
		await h.session.connect();
		h.session.dispose();
		await h.session.connect();
		expect(h.sockets).toHaveLength(1);
	});

	it('a new session forgets the old one and starts a shell', async () => {
		const h = harness();
		await connected(h);
		h.ws().reply({ type: 'closed', data: '' });
		await h.session.startFresh();
		h.ws().open();
		expect(h.ws().sent[0]).toMatchObject({ type: 'shell' });
		expect(h.session.currentView.showReconnect).toBe(false);
	});

	it('does not keep a screen too large for sessionStorage', async () => {
		const h = harness();
		await connected(h);
		h.store.setItem(savedBufferKey('tab-1'), 'stale');
		h.term.screen = 'x'.repeat(MAX_SAVED_BUFFER + 1);
		h.session.persist();
		expect(h.store.map.has(savedBufferKey('tab-1'))).toBe(false);
		expect(loadSessionRecord('tab-1', h.store)?.sessionId).toBe('sess-1');
	});

	it('clearSaved removes the screen and the record', async () => {
		const h = harness();
		await connected(h);
		h.session.persist();
		h.session.clearSaved();
		expect(h.store.map.size).toBe(0);
	});
});

describe('predictive echo', () => {
	const enc = (text: string) => new TextEncoder().encode(text);

	/** A shell that is up and has echoed once, so its epoch is confirmed */
	async function echoing(h: Harness) {
		await connected(h);
		h.ws().output(1, enc('$ '));
		h.session.sendInput('l');
		h.ws().output(2, enc('l'));
	}

	it('draws typed characters ahead of the echo and withdraws them once it is drawn', async () => {
		const h = harness({ echo: true });
		await echoing(h);
		h.session.sendInput('s');
		expect(h.screen.drawn).toBe('s');
		expect(h.ws().sent.at(-1)).toEqual({ type: 'input', data: 's' });

		h.ws().output(3, enc('s'));
		expect(h.screen.drawn).toBe('');
		expect(h.session.predictedCount).toBe(0);
	});

	it('checks predictions against output sent as JSON frames too', async () => {
		const h = harness({ echo: true });
		await echoing(h);
		h.session.sendInput('s');
		h.ws().reply({ type: 'output', data: 's', seq: 3 });
		expect(h.session.predictedCount).toBe(0);
	});

	it('predicts nothing before the shell is up', async () => {
		const h = harness({ echo: true });
		await h.session.connect();
		h.ws().open();
		h.session.sendInput('x');
		expect(h.ws().sent.at(-1)).toEqual({ type: 'input', data: 'x' });
		expect(h.session.predictedCount).toBe(0);
	});

	it('does not start predicting from a cursor the terminal has not drawn yet', async () => {
		const h = harness({ echo: true });
		await echoing(h);
		h.term.deferParse = true;
		h.ws().output(3, enc('\r\nmail\r\n$ '));
		h.session.sendInput('s'); // the cursor still reads as before the output
		expect(h.session.predictedCount).toBe(0);
		h.term.drain();
		h.term.deferParse = false;
		h.session.sendInput('w');
		expect(h.session.predictedCount).toBe(1);
	});

	it('withdraws predictions when the connection drops, and starts over after it', async () => {
		const h = harness({ echo: true });
		await echoing(h);
		h.session.sendInput('s');
		expect(h.screen.drawn).toBe('s');
		h.ws().drop();
		expect(h.screen.drawn).toBe('');
		expect(h.session.predictedCount).toBe(0);

		await vi.advanceTimersByTimeAsync(RECONNECT_DELAYS[0]);
		h.ws().open();
		h.ws().reply({ type: 'connected', session_id: 'sess-1', data: '' });
		h.ws().reply({ type: 'replay_start' });
		h.ws().output(3, enc('s'));
		h.ws().reply({ type: 'replay_end' });
		h.session.sendInput(' ');
		expect(h.screen.drawn).toBe(''); // a new epoch: waits for its own echo
	});

	it('withdraws predictions when the terminal is resized', async () => {
		const h = harness({ echo: true });
		await echoing(h);
		h.session.sendInput('s');
		h.session.sendResize(100, 30);
		expect(h.screen.drawn).toBe('');
		expect(h.session.predictedCount).toBe(0);
	});

	it('follows the preference: nothing when it is off', async () => {
		const h = harness({ echo: true });
		h.echoMode.value = 'off';
		await echoing(h);
		h.session.sendInput('s');
		expect(h.session.predictedCount).toBe(0);
		expect(h.screen.shown.every((cells) => cells.length === 0)).toBe(true);
	});

	it('writes output without a parse callback when there is no echo', async () => {
		const h = harness();
		const write = vi.spyOn(h.term, 'write');
		await connected(h);
		h.ws().output(1, enc('$ '));
		expect(write.mock.calls.at(-1)).toHaveLength(1);
		h.session.sendInput('x');
		expect(h.session.predictedCount).toBe(0);
	});
});
