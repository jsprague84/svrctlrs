<script lang="ts">
	/**
	 * Plays one recording in a read-only xterm at the size it was recorded, scaled down to fit.
	 * Space plays and pauses, ←/→ jump five seconds, Home/End go to either end. Seeking back
	 * redraws from the start (the terminal cannot run backwards); pauses longer than two seconds
	 * are shortened unless "Skip pauses" is off.
	 */
	import { onMount } from 'svelte';
	import { Terminal } from '@xterm/xterm';
	import { UnicodeGraphemesAddon } from '@xterm/addon-unicode-graphemes';
	import '@xterm/xterm/css/xterm.css';
	import { Download, Pause, Play, X } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Select from '#lib/components/ui/Select.svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import Toggle from '#lib/components/ui/Toggle.svelte';
	import { tokyoNightTheme, lightTheme } from '#lib/components/terminal/terminal-theme.js';
	import * as themeState from '#lib/state/theme.svelte.js';
	import * as terminalPrefs from '#lib/state/terminalPrefs.svelte.js';
	import { fetchCast } from '#lib/api/recordings.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { overlay } from '#lib/utils/overlay.js';
	import {
		formatBytes,
		formatClock,
		indexAt,
		limitIdle,
		markers,
		parseCast,
		steps,
		type Cast,
		type CastEvent,
		type Step
	} from '#lib/recordings/cast.js';
	import type { Recording } from '#lib/types/index.js';

	interface Props {
		recording: Recording;
		onClose: () => void;
		onDownload: (recording: Recording) => void;
	}

	let { recording, onClose, onDownload }: Props = $props();

	/** Pauses longer than this are shortened while "Skip pauses" is on */
	const MAX_IDLE = 2;

	let loadState = $state<'loading' | 'ready' | 'error'>('loading');
	let loadError = $state('');
	let duration = $state(0);
	let position = $state(0);
	let playing = $state(false);
	let speed = $state('1');
	let skipIdle = $state(true);
	let marks = $state<{ time: number; label: string }[]>([]);
	/** What was actually fetched: the real length (the list's figures lag a running recording) */
	let loaded = $state<{ duration: number; bytes: number } | null>(null);
	/** The unscaled terminal and the factor that fits it on the stage */
	let screenW = $state(0);
	let screenH = $state(0);
	let scale = $state(1);

	let cast: Cast | null = null;
	/** The events on the timeline being played (pauses shortened or not) */
	let events: CastEvent[] = [];
	/** How many events are on screen */
	let shown = 0;
	let term: Terminal | null = null;
	let frameEl: HTMLDivElement;
	let stageEl: HTMLDivElement;
	let raf = 0;
	let lastTick = 0;

	const started = $derived(new Date(recording.started_at));

	async function load() {
		loadState = 'loading';
		try {
			const text = await fetchCast(recording.id);
			cast = parseCast(text);
			loaded = { duration: cast.duration, bytes: new TextEncoder().encode(text).length };
			setTimeline();
			reset();
			loadState = 'ready';
			requestAnimationFrame(() => {
				measure();
				play();
			});
		} catch (e) {
			loadError = extractErrorMessage(e, 'Could not load the recording');
			loadState = 'error';
		}
	}

	function setTimeline() {
		if (!cast) return;
		events = limitIdle(cast.events, skipIdle ? MAX_IDLE : null);
		duration = events.length ? events[events.length - 1].time : 0;
		marks = markers(events);
	}

	/** Blank terminal at the recorded starting size */
	function reset() {
		if (!term || !cast) return;
		term.reset();
		term.resize(cast.header.width, cast.header.height);
		shown = 0;
	}

	function apply(list: Step[]) {
		if (!term) return;
		let resized = false;
		for (const s of list) {
			if ('write' in s) term.write(s.write);
			else {
				term.resize(s.resize.cols, s.resize.rows);
				resized = true;
			}
		}
		if (resized) requestAnimationFrame(measure);
	}

	/** Show the moment `time` (seconds on the current timeline) */
	function seek(time: number) {
		const t = Math.min(Math.max(0, time), duration);
		const to = indexAt(events, t);
		if (to < shown) reset();
		apply(steps(events, shown, to));
		shown = to;
		position = t;
	}

	function tick(now: number) {
		if (!playing) return;
		const dt = lastTick ? (now - lastTick) / 1000 : 0;
		lastTick = now;
		const target = Math.min(duration, position + dt * Number(speed));
		const to = indexAt(events, target);
		if (to > shown) {
			apply(steps(events, shown, to));
			shown = to;
		}
		position = target;
		if (target >= duration) {
			playing = false;
			return;
		}
		raf = requestAnimationFrame(tick);
	}

	function play() {
		if (loadState !== 'ready' || duration === 0) return;
		if (position >= duration) seek(0);
		playing = true;
		lastTick = 0;
		cancelAnimationFrame(raf);
		raf = requestAnimationFrame(tick);
	}

	function pause() {
		playing = false;
		cancelAnimationFrame(raf);
	}

	function toggle() {
		if (playing) pause();
		else play();
	}

	/** Switching "Skip pauses" keeps what is on screen and moves the clock to match */
	function changeIdle(on: boolean) {
		skipIdle = on;
		setTimeline();
		position = shown > 0 ? events[shown - 1].time : 0;
	}

	/** Scale the terminal down (never up) to fit the stage */
	function measure() {
		const screen = frameEl?.querySelector<HTMLElement>('.xterm-screen');
		if (!screen || !stageEl) return;
		screenW = screen.offsetWidth;
		screenH = screen.offsetHeight;
		const availW = stageEl.clientWidth - 16;
		const availH = stageEl.clientHeight - 16;
		if (screenW > 0 && screenH > 0) {
			scale = Math.min(1, availW / screenW, availH / screenH);
		}
	}

	function onKey(e: KeyboardEvent) {
		const target = e.target as HTMLElement | null;
		const tag = target?.tagName;
		// Controls handle their own keys (a button's Space, the slider's arrows)
		if (tag === 'BUTTON' || tag === 'INPUT' || tag === 'SELECT') return;
		let handled = true;
		if (e.key === ' ' || e.key === 'k') toggle();
		else if (e.key === 'ArrowLeft') seek(position - 5);
		else if (e.key === 'ArrowRight') seek(position + 5);
		else if (e.key === 'Home') seek(0);
		else if (e.key === 'End') seek(duration);
		else handled = false;
		if (handled) {
			e.preventDefault();
			e.stopPropagation();
		}
	}

	$effect(() => {
		const theme = themeState.isDark() ? tokyoNightTheme : lightTheme;
		if (term) term.options.theme = theme;
	});

	onMount(() => {
		const p = terminalPrefs.getPrefs();
		term = new Terminal({
			cols: recording.cols,
			rows: recording.rows,
			fontSize: 14,
			fontFamily: p.fontFamily,
			fontWeight: p.fontWeight,
			fontWeightBold: p.fontWeightBold,
			lineHeight: p.lineHeight,
			letterSpacing: p.letterSpacing,
			cursorBlink: false,
			disableStdin: true,
			scrollback: 1000,
			drawBoldTextInBrightColors: p.drawBoldTextInBrightColors,
			minimumContrastRatio: p.minimumContrastRatio,
			allowProposedApi: true,
			theme: themeState.isDark() ? tokyoNightTheme : lightTheme
		});
		term.loadAddon(new UnicodeGraphemesAddon());
		term.unicode.activeVersion = '15-graphemes';
		term.open(frameEl);
		const observer = new ResizeObserver(() => measure());
		observer.observe(stageEl);
		load();
		return () => {
			cancelAnimationFrame(raf);
			observer.disconnect();
			term?.dispose();
			term = null;
		};
	});
