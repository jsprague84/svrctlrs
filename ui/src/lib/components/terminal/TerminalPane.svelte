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
	import * as themeState from '$lib/state/theme.svelte.js';
	import * as terminalPrefs from '$lib/state/terminalPrefs.svelte.js';
	import * as terminalState from '$lib/state/terminal.svelte.js';
	import { getServerUrl } from '$lib/platform/index.js';
	import type { TerminalMode, ConnectionStatus, CmdRequest, CmdResponse, PtyRequest, PtyResponse } from '$lib/types/index.js';

	interface Props {
		tabId: string;
		serverId: number | null;
		mode: TerminalMode;
		active?: boolean;
		onStatusChange?: (tabId: string, status: ConnectionStatus) => void;
		onOpenPalette?: () => void;
	}

	let { tabId, serverId, mode = 'pty', active = true, onStatusChange, onOpenPalette }: Props = $props();

	let showReconnectOverlay = $state(false);
	let shellEl: HTMLDivElement;

	// ── Gesture state machine (PointerEvent-based, capture phase) ───────
	// Capture phase intercepts touch events BEFORE xterm.js sees them.
	// Mouse events pass through untouched for native selection/scrolling.
	// Touch events: tap → focus, double-tap → palette, vertical → scroll,
	// horizontal → tab swipe. Axis locks early and never switches.
	type GestureMode = 'idle' | 'pending' | 'vertical-scroll' | 'horizontal-swipe';

	let gestureMode: GestureMode = 'idle';
	let startX = 0;
	let startY = 0;
	let lastX = 0;
	let lastY = 0;
	let startTime = 0;
	let lastTapTime = 0;
	let tapFocusTimeout: ReturnType<typeof setTimeout> | null = null;

	const TAP_MAX_MS = 220;       // max pointer-down duration to count as a tap (not a drag)
	const DOUBLE_TAP_MS = 350;    // window between taps for double-tap detection
	const FOCUS_DELAY_MS = 380;   // delay focus slightly longer than double-tap window
	const LOCK_DISTANCE = 10;
	const AXIS_BIAS = 1.2;
	const SWIPE_COMMIT_PX = 60;
	const CELL_HEIGHT_FALLBACK = 18;

	function getCellHeight(): number {
		try {
			const dims = (terminal as any)?.['_core']?.['_renderService']?.['dimensions'];
			if (dims?.css?.cell?.height) return dims.css.cell.height;
		} catch { /* ignore */ }
		return CELL_HEIGHT_FALLBACK;
	}

	function handlePointerDown(e: PointerEvent) {
		if (e.pointerType !== 'touch') return;
		e.stopPropagation(); // prevent xterm.js from seeing this touch
		gestureMode = 'pending';
		startX = lastX = e.clientX;
		startY = lastY = e.clientY;
		startTime = performance.now();
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

			if (ady > adx * AXIS_BIAS) {
				gestureMode = 'vertical-scroll';
			} else if (adx > ady * AXIS_BIAS) {
				gestureMode = 'horizontal-swipe';
			} else {
				return;
			}
		}

		if (gestureMode === 'vertical-scroll' && terminal) {
			const deltaY = e.clientY - lastY;
			const cellH = getCellHeight();
			const lines = Math.round(-deltaY / cellH);
			if (lines !== 0) {
				terminal.scrollLines(lines);
			}
			e.preventDefault();
		}

		if (gestureMode === 'horizontal-swipe') {
			e.preventDefault();
		}

		lastX = e.clientX;
		lastY = e.clientY;
	}

	function handlePointerUp(e: PointerEvent) {
		if (e.pointerType !== 'touch') return;
		e.stopPropagation();

		const elapsed = performance.now() - startTime;
		const moved = Math.hypot(e.clientX - startX, e.clientY - startY);

		if (gestureMode === 'pending' && elapsed <= TAP_MAX_MS && moved < LOCK_DISTANCE) {
			const now = performance.now();
			if (now - lastTapTime <= DOUBLE_TAP_MS) {
				// Double-tap → cancel pending focus, open command palette
				if (tapFocusTimeout) { clearTimeout(tapFocusTimeout); tapFocusTimeout = null; }
				onOpenPalette?.();
				lastTapTime = 0;
			} else {
				// Single tap → delay focus so double-tap can cancel it
				lastTapTime = now;
				if (tapFocusTimeout) clearTimeout(tapFocusTimeout);
				tapFocusTimeout = setTimeout(() => {
					terminal?.focus();
					tapFocusTimeout = null;
				}, FOCUS_DELAY_MS);
			}
		}

		if (gestureMode === 'horizontal-swipe') {
			const totalDx = e.clientX - startX;
			if (Math.abs(totalDx) > SWIPE_COMMIT_PX) {
				if (totalDx < 0) terminalState.nextTab();
				else terminalState.prevTab();
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
	let commandHistory: string[] = [];
	let historyIndex = -1;
	let status = $state<ConnectionStatus>('disconnected');
	let ptyInputDisposable: { dispose: () => void } | null = null;
	let ptyResizeDisposable: { dispose: () => void } | null = null;
	let intentionalDisconnect = false;
	let reconnectAttempt = 0;
	let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;

	const HISTORY_KEY = 'svrctlrs-cmd-history';
	const MAX_HISTORY = 50;
	const PING_INTERVAL = 30_000;
	const MAX_OUTPUT_HISTORY = 10_000;
	const MAX_RECONNECT_ATTEMPTS = 5;
	const RECONNECT_DELAYS = [1000, 2000, 4000, 8000, 16000, 30000];

	function setStatus(s: ConnectionStatus) {
		status = s;
		onStatusChange?.(tabId, s);
	}

	function getWsUrl(wsMode: TerminalMode): string {
		const serverUrl = getServerUrl();
		const path = wsMode === 'pty' ? '/ws/terminal/pty' : '/ws/terminal';
		let url: string;
		if (serverUrl) {
			const wsBase = serverUrl.replace(/^http/, 'ws');
			url = `${wsBase}${path}`;
		} else {
			const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
			url = `${protocol}//${window.location.host}${path}`;
		}
		// Append auth token as query param (WebSocket API doesn't support custom headers)
		const token = localStorage.getItem('svrctlrs-session-token');
		const userId = localStorage.getItem('svrctlrs-user-id');
		if (token) {
			url += `?token=${encodeURIComponent(token)}`;
			if (userId) url += `&user_id=${encodeURIComponent(userId)}`;
		}
		return url;
	}

	function loadHistory() {
		try {
			const key = serverId ? `${HISTORY_KEY}-${serverId}` : HISTORY_KEY;
			const saved = localStorage.getItem(key);
			if (saved) {
				commandHistory = JSON.parse(saved);
				historyIndex = commandHistory.length;
			}
		} catch {
			// ignore
		}
	}

	function saveHistory() {
		try {
			const key = serverId ? `${HISTORY_KEY}-${serverId}` : HISTORY_KEY;
			localStorage.setItem(key, JSON.stringify(commandHistory));
		} catch {
			// ignore
		}
	}

	function addToHistory(cmd: string) {
		if (!cmd.trim()) return;
		if (commandHistory.length > 0 && commandHistory[commandHistory.length - 1] === cmd) return;
		commandHistory.push(cmd);
		if (commandHistory.length > MAX_HISTORY) commandHistory.shift();
		historyIndex = commandHistory.length;
		saveHistory();
	}

	export function getPreviousCommand(): string | null {
		if (historyIndex > 0) {
			historyIndex--;
			return commandHistory[historyIndex];
		}
		return null;
	}

	export function getNextCommand(): string {
		if (historyIndex < commandHistory.length - 1) {
			historyIndex++;
			return commandHistory[historyIndex];
		}
		historyIndex = commandHistory.length;
		return '';
	}

	function tryLoadWebgl() {
		if (!terminal) return;
		try {
			webglAddon = new WebglAddon();
			terminal.loadAddon(webglAddon);
			webglAddon.onContextLoss(() => {
				webglAddon?.dispose();
				webglAddon = null;
				setTimeout(() => tryLoadWebgl(), 1000);
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
					storageLimit: 128,
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

		resizeObserver = new ResizeObserver(() => {
			if (resizeTimer) clearTimeout(resizeTimer);
			resizeTimer = setTimeout(() => fit(), 100);
		});
		resizeObserver.observe(containerEl);
	}

	function appendOutput(data: string) {
		outputHistory.push(data);
		if (outputHistory.length > MAX_OUTPUT_HISTORY) {
			outputHistory = outputHistory.slice(-MAX_OUTPUT_HISTORY);
		}
	}

	function attemptReconnect() {
		if (reconnectAttempt >= MAX_RECONNECT_ATTEMPTS) {
			terminal?.writeln('\r\n\x1b[31m[Max reconnection attempts reached]\x1b[0m');
			showReconnectOverlay = true;
			return;
		}
		const delay = RECONNECT_DELAYS[Math.min(reconnectAttempt, RECONNECT_DELAYS.length - 1)];
		reconnectAttempt++;
		terminal?.writeln(`\r\n\x1b[33m[Reconnecting... attempt ${reconnectAttempt}/${MAX_RECONNECT_ATTEMPTS}]\x1b[0m`);
		reconnectTimeout = setTimeout(() => {
			reconnectTimeout = null;
			connect();
		}, delay);
	}

	/** Connect to the WebSocket for the current mode and serverId */
	export function connect() {
		if (!terminal || serverId === null) return;
		showReconnectOverlay = false;

		// Cancel any pending reconnect
		if (reconnectTimeout) {
			clearTimeout(reconnectTimeout);
			reconnectTimeout = null;
		}

		intentionalDisconnect = false;
		cleanupSocket();
		setStatus('connecting');

		if (reconnectAttempt === 0) {
			terminal.clear();
			outputHistory = [];
			restoreBuffer();
		}
		terminal.writeln('\x1b[33mConnecting...\x1b[0m');

		const wsUrl = getWsUrl(mode);

		try {
			socket = new WebSocket(wsUrl);
		} catch (e) {
			terminal.writeln(`\r\n\x1b[31mFailed to create WebSocket: ${e}\x1b[0m`);
			setStatus('error');
			return;
		}

		socket.onopen = () => {
			setStatus('connected');
			reconnectAttempt = 0;

			const { cols, rows } = terminal!;

			if (mode === 'pty') {
				// PTY: send shell request
				const req: PtyRequest = { type: 'shell', server_id: serverId!, cols, rows };
				socket!.send(JSON.stringify(req));

				// Forward keyboard input to PTY
				ptyInputDisposable = terminal!.onData((data: string) => {
					if (socket?.readyState === WebSocket.OPEN) {
						const req: PtyRequest = { type: 'input', data };
						socket.send(JSON.stringify(req));
					}
				});

				// Forward resize events
				ptyResizeDisposable = terminal!.onResize(({ cols, rows }) => {
					if (socket?.readyState === WebSocket.OPEN) {
						const req: PtyRequest = { type: 'resize', cols, rows };
						socket.send(JSON.stringify(req));
					}
				});

				terminal!.focus();
			}
			// CMD mode: nothing to send on open — commands sent via executeCommand()

			// Start keep-alive pings
			pingInterval = setInterval(() => {
				if (socket?.readyState === WebSocket.OPEN) {
					socket.send(JSON.stringify({ type: 'ping' }));
				}
			}, PING_INTERVAL);
		};

		socket.onmessage = (event: MessageEvent) => {
			try {
				if (mode === 'pty') {
					const res = JSON.parse(event.data) as PtyResponse;
					switch (res.type) {
						case 'output':
							terminal!.write(res.data);
							appendOutput(res.data);
							break;
						case 'connected':
							terminal!.write(res.data);
							terminal!.focus();
							break;
						case 'error':
							terminal!.write(res.data);
							break;
						case 'pong':
							break;
					}
				} else {
					const res = JSON.parse(event.data) as CmdResponse;
					switch (res.type) {
						case 'output':
						case 'exit':
						case 'error':
							terminal!.write(res.data);
							appendOutput(res.data);
							break;
						case 'pong':
							break;
					}
				}
			} catch {
				terminal!.writeln('\r\n\x1b[31mError parsing server response\x1b[0m');
			}
		};

		socket.onerror = () => {
			terminal!.writeln('\r\n\x1b[31m✗ Connection error\x1b[0m');
			setStatus('error');
		};

		socket.onclose = () => {
			const wasConnected = status === 'connected';
			setStatus('disconnected');
			cleanupPty();
			if (pingInterval) {
				clearInterval(pingInterval);
				pingInterval = null;
			}

			if (wasConnected && !intentionalDisconnect) {
				terminal!.writeln('\r\n\x1b[33m[Connection lost]\x1b[0m');
				attemptReconnect();
			} else if (!intentionalDisconnect && reconnectAttempt > 0) {
				// Reconnect attempt failed before fully connecting
				attemptReconnect();
			} else if (wasConnected) {
				terminal!.writeln('\r\n\x1b[33m[Disconnected]\x1b[0m');
			}
		};
	}

	/** Execute a single command in CMD mode */
	export function executeCommand(command: string) {
		if (!terminal || !socket || socket.readyState !== WebSocket.OPEN || mode !== 'cmd') return;

		addToHistory(command);
		const { cols, rows } = terminal;
		const req: CmdRequest = { type: 'execute', server_id: serverId!, command, cols, rows };
		socket.send(JSON.stringify(req));
	}

	function cleanupPty() {
		ptyInputDisposable?.dispose();
		ptyInputDisposable = null;
		ptyResizeDisposable?.dispose();
		ptyResizeDisposable = null;
	}

	function cleanupSocket() {
		if (pingInterval) {
			clearInterval(pingInterval);
			pingInterval = null;
		}
		cleanupPty();

		if (socket) {
			if (mode === 'pty' && socket.readyState === WebSocket.OPEN) {
				socket.send(JSON.stringify({ type: 'close' }));
			}
			socket.close();
			socket = null;
		}
	}

	/** Disconnect the WebSocket */
	export function disconnect() {
		serializeBuffer();
		intentionalDisconnect = true;
		reconnectAttempt = 0;
		if (reconnectTimeout) {
			clearTimeout(reconnectTimeout);
			reconnectTimeout = null;
		}
		cleanupSocket();
		setStatus('disconnected');
	}

	/** Serialize terminal buffer to sessionStorage for later restoration */
	function serializeBuffer() {
		if (!serializeAddon || !terminal) return;
		try {
			const data = serializeAddon.serialize();
			if (data.length > 512 * 1024) return; // Skip if > 512KB
			sessionStorage.setItem(`svrctlrs-term-buffer-${tabId}`, data);
		} catch {
			// sessionStorage full or unavailable
		}
	}

	/** Restore terminal buffer from sessionStorage */
	function restoreBuffer() {
		const key = `svrctlrs-term-buffer-${tabId}`;
		const data = sessionStorage.getItem(key);
		if (data && terminal) {
			terminal.write(data);
			sessionStorage.removeItem(key);
		}
	}

	/** Remove saved buffer (called when tab is explicitly closed) */
	export function clearSavedBuffer() {
		sessionStorage.removeItem(`svrctlrs-term-buffer-${tabId}`);
	}

	/** Inject text into the terminal as if the user typed it (PTY mode) */
	export function injectInput(data: string) {
		if (socket?.readyState === WebSocket.OPEN && mode === 'pty') {
			const req: PtyRequest = { type: 'input', data };
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
		terminal?.focus();
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
		requestAnimationFrame(() => fit());
	});

	onMount(() => {
		loadHistory();
		initTerminal();

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
		const suppressTouch = (e: Event) => { e.stopPropagation(); e.preventDefault(); };
		shellEl.addEventListener('touchstart', suppressTouch, { capture: true, passive: false });
		shellEl.addEventListener('touchmove', suppressTouch, { capture: true, passive: false });
		shellEl.addEventListener('touchend', suppressTouch, { capture: true, passive: false });

		// Serialize buffer when tab is hidden or page is unloading
		const handleVisibilityChange = () => {
			if (document.hidden) serializeBuffer();
		};
		const handleBeforeUnload = () => serializeBuffer();
		document.addEventListener('visibilitychange', handleVisibilityChange);
		window.addEventListener('beforeunload', handleBeforeUnload);

		return () => {
			if (tapFocusTimeout) clearTimeout(tapFocusTimeout);
			shellEl.removeEventListener('pointerdown', handlePointerDown, { capture: true });
			shellEl.removeEventListener('pointermove', handlePointerMove, { capture: true });
			shellEl.removeEventListener('pointerup', handlePointerUp, { capture: true });
			shellEl.removeEventListener('pointercancel', handlePointerUp, { capture: true });
			shellEl.removeEventListener('touchstart', suppressTouch, { capture: true });
			shellEl.removeEventListener('touchmove', suppressTouch, { capture: true });
			shellEl.removeEventListener('touchend', suppressTouch, { capture: true });
			document.removeEventListener('visibilitychange', handleVisibilityChange);
			window.removeEventListener('beforeunload', handleBeforeUnload);
			disconnect();
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

	{#if showReconnectOverlay}
		<div class="absolute inset-0 bg-background/80 flex flex-col items-center justify-center z-10 gap-3">
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
					onclick={() => { clear(); connect(); }}
				>
					New Session
				</button>
			</div>
		</div>
	{/if}
</div>
