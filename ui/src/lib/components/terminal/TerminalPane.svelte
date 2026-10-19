<script lang="ts">
	import { onMount } from 'svelte';
	import { Terminal, type IDecoration, type IMarker } from '@xterm/xterm';
	import { FitAddon } from '@xterm/addon-fit';
	import { SearchAddon } from '@xterm/addon-search';
	import { WebLinksAddon } from '@xterm/addon-web-links';
	import { WebglAddon } from '@xterm/addon-webgl';
	import { UnicodeGraphemesAddon } from '@xterm/addon-unicode-graphemes';
	import { ClipboardAddon } from '@xterm/addon-clipboard';
	import { ImageAddon } from '@xterm/addon-image';
	import { SerializeAddon } from '@xterm/addon-serialize';
	import '@xterm/xterm/css/xterm.css';

	import { tokyoNightTheme, lightTheme } from './terminal-theme.js';
	import * as themeState from '#lib/state/theme.svelte.js';
	import * as terminalPrefs from '#lib/state/terminalPrefs.svelte.js';
	import * as terminalState from '#lib/state/terminal.svelte.js';
	import { isMobile } from '#lib/state/mobile.svelte.js';
	import * as serversApi from '#lib/api/servers.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { PtySession, type SessionView } from '#lib/terminal/ptySession.js';
	import { browserPtySocketUrl } from '#lib/terminal/socketUrl.js';
	import type { EchoCell, EchoScreen } from '#lib/terminal/localEcho.js';
	import { overlay } from '#lib/utils/overlay.js';
	import type { ConnectionStatus } from '#lib/types/index.js';

	interface Props {
		tabId: string;
		serverId: number | null;
		active?: boolean;
		onStatusChange?: (tabId: string, status: ConnectionStatus) => void;
		/** The shell set its window title (OSC 0/2) */
		onTitleChange?: (tabId: string, title: string) => void;
		/** The shell rang the bell (BEL) */
		onBell?: (tabId: string) => void;
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
		onTitleChange,
		onBell,
		onModifierConsumed,
		onSwipe,
		onOpenPalette
	}: Props = $props();

	/** Status, banner, reconnect overlay and host-key challenge, as the session reports them */
	let view = $state<SessionView>({
		status: 'disconnected',
		note: null,
		showReconnect: false,
		hostKey: null,
		recording: false
	});
	/** The WebSocket side: connect, attach, replay, keep-alive, reconnect, persistence */
	let session: PtySession | null = null;
	/** Brief ring around the pane when the shell rings the bell */
	let bellFlash = $state(false);
	let bellFlashTimer: ReturnType<typeof setTimeout> | null = null;
	let trustingHostKey = $state(false);
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
		if (!terminal || !session?.isOpen) return;
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

	// ── Predictive echo ──────────────────────────────────────────────────
	// The session decides what to predict (`localEcho.ts`); the pane reads the screen for it
	// and draws the predictions as decorations over the cells, never into the terminal.

	/** One decoration (and its marker) per line of predicted characters */
	let echoMarks: (IDecoration | IMarker)[] = [];

	function echoScreenFor(term: Terminal): EchoScreen {
		return {
			get cols() {
				return term.cols;
			},
			cursor() {
				const buf = term.buffer.active;
				if (buf.type !== 'normal') return null;
				return { line: buf.baseY + buf.cursorY, col: buf.cursorX };
			},
			cell(line, col) {
				const cell = term.buffer.active.getLine(line)?.getCell(col);
				if (!cell) return null;
				return cell.getChars() || ' ';
			},
			textBeforeCursor() {
				const buf = term.buffer.active;
				return buf.getLine(buf.baseY + buf.cursorY)?.translateToString(false, 0, buf.cursorX) ?? '';
			},
			show: drawPredictions
		};
	}

	function drawPredictions(cells: EchoCell[]) {
		for (const mark of echoMarks) mark.dispose();
		echoMarks = [];
		if (!terminal || cells.length === 0) return;
		const term = terminal;
		const buf = term.buffer.active;
		const cursorLine = buf.baseY + buf.cursorY;
		const runs: { line: number; col: number; text: string }[] = [];
		for (const c of cells) {
			const run = runs[runs.length - 1];
			if (run && run.line === c.line && run.col + run.text.length === c.col) run.text += c.char;
			else runs.push({ line: c.line, col: c.col, text: c.char });
		}
		runs.forEach((run, i) => {
			const marker = term.registerMarker(run.line - cursorLine);
			if (!marker) return;
			const decoration = term.registerDecoration({
				marker,
				x: run.col,
				width: run.text.length,
				layer: 'top'
			});
			if (!decoration) {
				marker.dispose();
				return;
			}
			const caret = i === runs.length - 1;
			decoration.onRender((el) => paintPrediction(el, run.text, caret));
			echoMarks.push(decoration, marker);
		});
	}

	/** Fill a decoration with the predicted text: one span per cell, in the terminal's font,
	 *  over an opaque cell background so whatever is underneath does not show through. */
	function paintPrediction(el: HTMLElement, text: string, caret: boolean) {
		if (el.dataset.echo === text || !terminal) return;
		el.dataset.echo = text;
		const { theme, fontFamily, fontSize, fontWeight } = terminal.options;
		el.classList.add('local-echo');
		el.classList.toggle('local-echo-caret', caret);
		el.style.setProperty('--echo-bg', theme?.background ?? '#000000');
		el.style.setProperty('--echo-fg', theme?.foreground ?? '#ffffff');
		el.style.setProperty('--echo-caret', theme?.cursor ?? theme?.foreground ?? '#ffffff');
		el.style.fontFamily = fontFamily ?? 'monospace';
		el.style.fontSize = `${fontSize ?? 14}px`;
		el.style.fontWeight = String(fontWeight ?? 400);
		el.replaceChildren(
			...Array.from(text, (ch) => {
				const span = document.createElement('span');
				span.textContent = ch;
				return span;
			})
		);
	}

	let containerEl: HTMLDivElement;
	let terminal: Terminal | null = null;
	let fitAddon: FitAddon | null = null;
	let searchAddon: SearchAddon | null = null;
	let serializeAddon: SerializeAddon | null = null;
	let webglAddon: WebglAddon | null = null;
	let resizeObserver: ResizeObserver | null = null;
	let resizeTimer: ReturnType<typeof setTimeout> | null = null;

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
		// Grapheme clusters (emoji sequences, combining marks) take their real cell width. The
		// addon only registers the version; it has to be selected.
		terminal.loadAddon(new UnicodeGraphemesAddon());
		terminal.unicode.activeVersion = '15-graphemes';
		terminal.loadAddon(new ClipboardAddon());

		terminal.onTitleChange((title) => onTitleChange?.(tabId, title));
		terminal.onBell(() => {
			onBell?.(tabId);
			bellFlash = true;
			if (bellFlashTimer) clearTimeout(bellFlashTimer);
			bellFlashTimer = setTimeout(() => (bellFlash = false), 180);
			if (isMobile() && typeof navigator !== 'undefined' && 'vibrate' in navigator) {
				try {
					navigator.vibrate(40);
				} catch {
					// not allowed without a user gesture; the flash is enough
				}
			}
		});

		const term = terminal;
		session = new PtySession({
			tabId,
			serverId: () => serverId,
			terminal: {
				write: (data, done) => term.write(data, done),
				writeln: (line) => term.writeln(line),
				clear: () => term.clear(),
				reset: () => term.reset(),
				focus: () => term.focus(),
				fit: () => fit(),
				get cols() {
					return term.cols;
				},
				get rows() {
					return term.rows;
				},
				serialize: () => serializeAddon?.serialize() ?? null
			},
			socketUrl: browserPtySocketUrl,
			trustHostKey: serversApi.trustHostKey,
			echo: {
				screen: echoScreenFor(term),
				mode: () => terminalPrefs.getPrefs().predictiveEcho
			},
			onStatus: (s) => onStatusChange?.(tabId, s),
			onView: (v) => (view = v)
		});
		// Keyboard input and size changes go to the PTY while the socket is open. The pending
		// CTL/ALT is only used up by a keystroke that is actually sent.
		terminal.onData((data) => {
			if (session?.isOpen) session.sendInput(applyPendingModifier(data));
		});
		terminal.onResize(({ cols, rows }) => session?.sendResize(cols, rows));

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

	/** The user compared the fingerprint and accepts it: trust the key, then connect again. */
	async function trustHostKey() {
		if (!session || !view.hostKey || serverId === null) return;
		trustingHostKey = true;
		try {
			await session.trustHostKey();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Could not trust the host key'));
		} finally {
			trustingHostKey = false;
		}
	}

	/** Open the PTY WebSocket for the current serverId (new shell, or reattach) */
	export async function connect() {
		await session?.connect();
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

	/** End the shell and the connection (Disconnect button, tab close). */
	export function disconnect() {
		session?.disconnect();
	}

	/** Leave the shell running on the server and drop only this connection (pane unmount). */
	export function detach() {
		session?.detach();
	}

	/** Remove saved buffer and session record (called when tab is explicitly closed) */
	export function clearSavedBuffer() {
		session?.clearSaved();
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
		// Through onData while the socket is open; nothing to type into otherwise
		if (session?.isOpen) terminal.input(seq, true);
	}

	export function clear() {
		terminal?.clear();
		session?.clearOutput();
	}

	export function fit() {
		// A hidden pane (another tab in front) has no layout box. The fit addon would read its
		// computed `100%` width and height as 100 px and shrink the remote shell to 10×3, which
		// garbles both that tab and the prompt redraws of the one in front. Keep the last size;
		// the pane refits when it is shown again.
		if (!containerEl?.clientWidth || !containerEl.clientHeight) return;
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
		return session?.getOutput() ?? '';
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
		session?.loadSession();
		// Coming back to the terminal page (or reloading it): pick the shell up where it was
		if (session?.heldSessionId && serverId !== null) {
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

		// Save the screen when the tab is hidden; on return, make sure the socket is really alive
		const handleVisibilityChange = () => session?.visibilityChanged(document.hidden);
		const handleBeforeUnload = () => session?.persist();
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
			session?.dispose();
			drawPredictions([]);
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
	class="terminal-shell h-full w-full bg-background relative {bellFlash
		? 'ring-2 ring-inset ring-warning'
		: ''}"
	class:hidden={!active}
	bind:this={shellEl}
>
	<div class="terminal-viewport h-full w-full" bind:this={containerEl}></div>

	{#if view.note}
		<div
			class="absolute top-2 right-2 z-10 px-2 py-1 rounded-sm bg-surface/90 border border-border text-[11px] text-text-secondary pointer-events-none"
			role="status"
			aria-live="polite"
		>
			<span
				class="inline-block w-1.5 h-1.5 rounded-full bg-warning mr-1.5 align-middle animate-pulse"
			></span>{view.note}
		</div>
	{/if}

	{#if view.recording && view.status === 'connected' && !view.note}
		<div
			class="absolute top-1.5 right-2 z-10 px-1.5 py-0.5 rounded-sm bg-surface/80 border border-error/40 text-[10px] font-semibold tracking-wide text-error pointer-events-none select-none"
			role="status"
			aria-label="This session is being recorded"
			title="This session is being recorded"
		>
			<span class="inline-block w-1.5 h-1.5 rounded-full bg-error mr-1 align-middle"></span>REC
		</div>
	{/if}

	{#if view.hostKey}
		<div
			class="absolute inset-0 bg-background/85 flex items-center justify-center z-20 p-4"
			role="dialog"
			aria-modal="true"
			aria-labelledby="hostkey-title-{tabId}"
			{@attach overlay({ onEscape: () => session?.dismissHostKey() })}
		>
			<div
				class="w-full max-w-md bg-surface border border-border rounded-lg p-4 flex flex-col gap-3"
			>
				<h3 id="hostkey-title-{tabId}" class="text-sm font-semibold text-text-primary">
					Unknown host key
				</h3>
				<p class="text-xs text-text-secondary">
					First connection to <strong>{view.hostKey.server_name}</strong>. Compare the fingerprint
					with the server's own, for example
					<code class="font-mono">ssh-keygen -lf /etc/ssh/ssh_host_*_key.pub</code> on the host, before
					trusting it.
				</p>
				<div class="rounded-sm bg-background border border-border/60 px-3 py-2">
					<div class="text-[11px] text-text-muted">{view.hostKey.key_type}</div>
					<div class="font-mono text-xs text-text-primary break-all select-all">
						{view.hostKey.fingerprint}
					</div>
				</div>
				<div class="flex gap-2 justify-end">
					<button
						class="px-3 py-1.5 text-sm bg-secondary text-secondary-foreground rounded-sm hover:opacity-90"
						onclick={() => session?.dismissHostKey()}
						disabled={trustingHostKey}
					>
						Cancel
					</button>
					<button
						class="px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-sm hover:opacity-90 disabled:opacity-60"
						onclick={trustHostKey}
						disabled={trustingHostKey}
					>
						{trustingHostKey ? 'Trusting…' : 'Trust and connect'}
					</button>
				</div>
			</div>
		</div>
	{/if}

	{#if view.showReconnect}
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
					onclick={() => session?.startFresh()}
				>
					New Session
				</button>
			</div>
		</div>
	{/if}
</div>

<style>
	/* Predicted characters (predictive echo): dimmer than real text, dotted underline, a caret
	   after the last one because the terminal's own cursor is underneath the first. */
	:global(.local-echo) {
		display: flex;
		align-items: center;
		pointer-events: none;
		background: var(--echo-bg);
		color: color-mix(in srgb, var(--echo-fg) 70%, var(--echo-bg));
	}
	:global(.local-echo > span) {
		flex: 1 1 0;
		text-align: center;
		white-space: pre;
		text-decoration: underline dotted;
		text-underline-offset: 3px;
	}
	:global(.local-echo-caret::after) {
		content: '';
		position: absolute;
		left: 100%;
		top: 10%;
		bottom: 10%;
		width: 2px;
		background: var(--echo-caret);
	}
</style>
