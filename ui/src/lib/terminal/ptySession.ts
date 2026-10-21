/**
 * The WebSocket side of a terminal pane: one shell on `/ws/terminal/pty`, kept alive and
 * resumable. It connects (a new `shell`, or an `attach` to the session this tab held), replays
 * what the server's ring still has, pings with a pong watchdog, reconnects with backoff after a
 * drop, stops at the host-key challenge until the user trusts the key, and keeps the session
 * record and the serialized screen in sessionStorage so a reload picks the shell up again.
 *
 * It knows nothing about xterm: the pane hands it a `TerminalSurface` to write into and reads
 * the view state (`SessionView`) back through `onView`. With an `EchoScreen` it also runs the
 * predictive local echo (`localEcho.ts`): keystrokes are predicted as they are sent and the
 * predictions checked each time the terminal has drawn the server's output. Everything that touches the outside
 * world (socket, URL, timers, storage, the trust call) can be replaced, so the protocol is
 * tested without a browser.
 */

import type { ConnectionStatus, PtyRequest, PtyResponse } from '#lib/types/index.js';
import { LocalEcho, type EchoMode, type EchoScreen } from './localEcho.js';
import {
	loadSessionRecord,
	saveSessionRecord,
	type KeyValueStore
} from '#lib/utils/sessionRecord.js';

export const PING_INTERVAL = 30_000;
/** A ping unanswered for this long means the socket is dead even if the browser says OPEN. */
export const PONG_TIMEOUT = 8_000;
export const MAX_OUTPUT_HISTORY = 10_000;
export const RECONNECT_DELAYS = [1000, 2000, 4000, 8000, 16000, 30000];
export const MAX_RECONNECT_ATTEMPTS = RECONNECT_DELAYS.length;
/** A serialized screen larger than this is not kept; a resume redraws from the replay ring. */
export const MAX_SAVED_BUFFER = 512 * 1024;

/** `WebSocket.OPEN`, spelled out so the module does not need the browser global. */
const OPEN = 1;

/** The key whose fingerprint the user must confirm before the server opens a shell. */
export type HostKeyChallenge = NonNullable<PtyResponse['details']>;

/** What the pane draws from the session, beyond the terminal's own contents. */
export interface SessionView {
	status: ConnectionStatus;
	/** Connection progress, shown over the pane instead of written into the shell's buffer */
	note: string | null;
	/** "Session disconnected" with Reconnect / New Session */
	showReconnect: boolean;
	/** First contact: the server refused the shell until this key is confirmed */
	hostKey: HostKeyChallenge | null;
	/** The server is recording this session's output */
	recording: boolean;
}

/** The part of the terminal the session writes into. */
export interface TerminalSurface {
	/** `done` runs once the terminal has parsed (and so drawn) this data */
	write(data: string | Uint8Array, done?: () => void): void;
	writeln(line: string): void;
	clear(): void;
	reset(): void;
	focus(): void;
	fit(): void;
	readonly cols: number;
	readonly rows: number;
	/** The screen as escape sequences for sessionStorage, or null when it cannot be serialized */
	serialize(): string | null;
}

/** The subset of `WebSocket` the session uses. */
export interface SocketLike {
	readyState: number;
	binaryType: BinaryType;
	onopen: ((ev: Event) => unknown) | null;
	onmessage: ((ev: MessageEvent) => unknown) | null;
	onerror: ((ev: Event) => unknown) | null;
	onclose: ((ev: CloseEvent) => unknown) | null;
	send(data: string): void;
	close(): void;
}

export interface Timers {
	setTimeout(fn: () => void, ms: number): unknown;
	clearTimeout(handle: unknown): void;
	setInterval(fn: () => void, ms: number): unknown;
	clearInterval(handle: unknown): void;
}

export interface PtySessionOptions {
	tabId: string;
	/** The server the tab points at now; read on every connect */
	serverId: () => number | null;
	terminal: TerminalSurface;
	/** The `/ws/terminal/pty` URL with whatever credential the platform needs */
	socketUrl: () => Promise<string>;
	/** `POST /servers/{id}/host-keys/trust` */
	trustHostKey: (serverId: number, fingerprint: string) => Promise<unknown>;
	/** Every status change, as it happens (also repeated values) */
	onStatus?: (status: ConnectionStatus) => void;
	/** The view changed; called with a fresh object */
	onView?: (view: SessionView) => void;
	/** Predictive local echo over this screen; none without it */
	echo?: { screen: EchoScreen; mode: () => EchoMode };
	/** A socket closed before it ever opened. The browser does not say why (an expired
	 *  sign-in, a proxy's login redirect, the server down), so the pane asks the API. */
	onRefused?: () => void;
	// Seams for tests; the browser's own by default
	createSocket?: (url: string) => SocketLike;
	storage?: KeyValueStore;
	timers?: Timers;
	random?: () => number;
}