</script>

<div
	class="fixed inset-0 z-50 bg-background flex flex-col"
	role="dialog"
	aria-modal="true"
	aria-label="Recording of {recording.server_name}"
	onkeydowncapture={onKey}
	{@attach overlay({ onEscape: () => onClose() })}
>
	<header
		class="flex items-center gap-2 pl-4 pr-1.5 md:pr-3 py-1.5 border-b border-border pt-[max(0.375rem,env(safe-area-inset-top))]"
	>
		<div class="min-w-0 flex-1">
			<div class="text-sm font-medium text-text-primary truncate">{recording.server_name}</div>
			<div class="text-xs text-text-muted truncate">
				{started.toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })}{loaded
					? ` · ${formatClock(loaded.duration)} · ${formatBytes(loaded.bytes)}`
					: ''}{recording.status === 'recording' ? ' · still recording, shown so far' : ''}
			</div>
		</div>
		<Button
			variant="ghost"
			size="sm"
			class="min-w-11"
			onclick={() => onDownload(recording)}
			aria-label="Download"
		>
			<Download class="w-4 h-4" /> <span class="hidden sm:inline">Download</span>
		</Button>
		<button
			type="button"
			class="w-11 h-11 flex items-center justify-center rounded-sm text-text-secondary hover:text-text-primary hover:bg-surface-raised"
			onclick={onClose}
			aria-label="Close player"
			title="Close (Esc)"
		>
			<X class="w-5 h-5" />
		</button>
	</header>

	<div
		class="relative flex-1 min-h-0 flex items-center justify-center overflow-hidden"
		bind:this={stageEl}
	>
		{#if loadState === 'loading'}
			<div class="absolute inset-0 flex items-center justify-center">
				<StateBlock kind="loading" title="Loading recording…" />
			</div>
		{:else if loadState === 'error'}
			<div class="absolute inset-0 flex items-center justify-center">
				<StateBlock kind="error" title={loadError} onRetry={load} />
			</div>
		{/if}
		<div
			class="overflow-hidden rounded-sm border border-border/60 {loadState === 'ready'
				? ''
				: 'invisible'}"
			style="width: {Math.ceil(screenW * scale)}px; height: {Math.ceil(screenH * scale)}px"
		>
			<div
				class="recording-frame"
				bind:this={frameEl}
				style="transform: scale({scale}); transform-origin: 0 0; width: max-content"
			></div>
		</div>
	</div>

	<div
		class="border-t border-border px-3 md:px-4 pt-1 pb-[max(0.5rem,env(safe-area-inset-bottom))] flex flex-col"
	>
		<div class="relative">
			<input
				type="range"
				min="0"
				max={duration || 0}
				step="0.1"
				value={position}
				oninput={(e) => seek(Number(e.currentTarget.value))}
				disabled={loadState !== 'ready' || duration === 0}
				aria-label="Position"
				aria-valuetext="{formatClock(position)} of {formatClock(duration)}"
				class="w-full h-11 md:h-6 accent-accent cursor-pointer"
			/>
			{#each marks as m (m.time + m.label)}
				<span
					class="absolute bottom-1 w-0.5 h-2 bg-warning pointer-events-none"
					style="left: {duration ? (m.time / duration) * 100 : 0}%"
					title={m.label}
				></span>
			{/each}
		</div>
		<div class="flex items-center gap-2 md:gap-3">
			<Button
				size="sm"
				class="min-w-11"
				onclick={toggle}
				disabled={loadState !== 'ready' || duration === 0}
				aria-label={playing ? 'Pause' : 'Play'}
				title={playing ? 'Pause (Space)' : 'Play (Space)'}
			>
				{#if playing}<Pause class="w-4 h-4" />{:else}<Play class="w-4 h-4" />{/if}
			</Button>
			<span class="text-xs font-mono text-text-secondary tabular-nums whitespace-nowrap"
				>{formatClock(position)} / {formatClock(duration)}</span
			>
			<div class="flex-1"></div>
			<div class="text-xs">
				<Toggle label="Skip pauses" checked={skipIdle} onchange={changeIdle} />
			</div>
			<Select bind:value={speed} aria-label="Speed" class="w-20">
				<option value="0.5">0.5×</option>
				<option value="1">1×</option>
				<option value="2">2×</option>
				<option value="4">4×</option>
				<option value="8">8×</option>
			</Select>
		</div>
		{#if recording.status !== 'complete' && recording.end_reason}
			<p class="text-xs text-text-muted pb-1">{recording.end_reason}</p>
		{/if}
	</div>
</div>

<style>
	/* The player's terminal is read-only: no blinking caret over the recording */
	.recording-frame :global(.xterm-helper-textarea) {
		pointer-events: none;
	}
</style>
