<script lang="ts">
	import { onMount } from 'svelte';
	import { Terminal } from '@xterm/xterm';
	import { FitAddon } from '@xterm/addon-fit';
	import { SearchAddon } from '@xterm/addon-search';
	import { WebLinksAddon } from '@xterm/addon-web-links';
	import { WebglAddon } from '@xterm/addon-webgl';
	import { Unicode11Addon } from '@xterm/addon-unicode11';
	import { ClipboardAddon } from '@xterm/addon-clipboard';
	import { ImageAddon } from '@xterm/addon-image';
	import { SerializeAddon } from '@xterm/addon-serialize';
	import '@xterm/xterm/css/xterm.css';

	import { tokyoNightTheme, lightTheme } from './terminal-theme.js';
	import * as themeState from '#lib/state/theme.svelte.js';
	import * as terminalPrefs from '#lib/state/terminalPrefs.svelte.js';
	import * as terminalState from '#lib/state/terminal.svelte.js';
	import { isMobile } from '#lib/state/mobile.svelte.js';
	import { getServerUrl } from '#lib/platform/index.js';
	import { post } from '#lib/api/client.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { loadSessionRecord, saveSessionRecord } from '#lib/utils/sessionRecord.js';
	import type { ConnectionStatus, PtyRequest, PtyResponse } from '#lib/types/index.js';

	interface Props {
		tabId: string;
		serverId: number | null;
		active?: boolean;
		onStatusChange?: (tabId: string, status: ConnectionStatus) => void;
		/** The pending CTL/ALT from the extra-keys row was used up by a typed character */
		onModifierConsumed?: () => void;
		/** A horizontal swipe switched tabs; the view re-focuses the new one */
		onSwipe?: () => void;
		onOpenPalette?: () => void;
	}

	let {
		tabId,
		serverId,
		active = true,
		onStatusChange,
		onModifierConsumed,
		onSwipe,
		onOpenPalette
	}: Props = $props();

	let showReconnectOverlay = $state(false);
	/** Connection progress shown over the pane instead of being written into the shell's buffer. */
	let statusNote = $state<string | null>(null);
	/** Bumped by every connect(); handlers of an older socket ignore their events. */
	let connectGen = 0;
	/** Set from the mobile extra-keys row: the next typed character becomes Ctrl+/Alt+ it. */
	let pendingModifier: 'ctl' | 'alt' | null = null;
	let shellEl: HTMLDivElement;

	// ── Gesture state machine (PointerEvent-based, capture phase) ───────
	// Capture phase intercepts touch events BEFORE xterm.js sees them.
	// Mouse events pass through untouched for native selection/scrolling.
	// Touch events: tap → focus, double-tap → palette, vertical → scroll,
	// horizontal → tab swipe. Axis locks early and never switches.
	type GestureMode = 'idle' | 'pending' | 'vertical-scroll' | 'horizontal-swipe';

	let gestureMode: GestureMode = 'idle';
	let scrollAccum = 0;
	let startX = 0;
	let startY = 0;
	let lastY = 0;
	let startTime = 0;
	let lastTapTime = 0;

	const TAP_MAX_MS = 220; // max pointer-down duration to count as a tap (not a drag)
	const DOUBLE_TAP_MS = 350; // window between taps for double-tap detection
	const LOCK_DISTANCE = 10;
	const AXIS_BIAS = 1.2;
	const SWIPE_COMMIT_PX = 60;
	const CELL_HEIGHT_FALLBACK = 18;
	const CELL_WIDTH_FALLBACK = 9;
	const LONG_PRESS_MS = 500;
	let longPressTimer: ReturnType<typeof setTimeout> | null = null;
	let longPressFired = false;

	function getCellSize(): { width: number; height: number } {
		try {
			// xterm has no public cell-size API; this reaches into its render service.
			const internals = terminal as unknown as {
				_core?: {
					_renderService?: {
						dimensions?: { css?: { cell?: { height?: number; width?: number } } };
					};
				};
			};
			const cell = internals?._core?._renderService?.dimensions?.css?.cell;
			if (cell?.height && cell?.width) return { width: cell.width, height: cell.height };
		} catch {
			/* ignore */
		}
		return { width: CELL_WIDTH_FALLBACK, height: CELL_HEIGHT_FALLBACK };
	}

	function getCellHeight(): number {
		return getCellSize().height;
	}

	/** The buffer cell under a point on screen, or null when outside the text area. */
	function cellAt(clientX: number, clientY: number): { col: number; row: number } | null {
		const screen = terminal?.element?.querySelector('.xterm-screen');
		if (!terminal || !screen) return null;
		const rect = screen.getBoundingClientRect();
		const { width, height } = getCellSize();
		const col = Math.floor((clientX - rect.left) / width);
		const row = Math.floor((clientY - rect.top) / height);
		if (col < 0 || row < 0 || col >= terminal.cols || row >= terminal.rows) return null;
		return { col, row };
	}

	/**
	 * Tap on the line being edited: move the shell's cursor there with arrow keys. Only in the
	 * normal buffer (a full-screen program gets nothing) and only on the cursor's own row.
	 */
	function moveCursorTo(clientX: number, clientY: number) {
		if (!terminal || !socket || socket.readyState !== WebSocket.OPEN) return;
		const buf = terminal.buffer.active;
		if (buf.type !== 'normal') return;
		const cell = cellAt(clientX, clientY);
		if (!cell) return;
		const cursorViewportRow = buf.baseY + buf.cursorY - buf.viewportY;
		if (cell.row !== cursorViewportRow) return;
		const delta = cell.col - buf.cursorX;
		if (delta === 0 || Math.abs(delta) > 200) return;
		const seq = delta > 0 ? '\x1b[C' : '\x1b[D';
		injectInput(seq.repeat(Math.abs(delta)));
	}

	/** Long-press: select the word under the finger and copy it. */
	function copyWordAt(clientX: number, clientY: number) {
		if (!terminal) return;
		const cell = cellAt(clientX, clientY);
		if (!cell) return;
		const buf = terminal.buffer.active;
		const absRow = buf.viewportY + cell.row;
		const text = buf.getLine(absRow)?.translateToString(true) ?? '';
		if (cell.col >= text.length || /\s/.test(text[cell.col])) return;
		let start = cell.col;
		let end = cell.col;
		while (start > 0 && !/\s/.test(text[start - 1])) start--;
		while (end < text.length - 1 && !/\s/.test(text[end + 1])) end++;
		const word = text.slice(start, end + 1);
		terminal.select(start, absRow, word.length);
		navigator.clipboard
			?.writeText(word)
			.then(() => toast.success(`Copied “${word.length > 40 ? word.slice(0, 40) + '…' : word}”`))
			.catch(() => toast.error('Clipboard unavailable'));
		if (navigator.vibrate) navigator.vibrate(15);
	}

	function clearLongPress() {
		if (longPressTimer) {
			clearTimeout(longPressTimer);
			longPressTimer = null;
		}
	}

	/**
	 * Focus for typing. Android closes the soft keyboard on Back while the textarea keeps focus,
	 * and `focus()` on an already-focused element shows nothing — so blur first. Must run inside
	 * the user's gesture for the keyboard to appear.
	 */
	function focusForTyping() {
		if (!terminal) return;
		const ta = terminal.textarea;
		if (ta && document.activeElement === ta) ta.blur();
		terminal.focus();
		terminal.scrollToBottom();
	}

	/** Controls drawn over the terminal (reconnect overlay, banners) must get their taps. */
	function isPassthroughTarget(e: Event): boolean {
		const t = e.target as Element | null;
		return !!t?.closest?.('button, a, input, select, [role="dialog"], [data-touch-passthrough]');
	}

	function handlePointerDown(e: PointerEvent) {
		if (e.pointerType !== 'touch' || isPassthroughTarget(e)) return;
		e.stopPropagation(); // prevent xterm.js from seeing this touch
		gestureMode = 'pending';
		scrollAccum = 0;
		startX = e.clientX;
		startY = lastY = e.clientY;
		startTime = performance.now();
		longPressFired = false;
		clearLongPress();
		const { clientX, clientY } = e;
		longPressTimer = setTimeout(() => {
			longPressTimer = null;
			if (gestureMode !== 'pending') return;
			longPressFired = true;
			copyWordAt(clientX, clientY);
		}, LONG_PRESS_MS);
		shellEl.setPointerCapture(e.pointerId);
	}

	function handlePointerMove(e: PointerEvent) {
		if (e.pointerType !== 'touch' || gestureMode === 'idle') return;
		e.stopPropagation();

		const dx = e.clientX - startX;
		const dy = e.clientY - startY;
		const adx = Math.abs(dx);
		const ady = Math.abs(dy);

		if (gestureMode === 'pending') {
			if (adx < LOCK_DISTANCE && ady < LOCK_DISTANCE) return;
			clearLongPress();

			if (ady > adx * AXIS_BIAS) {
				gestureMode = 'vertical-scroll';
			} else if (adx > ady * AXIS_BIAS) {
				gestureMode = 'horizontal-swipe';
			} else {
				return;
			}
		}

		if (gestureMode === 'vertical-scroll' && terminal) {
			// Accumulate sub-cell movement so slow drags still scroll
			scrollAccum += -(e.clientY - lastY);
			const cellH = getCellHeight();
			const lines = Math.trunc(scrollAccum / cellH);
			if (lines !== 0) {
				scrollAccum -= lines * cellH;
				if (terminal.buffer.active.type === 'alternate') {
					// less/man/vim: there is no scrollback, the program scrolls itself
					const key = lines > 0 ? arrowSeq('B') : arrowSeq('A');
					injectInput(key.repeat(Math.min(Math.abs(lines), 20)));
				} else {
					terminal.scrollLines(lines);
				}
			}
			e.preventDefault();
		}

		if (gestureMode === 'horizontal-swipe') {
			e.preventDefault();
		}

		lastY = e.clientY;
	}

	function handlePointerUp(e: PointerEvent) {
		if (e.pointerType !== 'touch' || gestureMode === 'idle') return;
		e.stopPropagation();
		clearLongPress();

		const elapsed = performance.now() - startTime;
		const moved = Math.hypot(e.clientX - startX, e.clientY - startY);

		if (longPressFired) {
			longPressFired = false;
		} else if (gestureMode === 'pending' && elapsed <= TAP_MAX_MS && moved < LOCK_DISTANCE) {
			const now = performance.now();
			if (now - lastTapTime <= DOUBLE_TAP_MS) {
				// Double-tap → command palette
				onOpenPalette?.();
				lastTapTime = 0;
			} else {
				// Single tap → focus now (the keyboard only appears inside the gesture) and place
				// the cursor; a following double-tap still opens the palette on top.
				lastTapTime = now;
				focusForTyping();
				moveCursorTo(e.clientX, e.clientY);
			}
		}

		if (gestureMode === 'horizontal-swipe') {
			const totalDx = e.clientX - startX;
			if (Math.abs(totalDx) > SWIPE_COMMIT_PX) {
				if (totalDx < 0) terminalState.nextTab();
				else terminalState.prevTab();
				onSwipe?.();
			}
		}

		gestureMode = 'idle';
	}

	let containerEl: HTMLDivElement;
	let terminal: Terminal | null = null;
	let fitAddon: FitAddon | null = null;
	let searchAddon: SearchAddon | null = null;
	let serializeAddon: SerializeAddon | null = null;
	let webglAddon: WebglAddon | null = null;
	let socket: WebSocket | null = null;
	let resizeObserver: ResizeObserver | null = null;
	let resizeTimer: ReturnType<typeof setTimeout> | null = null;
	let pingInterval: ReturnType<typeof setInterval> | null = null;
	let outputHistory: string[] = [];
	/** Decodes binary output frames for the copy/download history; xterm gets the raw bytes.
	 *  Streaming, so a character split across frames is joined; renewed per connection. */
	let outputDecoder = new TextDecoder();
	let status = $state<ConnectionStatus>('disconnected');
	let ptyInputDisposable: { dispose: () => void } | null = null;
	let ptyResizeDisposable: { dispose: () => void } | null = null;
	let intentionalDisconnect = false;
	let reconnectAttempt = 0;
	let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;

	const PING_INTERVAL = 30_000;
	/** A ping unanswered for this long means the socket is dead even if the browser says OPEN. */
	const PONG_TIMEOUT = 8_000;
	let pongTimeout: ReturnType<typeof setTimeout> | null = null;
	const MAX_OUTPUT_HISTORY = 10_000;
	const RECONNECT_DELAYS = [1000, 2000, 4000, 8000, 16000, 30000];
	const MAX_RECONNECT_ATTEMPTS = RECONNECT_DELAYS.length;
	/** Set by onerror when an established session drops; onclose fires after and reads it. */
	let lostWhileConnected = false;
	/** The server-side session this pane is (or was last) attached to, and how much of its
	 *  output has been shown. Both persist in sessionStorage so a reload resumes the shell. */
	let sessionId: string | null = null;
	let lastSeq = 0;
	/** The server the session was opened on; the tab may be pointed elsewhere later */
	let sessionServerId: number | null = null;

	function saveSession() {
		saveSessionRecord(tabId, sessionId ? { sessionId, lastSeq, serverId: sessionServerId } : null);
	}

	function loadSession() {
		const saved = loadSessionRecord(tabId);
		if (saved) {
			sessionId = saved.sessionId;
			lastSeq = saved.lastSeq;
			sessionServerId = saved.serverId;
		}
	}

	function forgetSession() {
		sessionId = null;
		lastSeq = 0;
		saveSession();
	}

	function setStatus(s: ConnectionStatus) {
		status = s;
		onStatusChange?.(tabId, s);
	}

	async function getWsUrl(): Promise<string> {
		const serverUrl = getServerUrl();
		const path = '/ws/terminal/pty';
		let url: string;
		if (serverUrl) {
			const wsBase = serverUrl.replace(/^http/, 'ws');
			url = `${wsBase}${path}`;
		} else {
			const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
			url = `${protocol}//${window.location.host}${path}`;
		}
		// The browser relies on the session cookie. Native apps cannot set headers on a
		// WebSocket handshake, so they exchange their token for a one-time ticket that is
		// bound to them and expires in seconds — nothing durable ends up in the URL.
		const token = localStorage.getItem('svrctlrs-session-token');
		if (token) {
			try {
				const { ticket } = await post<{ ticket: string }>('/auth/ws-ticket');
				url += `?ticket=${encodeURIComponent(ticket)}`;
			} catch {
				// Server without ticket support: fall back to the token (deprecated).
				url += `?token=${encodeURIComponent(token)}`;
			}
		}
		return url;
	}

	let webglLosses = 0;
	function tryLoadWebgl() {
		if (!terminal) return;
		try {
			webglAddon = new WebglAddon();
			terminal.loadAddon(webglAddon);
			webglAddon.onContextLoss(() => {
				webglAddon?.dispose();
				webglAddon = null;
				// Phones cap live WebGL contexts; after a few losses stay on the DOM renderer
				if (++webglLosses <= 3) setTimeout(() => tryLoadWebgl(), 1000);
			});
		} catch {
			webglAddon = null;
		}
	}

	function initTerminal() {
		if (!containerEl) return;

		const p = terminalPrefs.getPrefs();
		terminal = new Terminal({
			cursorBlink: p.cursorBlink,
			cursorStyle: p.cursorStyle,
			fontSize: p.fontSize,
			fontFamily: p.fontFamily,
			fontWeight: p.fontWeight,
			fontWeightBold: p.fontWeightBold,
			lineHeight: p.lineHeight,
			letterSpacing: p.letterSpacing,
			scrollback: p.scrollback,
			scrollSensitivity: p.scrollSensitivity,
			fastScrollSensitivity: p.fastScrollSensitivity,
			smoothScrollDuration: p.smoothScrollDuration,
			scrollOnUserInput: p.scrollOnUserInput,
			tabStopWidth: p.tabStopWidth,
			drawBoldTextInBrightColors: p.drawBoldTextInBrightColors,
			minimumContrastRatio: p.minimumContrastRatio,
			allowProposedApi: true,
			theme: themeState.isDark() ? tokyoNightTheme : lightTheme
		});

		fitAddon = new FitAddon();
		terminal.loadAddon(fitAddon);

		searchAddon = new SearchAddon();
		terminal.loadAddon(searchAddon);

		serializeAddon = new SerializeAddon();
		terminal.loadAddon(serializeAddon);

		terminal.loadAddon(new WebLinksAddon());
		terminal.loadAddon(new Unicode11Addon());
		terminal.loadAddon(new ClipboardAddon());

		try {
			terminal.loadAddon(
				new ImageAddon({
					enableSizeReports: true,
					pixelLimit: 16777216,
					sixelSupport: true,
					sixelScrolling: true,
					sixelPaletteLimit: 256,
					sixelSizeLimit: 25000000,
					storageLimit: isMobile() ? 16 : 128,
					showPlaceholder: true,
					iipSupport: true,
					iipSizeLimit: 20000000
				})
			);
		} catch {
			// ImageAddon may not be supported in all environments
		}

		terminal.open(containerEl);
		tryLoadWebgl();

		fit();
		// The WebGL renderer builds its glyph atlas from whatever font is loaded when it starts.
		// The bundled web font usually arrives later, so the atlas holds the fallback font's
		// metrics while glyphs are drawn from the real one — misaligned columns and garbled text
		// on real mobile GPUs. Once the font settles, rebuild the atlas and refit.
		document.fonts?.ready.then(() => refreshForFontChange());

		resizeObserver = new ResizeObserver(() => {
			if (resizeTimer) clearTimeout(resizeTimer);
			resizeTimer = setTimeout(() => fit(), 100);
		});
		resizeObserver.observe(containerEl);
	}

	/**
	 * The font (or its metrics) changed under a live WebGL renderer: its glyph atlas was baked
	 * with the old font, so drop it and let xterm rebuild it before refitting. Harmless on the
	 * DOM renderer, which has no atlas.
	 */
	function refreshForFontChange() {
		if (!terminal) return;
		try {
			webglAddon?.clearTextureAtlas();
		} catch {
			// renderer gone; ignore
		}
		fit();
	}

	function appendOutput(data: string) {
		outputHistory.push(data);
		if (outputHistory.length > MAX_OUTPUT_HISTORY) {
			outputHistory = outputHistory.slice(-MAX_OUTPUT_HISTORY);
		}
	}

	function clearPongTimeout() {
		if (pongTimeout) {
			clearTimeout(pongTimeout);
			pongTimeout = null;
		}
	}

	/** Ping and arm the watchdog; a missing pong tears the socket down so onclose reconnects. */
	function sendPing() {
		if (!socket || socket.readyState !== WebSocket.OPEN) return;
		socket.send(JSON.stringify({ type: 'ping' }));
		if (pongTimeout) return;
		pongTimeout = setTimeout(() => {
			pongTimeout = null;
			if (!socket || intentionalDisconnect) return;
			statusNote = 'Connection unresponsive — reconnecting';
			lostWhileConnected = status === 'connected';
			socket.close();
		}, PONG_TIMEOUT);
	}

	/** Called when the tab becomes visible again. */
	function checkAlive() {
		if (intentionalDisconnect) return;
		// A connect is in flight: let it finish rather than starting a second one
		if (status === 'connecting' || reconnectTimeout) return;
		if (!socket || socket.readyState !== WebSocket.OPEN) {
			// Died in the background without a close event we acted on
			lostWhileConnected = true;
			reconnectAttempt = 0;
			attemptReconnect();
			return;
		}
		sendPing();
	}

	function attemptReconnect() {
		if (reconnectAttempt >= MAX_RECONNECT_ATTEMPTS) {
			statusNote = null;
			showReconnectOverlay = true;
			return;
		}
		// ±15% jitter so many panes dropped at once do not retry in lockstep
		const base = RECONNECT_DELAYS[Math.min(reconnectAttempt, RECONNECT_DELAYS.length - 1)];
		const delay = Math.round(base * (0.85 + Math.random() * 0.3));
		reconnectAttempt++;
		statusNote = `Reconnecting… (${reconnectAttempt}/${MAX_RECONNECT_ATTEMPTS})`;
		reconnectTimeout = setTimeout(() => {
			reconnectTimeout = null;
			connect();
		}, delay);
	}

	/** Open the PTY WebSocket for the current serverId (new shell, or reattach) */
	export async function connect() {
		if (!terminal || serverId === null) return;
		showReconnectOverlay = false;

		// Cancel any pending reconnect
		if (reconnectTimeout) {
			clearTimeout(reconnectTimeout);
			reconnectTimeout = null;
		}

		intentionalDisconnect = false;
		cleanupSocket(false);
		// A session resumes only on the server it was opened on
		if (sessionId && sessionServerId !== null && sessionServerId !== serverId) forgetSession();
		setStatus('connecting');
		const gen = ++connectGen;

		if (reconnectAttempt === 0) {
			terminal.clear();
			outputHistory = [];
			// The saved buffer and the saved lastSeq were written together; without the buffer the
			// screen is empty, so ask the server for everything it still has.
			if (!restoreBuffer()) lastSeq = 0;
		}
		// Progress goes in the banner, never into the shell's buffer: a resumed screen must look
		// exactly as it did, prompt and cursor included.
		statusNote = sessionId ? 'Resuming session…' : 'Connecting…';

		const wsUrl = await getWsUrl();
		// The ticket request yields; the tab may have been closed or reconnected meanwhile.
		if (!terminal || intentionalDisconnect || gen !== connectGen) return;

		let ws: WebSocket;
		try {
			ws = new WebSocket(wsUrl);
		} catch (e) {
			terminal.writeln(`\r\n\x1b[31mFailed to create WebSocket: ${e}\x1b[0m`);
			statusNote = null;
			setStatus('error');
			return;
		}
		// PTY output comes as binary frames (seq header + raw bytes); control frames stay JSON
		ws.binaryType = 'arraybuffer';
		outputDecoder = new TextDecoder();
		socket = ws;
		const stale = () => ws !== socket;

		ws.onopen = () => {
			if (stale()) return;
			// The tab counts as connected once the shell is up, not when the socket opens
			reconnectAttempt = 0;

			const { cols, rows } = terminal!;

			// Resume the server-side session if we have one, otherwise start a shell
			const req: PtyRequest = sessionId
				? { type: 'attach', session_id: sessionId, last_seq: lastSeq, cols, rows, binary: true }
				: { type: 'shell', server_id: serverId!, cols, rows, binary: true };
			ws.send(JSON.stringify(req));

			// Forward keyboard input to PTY
			cleanupPty();
			ptyInputDisposable = terminal!.onData((data: string) => {
				if (ws.readyState !== WebSocket.OPEN) return;
				const req: PtyRequest = { type: 'input', data: applyPendingModifier(data) };
				ws.send(JSON.stringify(req));
			});

			// Forward resize events
			ptyResizeDisposable = terminal!.onResize(({ cols, rows }) => {
				if (ws.readyState === WebSocket.OPEN) {
					const req: PtyRequest = { type: 'resize', cols, rows };
					ws.send(JSON.stringify(req));
				}
			});

			terminal!.focus();

			// Start keep-alive pings; each one arms a watchdog that the pong disarms
			pingInterval = setInterval(() => sendPing(), PING_INTERVAL);
		};

		ws.onmessage = (event: MessageEvent) => {
			if (stale()) return;
			if (event.data instanceof ArrayBuffer) {
				// Numbered PTY output: 8-byte big-endian seq, then the shell's bytes as produced
				if (event.data.byteLength < 8) return;
				const seq = Number(new DataView(event.data).getBigUint64(0));
				const bytes = new Uint8Array(event.data, 8);
				terminal!.write(bytes);
				appendOutput(outputDecoder.decode(bytes, { stream: true }));
				lastSeq = seq;
				return;
			}
			try {
				const res = JSON.parse(event.data) as PtyResponse;
				switch (res.type) {
					case 'output':
						terminal!.write(res.data);
						appendOutput(res.data);
						if (res.seq !== undefined) lastSeq = res.seq;
						break;
					case 'connected':
						setStatus('connected');
						if (res.session_id) {
							sessionId = res.session_id;
							sessionServerId = serverId;
							saveSession();
						}
						terminal!.write(res.data);
						if (res.data) statusNote = null; // a new shell; a resume clears at replay_end
						terminal!.focus();
						break;
					case 'replay_start':
						// Nothing of this session on screen yet: the ring is redrawn from the top
						if (lastSeq === 0) {
							terminal!.reset();
							outputHistory = [];
						}
						break;
					case 'replay_window_lost':
						// Too far behind the ring; the server now resends what it still has
						terminal!.reset();
						outputHistory = [];
						lastSeq = 0;
						break;
					case 'replay_end':
						statusNote = null;
						terminal!.focus();
						nudgeRedraw();
						break;
					case 'closed':
						statusNote = null;
						terminal!.writeln(`\r\n\x1b[33m[${res.data || 'Session ended'}]\x1b[0m`);
						forgetSession();
						stopReconnecting();
						cleanupSocket(false);
						setStatus('disconnected');
						showReconnectOverlay = true;
						break;
					case 'error':
						if (res.code === 'session_gone') {
							// Kept too long or killed elsewhere; fall back to a fresh shell
							forgetSession();
							terminal!.reset();
							outputHistory = [];
							statusNote = 'Previous session ended — starting a new shell';
							terminal!.writeln(
								'\x1b[33m[Previous session has ended — starting a new shell]\x1b[0m'
							);
							const { cols, rows } = terminal!;
							const req: PtyRequest = { type: 'shell', server_id: serverId!, cols, rows };
							ws.send(JSON.stringify(req));
						} else {
							terminal!.write(res.data);
							if (!sessionId) {
								// The shell never came up (auth, host key, unreachable)
								statusNote = null;
								setStatus('error');
							}
						}
						break;
					case 'pong':
						clearPongTimeout();
						break;
				}
			} catch {
				terminal!.writeln('\r\n\x1b[31mError parsing server response\x1b[0m');
			}
		};

		ws.onerror = () => {
			if (stale()) return;
			// onclose follows and decides about reconnecting; remember that this was an
			// established session, since the status is about to read 'error'.
			if (status === 'connected') lostWhileConnected = true;
			statusNote = 'Connection error';
			setStatus('error');
		};

		ws.onclose = () => {
			if (stale()) return;
			const wasConnected = status === 'connected' || lostWhileConnected;
			lostWhileConnected = false;
			setStatus('disconnected');
			cleanupPty();
			if (pingInterval) {
				clearInterval(pingInterval);
				pingInterval = null;
			}
			clearPongTimeout();

			if (wasConnected && !intentionalDisconnect) {
				statusNote = 'Connection lost';
				attemptReconnect();
			} else if (!intentionalDisconnect && reconnectAttempt > 0) {
				// Reconnect attempt failed before fully connecting
				attemptReconnect();
			} else if (!intentionalDisconnect) {
				// Refused before it ever connected (auth, origin, server down)
				statusNote = 'Could not connect';
				setStatus('error');
			} else {
				statusNote = null;
			}
		};
	}

	/**
	 * After a resume the screen is restored from the saved buffer plus replayed output, but the
	 * shell does not know anyone is looking. Two window-size changes give every program a
	 * SIGWINCH, so prompts, editors and `top` repaint themselves at the real size.
	 */
	function nudgeRedraw() {
		const ws = socket;
		if (!terminal || !ws || ws.readyState !== WebSocket.OPEN) return;
		fit();
		const { cols, rows } = terminal;
		ws.send(JSON.stringify({ type: 'resize', cols, rows: Math.max(2, rows - 1) }));
		setTimeout(() => {
			if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: 'resize', cols, rows }));
		}, 60);
	}

	/** Mobile extra keys: CTL/ALT apply to the next character typed on the soft keyboard. */
	export function setPendingModifier(mod: 'ctl' | 'alt' | null) {
		pendingModifier = mod;
	}

	function applyPendingModifier(data: string): string {
		if (!pendingModifier || data.length !== 1) return data;
		const mod = pendingModifier;
		pendingModifier = null;
		onModifierConsumed?.();
		if (mod === 'alt') return '\x1b' + data;
		const code = data.toUpperCase().charCodeAt(0) - 64;
		return code >= 0 && code <= 31 ? String.fromCharCode(code) : data;
	}

	function cleanupPty() {
		ptyInputDisposable?.dispose();
		ptyInputDisposable = null;
		ptyResizeDisposable?.dispose();
		ptyResizeDisposable = null;
	}

	/** Drop the socket; `kill` ends the shell too, otherwise the server keeps it for reattach. */
	function cleanupSocket(kill: boolean) {
		if (pingInterval) {
			clearInterval(pingInterval);
			pingInterval = null;
		}
		cleanupPty();

		if (socket) {
			if (socket.readyState === WebSocket.OPEN) {
				socket.send(JSON.stringify({ type: kill ? 'close' : 'detach' }));
			}
			socket.onclose = null;
			socket.onerror = null;
			socket.onmessage = null;
			socket.close();
			socket = null;
		}
	}

	function stopReconnecting() {
		intentionalDisconnect = true;
		reconnectAttempt = 0;
		if (reconnectTimeout) {
			clearTimeout(reconnectTimeout);
			reconnectTimeout = null;
		}
	}

	/** End the shell and the connection (Disconnect button, tab close). */
	export function disconnect() {
		serializeBuffer();
		stopReconnecting();
		cleanupSocket(true);
		forgetSession();
		statusNote = null;
		setStatus('disconnected');
	}

	/** Leave the shell running on the server and drop only this connection (pane unmount). */
	export function detach() {
		serializeBuffer();
		stopReconnecting();
		cleanupSocket(false);
		setStatus('disconnected');
	}

	/** Serialize terminal buffer to sessionStorage for later restoration */
	function serializeBuffer() {
		if (!serializeAddon || !terminal) return;
		try {
			const data = serializeAddon.serialize();
			if (data.length > 512 * 1024) {
				// Too big to keep: a resume redraws from the server's replay ring instead
				sessionStorage.removeItem(`svrctlrs-term-buffer-${tabId}`);
			} else {
				sessionStorage.setItem(`svrctlrs-term-buffer-${tabId}`, data);
			}
			saveSession();
		} catch {
			// sessionStorage full or unavailable
		}
	}

	/** Restore terminal buffer from sessionStorage; true when something was written */
	function restoreBuffer(): boolean {
		const key = `svrctlrs-term-buffer-${tabId}`;
		const data = sessionStorage.getItem(key);
		if (data && terminal) {
			terminal.write(data);
			sessionStorage.removeItem(key);
			return true;
		}
		return false;
	}

	/** Remove saved buffer and session record (called when tab is explicitly closed) */
	export function clearSavedBuffer() {
		sessionStorage.removeItem(`svrctlrs-term-buffer-${tabId}`);
		saveSessionRecord(tabId, null);
	}

	/** Plain arrow in the current cursor-key mode (DECCKM switches CSI to SS3). */
	function arrowSeq(final: 'A' | 'B' | 'C' | 'D'): string {
		return terminal?.modes.applicationCursorKeysMode ? `\x1bO${final}` : `\x1b[${final}`;
	}

	/**
	 * Inject keys as if typed: through xterm, so the extra-keys row, quick commands and gestures
	 * behave like the keyboard (scroll to the bottom, the same onData path). Plain arrows are
	 * rewritten for application cursor mode.
	 */
	export function injectInput(data: string) {
		if (!terminal) return;
		const plainArrow = data.length === 3 && data.startsWith('\x1b[') && 'ABCD'.includes(data[2]);
		const seq = plainArrow ? arrowSeq(data[2] as 'A' | 'B' | 'C' | 'D') : data;
		if (ptyInputDisposable) {
			terminal.input(seq, true);
		} else if (socket?.readyState === WebSocket.OPEN) {
			const req: PtyRequest = { type: 'input', data: seq };
			socket.send(JSON.stringify(req));
		}
	}

	export function clear() {
		terminal?.clear();
		outputHistory = [];
	}

	export function fit() {
		if (fitAddon && terminal) {
			try {
				fitAddon.fit();
			} catch {
				// ignore
			}
		}
	}

	export function focus() {
		focusForTyping();
	}

	export function search(term: string, caseSensitive = false): boolean {
		return searchAddon?.findNext(term, { caseSensitive, incremental: true }) ?? false;
	}

	export function searchNext(term: string, caseSensitive = false): boolean {
		return searchAddon?.findNext(term, { caseSensitive }) ?? false;
	}

	export function searchPrevious(term: string, caseSensitive = false): boolean {
		return searchAddon?.findPrevious(term, { caseSensitive }) ?? false;
	}

	export function clearSearch() {
		searchAddon?.clearDecorations();
	}

	export function getOutput(): string {
		// eslint-disable-next-line no-control-regex -- ANSI escape sequences start with ESC
		const stripAnsi = (s: string) => s.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '');
		return outputHistory.map(stripAnsi).join('');
	}

	export async function copyOutput() {
		const output = getOutput();
		try {
			await navigator.clipboard.writeText(output);
			terminal?.writeln('\r\n\x1b[32m✓ Copied to clipboard\x1b[0m');
		} catch {
			terminal?.writeln('\r\n\x1b[31m✗ Failed to copy\x1b[0m');
		}
	}

	export function downloadOutput() {
		const output = getOutput();
		const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
		const filename = `svrctlrs-terminal-${ts}.txt`;
		const blob = new Blob([output], { type: 'text/plain' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = filename;
		document.body.appendChild(a);
		a.click();
		document.body.removeChild(a);
		URL.revokeObjectURL(url);
		terminal?.writeln(`\r\n\x1b[32m✓ Downloaded: ${filename}\x1b[0m`);
	}

	// Refit when pane becomes active/visible
	$effect(() => {
		if (active && terminal) {
			// Small delay to let CSS layout settle
			requestAnimationFrame(() => fit());
		}
	});

	// Phones cap live WebGL contexts: only the visible pane keeps one
	$effect(() => {
		if (!terminal || !isMobile()) return;
		if (active && !webglAddon && webglLosses <= 3) {
			tryLoadWebgl();
		} else if (!active && webglAddon) {
			try {
				webglAddon.dispose();
			} catch {
				// ignore
			}
			webglAddon = null;
		}
	});

	// Update terminal theme when app theme changes
	$effect(() => {
		const currentTheme = themeState.getTheme();
		if (terminal) {
			terminal.options.theme = currentTheme === 'dark' ? tokyoNightTheme : lightTheme;
		}
	});

	// Live-update terminal when preferences change
	$effect(() => {
		const p = terminalPrefs.getPrefs();
		if (!terminal) return;
		terminal.options.fontSize = p.fontSize;
		terminal.options.fontFamily = p.fontFamily;
		terminal.options.fontWeight = p.fontWeight;
		terminal.options.fontWeightBold = p.fontWeightBold;
		terminal.options.lineHeight = p.lineHeight;
		terminal.options.letterSpacing = p.letterSpacing;
		terminal.options.cursorStyle = p.cursorStyle;
		terminal.options.cursorBlink = p.cursorBlink;
		terminal.options.scrollback = p.scrollback;
		terminal.options.scrollSensitivity = p.scrollSensitivity;
		terminal.options.fastScrollSensitivity = p.fastScrollSensitivity;
		terminal.options.smoothScrollDuration = p.smoothScrollDuration;
		terminal.options.scrollOnUserInput = p.scrollOnUserInput;
		terminal.options.tabStopWidth = p.tabStopWidth;
		terminal.options.drawBoldTextInBrightColors = p.drawBoldTextInBrightColors;
		terminal.options.minimumContrastRatio = p.minimumContrastRatio;
		// Font metrics may have changed; drop the WebGL atlas so it is rebuilt, then refit.
		requestAnimationFrame(() => refreshForFontChange());
	});

	onMount(() => {
		initTerminal();
		loadSession();
		// Coming back to the terminal page (or reloading it): pick the shell up where it was
		if (sessionId && serverId !== null) {
			requestAnimationFrame(() => connect());
		}

		// Capture-phase pointer listeners on the shell — intercepts touch
		// BEFORE xterm.js can handle it and focus the hidden textarea.
		// Mouse events are ignored (pointerType check) so xterm.js handles them.
		shellEl.addEventListener('pointerdown', handlePointerDown, { capture: true });
		shellEl.addEventListener('pointermove', handlePointerMove, { capture: true });
		shellEl.addEventListener('pointerup', handlePointerUp, { capture: true });
		shellEl.addEventListener('pointercancel', handlePointerUp, { capture: true });

		// Suppress TouchEvents in capture phase — they're SEPARATE from PointerEvents.
		// stopPropagation: prevents xterm.js from seeing touch events.
		// preventDefault: prevents browser from generating synthetic mouse events
		//   (mousedown/mouseup/click) which would also reach xterm.js and trigger focus.
		// passive: false is REQUIRED — browsers default touchstart to passive,
		//   which silently ignores preventDefault().
		const suppressTouch = (e: Event) => {
			if (isPassthroughTarget(e)) return;
			e.stopPropagation();
			e.preventDefault();
		};
		shellEl.addEventListener('touchstart', suppressTouch, { capture: true, passive: false });
		shellEl.addEventListener('touchmove', suppressTouch, { capture: true, passive: false });
		shellEl.addEventListener('touchend', suppressTouch, { capture: true, passive: false });

		// Serialize buffer when tab is hidden; on return, make sure the socket is really alive.
		// Browsers throttle or drop background sockets without firing onclose promptly, which
		// left a terminal that looked connected but swallowed every keystroke.
		const handleVisibilityChange = () => {
			if (document.hidden) {
				serializeBuffer();
			} else if (status === 'connected' || status === 'error') {
				checkAlive();
			}
		};
		const handleBeforeUnload = () => serializeBuffer();
		document.addEventListener('visibilitychange', handleVisibilityChange);
		window.addEventListener('beforeunload', handleBeforeUnload);

		return () => {
			shellEl.removeEventListener('pointerdown', handlePointerDown, { capture: true });
			shellEl.removeEventListener('pointermove', handlePointerMove, { capture: true });
			shellEl.removeEventListener('pointerup', handlePointerUp, { capture: true });
			shellEl.removeEventListener('pointercancel', handlePointerUp, { capture: true });
			shellEl.removeEventListener('touchstart', suppressTouch, { capture: true });
			shellEl.removeEventListener('touchmove', suppressTouch, { capture: true });
			shellEl.removeEventListener('touchend', suppressTouch, { capture: true });
			document.removeEventListener('visibilitychange', handleVisibilityChange);
			window.removeEventListener('beforeunload', handleBeforeUnload);
			detach();
			if (resizeTimer) clearTimeout(resizeTimer);
			resizeObserver?.disconnect();
			if (webglAddon) {
				try {
					webglAddon.dispose();
				} catch {
					// ignore
				}
			}
			terminal?.dispose();
		};
	});
</script>

<!-- Terminal shell: owns touch gesture recognition via capture-phase listeners.
     Mouse passes through to xterm.js. Touch is intercepted before xterm.js sees it. -->
<div
	class="terminal-shell h-full w-full bg-background relative"
	class:hidden={!active}
	bind:this={shellEl}
>
	<div class="terminal-viewport h-full w-full" bind:this={containerEl}></div>

	{#if statusNote}
		<div
			class="absolute top-2 right-2 z-10 px-2 py-1 rounded-sm bg-surface/90 border border-border text-[11px] text-text-secondary pointer-events-none"
			role="status"
			aria-live="polite"
		>
			<span
				class="inline-block w-1.5 h-1.5 rounded-full bg-warning mr-1.5 align-middle animate-pulse"
			></span>{statusNote}
		</div>
	{/if}

	{#if showReconnectOverlay}
		<div
			class="absolute inset-0 bg-background/80 flex flex-col items-center justify-center z-10 gap-3"
		>
			<p class="text-text-muted text-sm">Session disconnected</p>
			<div class="flex gap-2">
				<button
					class="px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-sm hover:opacity-90"
					onclick={() => connect()}
				>
					Reconnect
				</button>
				<button
					class="px-3 py-1.5 text-sm bg-secondary text-secondary-foreground rounded-sm hover:opacity-90"
					onclick={() => {
						forgetSession();
						clear();
						connect();
					}}
				>
					New Session
				</button>
			</div>
		</div>
	{/if}
</div>