const browserTimers: Timers = {
	setTimeout: (fn, ms) => setTimeout(fn, ms),
	clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
	setInterval: (fn, ms) => setInterval(fn, ms),
	clearInterval: (h) => clearInterval(h as ReturnType<typeof setInterval>)
};

export function savedBufferKey(tabId: string): string {
	return `svrctlrs-term-buffer-${tabId}`;
}

// eslint-disable-next-line no-control-regex -- ANSI escape sequences start with ESC
const ANSI_CSI = /\x1b\[[0-9;]*[a-zA-Z]/g;

/** Reconnect delay for an attempt (0-based): the backoff step with ±15 % jitter. */
export function reconnectDelay(attempt: number, random: () => number = Math.random): number {
	const base = RECONNECT_DELAYS[Math.min(attempt, RECONNECT_DELAYS.length - 1)];
	return Math.round(base * (0.85 + random() * 0.3));
}

export class PtySession {
	private readonly opts: PtySessionOptions;
	private readonly term: TerminalSurface;
	private readonly timers: Timers;
	private readonly random: () => number;
	private readonly createSocket: (url: string) => SocketLike;

	private view: SessionView = {
		status: 'disconnected',
		note: null,
		showReconnect: false,
		hostKey: null,
		recording: false
	};

	private socket: SocketLike | null = null;
	private disposed = false;
	/** Bumped by every connect(); handlers of an older socket ignore their events. */
	private connectGen = 0;
	/** The current socket has opened and input/resize may be sent through it */
	private wired = false;
	private intentionalDisconnect = false;
	private reconnectAttempt = 0;
	private reconnectTimeout: unknown = null;
	private pingInterval: unknown = null;
	private pongTimeout: unknown = null;
	/** Set by onerror when an established session drops; onclose fires after and reads it. */
	private lostWhileConnected = false;

	/** The server-side session this pane is (or was last) attached to, and how much of its
	 *  output has been shown. Both persist in sessionStorage so a reload resumes the shell. */
	private sessionId: string | null = null;
	private lastSeq = 0;
	/** The server the session was opened on; the tab may be pointed elsewhere later */
	private sessionServerId: number | null = null;

	private outputHistory: string[] = [];
	/** Decodes binary output frames for the copy/download history; xterm gets the raw bytes.
	 *  Streaming, so a character split across frames is joined; renewed per connection. */
	private outputDecoder = new TextDecoder();

	/** Predictive local echo, when the pane gave us a screen to check predictions against */
	private readonly echo: LocalEcho | null;
	/** Server output handed to the terminal and not yet parsed by it */
	private writesInFlight = 0;

	constructor(opts: PtySessionOptions) {
		this.opts = opts;
		this.term = opts.terminal;
		this.timers = opts.timers ?? browserTimers;
		this.random = opts.random ?? Math.random;
		this.createSocket = opts.createSocket ?? ((url) => new WebSocket(url) as SocketLike);
		this.echo = opts.echo
			? new LocalEcho({
					screen: opts.echo.screen,
					mode: opts.echo.mode,
					busy: () => this.writesInFlight > 0,
					timers: this.timers
				})
			: null;
	}

	// ── State ────────────────────────────────────────────────────────────

	get status(): ConnectionStatus {
		return this.view.status;
	}

	get currentView(): SessionView {
		return { ...this.view };
	}

	/** The session id this tab resumes, if any */
	get heldSessionId(): string | null {
		return this.sessionId;
	}

	/** Keystrokes predicted and not yet confirmed by the shell's echo (drawn or not) */
	get predictedCount(): number {
		return this.echo?.pendingCount ?? 0;
	}

	/** True between the socket opening and closing: keystrokes go through */
	get isOpen(): boolean {
		return this.wired && this.socket?.readyState === OPEN;
	}

	private update(patch: Partial<SessionView>) {
		this.view = { ...this.view, ...patch };
		this.opts.onView?.({ ...this.view });
	}

	private setNote(note: string | null) {
		this.update({ note });
	}

	private setStatus(status: ConnectionStatus) {
		this.update({ status });
		this.opts.onStatus?.(status);
	}

	private get storage(): KeyValueStore | null {
		if (this.opts.storage) return this.opts.storage;
		return typeof sessionStorage === 'undefined' ? null : sessionStorage;
	}

	// ── Session record and saved screen ──────────────────────────────────

	/** Pick up the session record a previous page (or a resume) left for this tab. */
	loadSession(): void {
		const saved = loadSessionRecord(this.opts.tabId, this.opts.storage);
		if (saved) {
			this.sessionId = saved.sessionId;
			this.lastSeq = saved.lastSeq;
			this.sessionServerId = saved.serverId;
		}
	}

	private saveSession() {
		saveSessionRecord(
			this.opts.tabId,
			this.sessionId
				? { sessionId: this.sessionId, lastSeq: this.lastSeq, serverId: this.sessionServerId }
				: null,
			this.opts.storage
		);
	}

	/** Drop the session this tab resumes; the next connect starts a new shell. */
	forgetSession(): void {
		this.sessionId = null;
		this.lastSeq = 0;
		this.saveSession();
	}

	/** Serialize the screen and the session record to sessionStorage (tab hidden, unload). */
	persist(): void {
		const store = this.storage;
		if (!store) return;
		try {
			const data = this.term.serialize();
			if (data === null) return;
			if (data.length > MAX_SAVED_BUFFER) {
				// Too big to keep: a resume redraws from the server's replay ring instead
				store.removeItem(savedBufferKey(this.opts.tabId));
			} else {
				store.setItem(savedBufferKey(this.opts.tabId), data);
			}
			this.saveSession();
		} catch {
			// sessionStorage full or unavailable
		}
	}

	/** Write the saved screen back once; true when something was written */
	private restoreBuffer(): boolean {
		const store = this.storage;
		const key = savedBufferKey(this.opts.tabId);
		const data = store?.getItem(key);
		if (data && store) {
			this.term.write(data);
			store.removeItem(key);
			return true;
		}
		return false;
	}

	/** Remove the saved screen and the session record (the tab is closed for good). */
	clearSaved(): void {
		this.storage?.removeItem(savedBufferKey(this.opts.tabId));
		saveSessionRecord(this.opts.tabId, null, this.opts.storage);
	}

	// ── Output history (copy / download) ─────────────────────────────────

	private appendOutput(data: string) {
		this.outputHistory.push(data);
		if (this.outputHistory.length > MAX_OUTPUT_HISTORY) {
			this.outputHistory = this.outputHistory.slice(-MAX_OUTPUT_HISTORY);
		}
	}

	clearOutput(): void {
		this.outputHistory = [];
	}

	/** Everything the shell printed this connection, without CSI escape sequences */
	getOutput(): string {
		return this.outputHistory.map((s) => s.replace(ANSI_CSI, '')).join('');
	}

	// ── Sending ──────────────────────────────────────────────────────────

	private send(req: PtyRequest) {
		this.socket?.send(JSON.stringify(req));
	}

	/** Keystrokes from the terminal; dropped unless the socket is open. Printable ones are
	 *  predicted (drawn ahead of the echo) once the shell is up. */
	sendInput(data: string): void {
		if (!this.isOpen) return;
		if (this.view.status === 'connected') this.echo?.input(data);
		this.send({ type: 'input', data });
	}

	/** The terminal's new size; dropped unless the socket is open. */
	sendResize(cols: number, rows: number): void {
		// Lines reflow at a new width: whatever was predicted is no longer where it was
		this.echo?.reset();
		if (!this.isOpen) return;
		this.send({ type: 'resize', cols, rows });
	}

	/** The shell's output into the terminal; predictions are checked once it is drawn. */
	private writeOutput(data: string | Uint8Array) {
		if (!this.echo) {
			this.term.write(data);
			return;
		}
		this.writesInFlight++;
		this.term.write(data, () => {
			this.writesInFlight = Math.max(0, this.writesInFlight - 1);
			if (this.writesInFlight === 0) this.echo?.settled();
		});
	}

	// ── Keep-alive ───────────────────────────────────────────────────────

	private clearPongTimeout() {
		if (this.pongTimeout) {
			this.timers.clearTimeout(this.pongTimeout);
			this.pongTimeout = null;
		}
	}

	private stopPinging() {
		if (this.pingInterval) {
			this.timers.clearInterval(this.pingInterval);
			this.pingInterval = null;
		}
	}

	/** Ping and arm the watchdog; a missing pong tears the socket down so onclose reconnects. */
	private sendPing() {
		const socket = this.socket;
		if (!socket || socket.readyState !== OPEN) return;
		socket.send(JSON.stringify({ type: 'ping' }));
		if (this.pongTimeout) return;
		this.pongTimeout = this.timers.setTimeout(() => {
			this.pongTimeout = null;
			if (!this.socket || this.intentionalDisconnect) return;
			this.setNote('Connection unresponsive — reconnecting');
			this.lostWhileConnected = this.view.status === 'connected';
			this.socket.close();
		}, PONG_TIMEOUT);
	}

	/**
	 * The page became visible (`hidden` false) or was hidden. Hidden: save the screen. Visible:
	 * make sure the socket is really alive. Browsers throttle or drop background sockets without
	 * firing onclose promptly, which left a terminal that looked connected but swallowed every
	 * keystroke.
	 */
	visibilityChanged(hidden: boolean): void {
		if (hidden) {
			this.persist();
		} else if (this.view.status === 'connected' || this.view.status === 'error') {
			this.checkAlive();
		}
	}

	private checkAlive() {
		if (this.intentionalDisconnect) return;
		// A connect is in flight: let it finish rather than starting a second one
		if (this.view.status === 'connecting' || this.reconnectTimeout) return;
		if (!this.socket || this.socket.readyState !== OPEN) {
			// Died in the background without a close event we acted on
			this.lostWhileConnected = true;
			this.reconnectAttempt = 0;
			this.attemptReconnect();
			return;
		}
		this.sendPing();
	}

	// ── Reconnect ────────────────────────────────────────────────────────

	private attemptReconnect() {
		if (this.reconnectAttempt >= MAX_RECONNECT_ATTEMPTS) {
			this.update({ note: null, showReconnect: true });
			return;
		}
		// ±15% jitter so many panes dropped at once do not retry in lockstep
		const delay = reconnectDelay(this.reconnectAttempt, this.random);
		this.reconnectAttempt++;
		this.setNote(`Reconnecting… (${this.reconnectAttempt}/${MAX_RECONNECT_ATTEMPTS})`);
		this.reconnectTimeout = this.timers.setTimeout(() => {
			this.reconnectTimeout = null;
			void this.connect();
		}, delay);
	}

	private stopReconnecting() {
		this.intentionalDisconnect = true;
		this.reconnectAttempt = 0;
		if (this.reconnectTimeout) {
			this.timers.clearTimeout(this.reconnectTimeout);
			this.reconnectTimeout = null;
		}
	}

	// ── Host key ─────────────────────────────────────────────────────────

	/** The user compared the fingerprint and accepts it: trust the key, then connect again.
	 *  Rejects when the trust call fails; the challenge stays up. */
	async trustHostKey(): Promise<void> {
		const challenge = this.view.hostKey;
		if (!challenge || this.opts.serverId() === null) return;
		await this.opts.trustHostKey(challenge.server_id, challenge.fingerprint);
		this.update({ hostKey: null });
		this.term.writeln('\x1b[32m✓ Host key trusted\x1b[0m');
		await this.connect();
	}

	/** The user declined the key: close the dialog, stay disconnected. */
	dismissHostKey(): void {
		this.update({ hostKey: null });
	}

	// ── Connect / disconnect ─────────────────────────────────────────────

	/** Open the PTY WebSocket for the tab's server (new shell, or reattach) */
	async connect(): Promise<void> {
		const serverId = this.opts.serverId();
		if (this.disposed || serverId === null) return;
		this.update({ hostKey: null, showReconnect: false });

		// Cancel any pending reconnect
		if (this.reconnectTimeout) {
			this.timers.clearTimeout(this.reconnectTimeout);
			this.reconnectTimeout = null;
		}

		this.intentionalDisconnect = false;
		this.cleanupSocket(false);
		// A session resumes only on the server it was opened on
		if (this.sessionId && this.sessionServerId !== null && this.sessionServerId !== serverId) {
			this.forgetSession();
		}
		this.setStatus('connecting');
		const gen = ++this.connectGen;

		if (this.reconnectAttempt === 0) {
			this.term.clear();
			this.outputHistory = [];
			// The saved buffer and the saved lastSeq were written together; without the buffer
			// the screen is empty, so ask the server for everything it still has.
			if (!this.restoreBuffer()) this.lastSeq = 0;
		}
		// Progress goes in the banner, never into the shell's buffer: a resumed screen must look
		// exactly as it did, prompt and cursor included.
		this.setNote(this.sessionId ? 'Resuming session…' : 'Connecting…');

		let wsUrl: string;
		try {
			wsUrl = await this.opts.socketUrl();
		} catch {
			// The desktop app's ticket request failed (server unreachable, or signed out: the API
			// client has already sent a signed-out app to its login page). Try again like a
			// dropped socket would; the token never goes into the URL instead.
			if (this.disposed || this.intentionalDisconnect || gen !== this.connectGen) return;
			this.attemptReconnect();
			return;
		}
		// The ticket request yields; the tab may have been closed or reconnected meanwhile.
		if (this.disposed || this.intentionalDisconnect || gen !== this.connectGen) return;

		let ws: SocketLike;
		try {
			ws = this.createSocket(wsUrl);
		} catch (e) {
			this.term.writeln(`\r\n\x1b[31mFailed to create WebSocket: ${e}\x1b[0m`);
			this.setNote(null);
			this.setStatus('error');
			return;
		}
		// PTY output comes as binary frames (seq header + raw bytes); control frames stay JSON
		ws.binaryType = 'arraybuffer';
		this.outputDecoder = new TextDecoder();
		this.socket = ws;
		const stale = () => ws !== this.socket;

		ws.onopen = () => {
			if (stale()) return;
			// The tab counts as connected once the shell is up, not when the socket opens
			this.reconnectAttempt = 0;
			const { cols, rows } = this.term;
			// Resume the server-side session if we have one, otherwise start a shell
			const req: PtyRequest = this.sessionId
				? {
						type: 'attach',
						session_id: this.sessionId,
						last_seq: this.lastSeq,
						cols,
						rows,
						binary: true
					}
				: { type: 'shell', server_id: serverId, cols, rows, binary: true };
			ws.send(JSON.stringify(req));
			this.wired = true;
			this.term.focus();
			// Start keep-alive pings; each one arms a watchdog that the pong disarms
			this.pingInterval = this.timers.setInterval(() => this.sendPing(), PING_INTERVAL);
		};

		ws.onmessage = (event: MessageEvent) => {
			if (stale()) return;
			if (event.data instanceof ArrayBuffer) {
				this.handleBinary(event.data);
				return;
			}
			try {
				this.handleMessage(ws, serverId, JSON.parse(event.data) as PtyResponse);
			} catch {
				this.term.writeln('\r\n\x1b[31mError parsing server response\x1b[0m');
			}
		};

		ws.onerror = () => {
			if (stale()) return;
			// onclose follows and decides about reconnecting; remember that this was an
			// established session, since the status is about to read 'error'.
			if (this.view.status === 'connected') this.lostWhileConnected = true;
			this.setNote('Connection error');
			this.setStatus('error');
		};

		ws.onclose = () => {
			if (stale()) return;
			const wasConnected = this.view.status === 'connected' || this.lostWhileConnected;
			this.lostWhileConnected = false;
			this.setStatus('disconnected');
			this.wired = false;
			this.echo?.reset();
			this.stopPinging();
			this.clearPongTimeout();

			if (wasConnected && !this.intentionalDisconnect) {
				this.setNote('Connection lost');
				this.attemptReconnect();
			} else if (!this.intentionalDisconnect && this.reconnectAttempt > 0) {
				// Reconnect attempt failed before fully connecting
				this.opts.onRefused?.();
				this.attemptReconnect();
			} else if (!this.intentionalDisconnect) {
				// Refused before it ever connected (auth, origin, server down)
				this.opts.onRefused?.();
				this.setNote('Could not connect');
				this.setStatus('error');
			} else {
				this.setNote(null);
			}
		};
	}

	/** Numbered PTY output: 8-byte big-endian seq, then the shell's bytes as produced */
	private handleBinary(buf: ArrayBuffer) {
		if (buf.byteLength < 8) return;
		const seq = Number(new DataView(buf).getBigUint64(0));
		const bytes = new Uint8Array(buf, 8);
		this.writeOutput(bytes);
		this.appendOutput(this.outputDecoder.decode(bytes, { stream: true }));
		this.lastSeq = seq;
	}

	private handleMessage(ws: SocketLike, serverId: number, res: PtyResponse) {
		switch (res.type) {
			case 'output':
				this.writeOutput(res.data);
				this.appendOutput(res.data);
				if (res.seq !== undefined) this.lastSeq = res.seq;
				break;
			case 'connected':
				this.setStatus('connected');
				this.update({ recording: res.recording === true });
				if (res.session_id) {
					this.sessionId = res.session_id;
					this.sessionServerId = serverId;
					this.saveSession();
				}
				this.writeOutput(res.data);
				if (res.data) this.setNote(null); // a new shell; a resume clears at replay_end
				this.term.focus();
				break;
			case 'replay_start':
				this.echo?.reset();
				// Nothing of this session on screen yet: the ring is redrawn from the top
				if (this.lastSeq === 0) {
					this.term.reset();
					this.outputHistory = [];
				}
				break;
			case 'replay_window_lost':
				// Too far behind the ring; the server now resends what it still has
				this.echo?.reset();
				this.term.reset();
				this.outputHistory = [];
				this.lastSeq = 0;
				break;
			case 'replay_end':
				this.setNote(null);
				this.term.focus();
				this.nudgeRedraw();
				break;
			case 'closed':
				this.setNote(null);
				this.update({ recording: false });
				this.term.writeln(`\r\n\x1b[33m[${res.data || 'Session ended'}]\x1b[0m`);
				this.forgetSession();
				this.stopReconnecting();
				this.cleanupSocket(false);
				this.setStatus('disconnected');
				this.update({ showReconnect: true });
				break;
			case 'error':
				if (res.code === 'session_gone') {
					// Kept too long or killed elsewhere; fall back to a fresh shell
					this.echo?.reset();
					this.forgetSession();
					this.term.reset();
					this.outputHistory = [];
					this.setNote('Previous session ended — starting a new shell');
					this.term.writeln('\x1b[33m[Previous session has ended — starting a new shell]\x1b[0m');
					const { cols, rows } = this.term;
					const req: PtyRequest = { type: 'shell', server_id: serverId, cols, rows, binary: true };
					ws.send(JSON.stringify(req));
				} else {
					this.term.write(res.data);
					if (res.code === 'host_key_unconfirmed' && res.details) {
						this.update({ hostKey: res.details });
					}
					if (!this.sessionId) {
						// The shell never came up (auth, host key, unreachable)
						this.setNote(null);
						this.setStatus('error');
					}
				}
				break;
			case 'pong':
				this.clearPongTimeout();
				break;
		}
	}

	/**
	 * After a resume the screen is restored from the saved buffer plus replayed output, but the
	 * shell does not know anyone is looking. Two window-size changes give every program a
	 * SIGWINCH, so prompts, editors and `top` repaint themselves at the real size.
	 */
	private nudgeRedraw() {
		const ws = this.socket;
		if (!ws || ws.readyState !== OPEN) return;
		this.term.fit();
		const { cols, rows } = this.term;
		ws.send(JSON.stringify({ type: 'resize', cols, rows: Math.max(2, rows - 1) }));
		this.timers.setTimeout(() => {
			if (ws.readyState === OPEN) ws.send(JSON.stringify({ type: 'resize', cols, rows }));
		}, 60);
	}

	/** Drop the socket; `kill` ends the shell too, otherwise the server keeps it for reattach. */
	private cleanupSocket(kill: boolean) {
		this.stopPinging();
		this.wired = false;
		this.echo?.reset();
		const socket = this.socket;
		if (socket) {
			if (socket.readyState === OPEN) {
				socket.send(JSON.stringify({ type: kill ? 'close' : 'detach' }));
			}
			socket.onclose = null;
			socket.onerror = null;
			socket.onmessage = null;
			socket.close();
			this.socket = null;
		}
	}

	/** End the shell and the connection (Disconnect button, tab close). */
	disconnect(): void {
		this.persist();
		this.stopReconnecting();
		this.cleanupSocket(true);
		this.forgetSession();
		this.setNote(null);
		this.setStatus('disconnected');
	}

	/** Leave the shell running on the server and drop only this connection (pane unmount). */
	detach(): void {
		this.persist();
		this.stopReconnecting();
		this.cleanupSocket(false);
		this.setStatus('disconnected');
	}

	/** The pane is going away: detach and refuse any later connect (a queued frame, a timer). */
	dispose(): void {
		this.detach();
		this.echo?.dispose();
		this.disposed = true;
	}

	/** "New Session" on the disconnected overlay: forget the old shell and start another. */
	async startFresh(): Promise<void> {
		this.forgetSession();
		this.term.clear();
		this.outputHistory = [];
		await this.connect();
	}
}
