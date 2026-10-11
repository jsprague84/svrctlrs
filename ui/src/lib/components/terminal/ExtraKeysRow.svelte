<script lang="ts">
	/**
	 * Keys the soft keyboard lacks. Two rows, Termux style: the first has the modifiers and
	 * arrows, the second the control characters, navigation keys and shell punctuation that
	 * Gboard hides behind its symbol layer. Keys fire on pointerdown (no 300 ms wait), repeat
	 * while held, never take focus from the terminal, and give a short haptic tick.
	 *
	 * CTL and ALT are sticky for one key — a key here or the next character typed on the soft
	 * keyboard. Tapping a modifier twice locks it until tapped again.
	 */
	import { Command, ChevronUp, ChevronDown } from '@lucide/svelte';

	interface Props {
		onKeyPress: (sequence: string) => void;
		/** CTL/ALT also apply to the next character typed on the soft keyboard */
		onModifier?: (mod: 'ctl' | 'alt' | null) => void;
		/** The ⌘ key: open the command palette */
		onPalette?: () => void;
	}

	let { onKeyPress, onModifier, onPalette }: Props = $props();

	type Mod = 'ctl' | 'alt';
	let active = $state<Mod | null>(null);
	let locked = $state(false);
	let showSecondRow = $state(readRowPref());

	function readRowPref(): boolean {
		try {
			return localStorage.getItem('svrctlrs-extra-keys-row2') !== 'hidden';
		} catch {
			return true;
		}
	}

	function toggleSecondRow() {
		showSecondRow = !showSecondRow;
		try {
			localStorage.setItem('svrctlrs-extra-keys-row2', showSecondRow ? 'shown' : 'hidden');
		} catch {
			// ignore
		}
	}

	/** Called by the pane once a typed character consumed the modifier */
	export function clearModifiers() {
		if (locked) {
			onModifier?.(active);
			return;
		}
		active = null;
		onModifier?.(null);
	}

	function tapModifier(mod: Mod) {
		if (active === mod) {
			if (locked) {
				locked = false;
				active = null;
			} else {
				locked = true;
			}
		} else {
			active = mod;
			locked = false;
		}
		onModifier?.(active);
	}

	function consumeModifier() {
		if (locked) return;
		active = null;
		onModifier?.(null);
	}

	interface Key {
		label: string;
		/** Plain sequence */
		seq: string;
		/** Sequences with CTL / ALT held; `char` derives Ctrl+<char> */
		ctl?: string;
		alt?: string;
		char?: string;
		repeat?: boolean;
		title?: string;
	}

	// CSI with modifier parameter: 1;5 = Ctrl, 1;3 = Alt (xterm style)
	const csi = (final: string) => ({
		seq: `\x1b[${final}`,
		ctl: `\x1b[1;5${final}`,
		alt: `\x1b[1;3${final}`
	});
	const tilde = (n: number) => ({
		seq: `\x1b[${n}~`,
		ctl: `\x1b[${n};5~`,
		alt: `\x1b[${n};3~`
	});

	const row1: Key[] = [
		{ label: 'ESC', seq: '\x1b', alt: '\x1b\x1b' },
		{ label: 'TAB', seq: '\t', alt: '\x1b\t', title: 'Tab (CTL: Shift+Tab)', ctl: '\x1b[Z' },
		{ label: '←', ...csi('D'), repeat: true },
		{ label: '↓', ...csi('B'), repeat: true },
		{ label: '↑', ...csi('A'), repeat: true },
		{ label: '→', ...csi('C'), repeat: true }
	];
	const row2: Key[] = [
		{ label: '^C', seq: '\x03', title: 'Interrupt' },
		{ label: '^D', seq: '\x04', title: 'End of input / logout' },
		{ label: '^Z', seq: '\x1a', title: 'Suspend' },
		{ label: '^R', seq: '\x12', title: 'Search history' },
		{ label: 'HOME', ...csi('H') },
		{ label: 'END', ...csi('F') },
		{ label: 'PGUP', ...tilde(5), repeat: true },
		{ label: 'PGDN', ...tilde(6), repeat: true },
		{ label: '|', seq: '|', char: '|' },
		{ label: '~', seq: '~', char: '~' },
		{ label: '/', seq: '/', char: '/' },
		{ label: '-', seq: '-', char: '-' }
	];

	function sequenceFor(key: Key): string {
		if (active === 'ctl') {
			if (key.ctl) return key.ctl;
			if (key.char) {
				const code = key.char.toUpperCase().charCodeAt(0) - 64;
				if (code >= 0 && code <= 31) return String.fromCharCode(code);
			}
			return key.seq;
		}
		if (active === 'alt') return key.alt ?? '\x1b' + key.seq;
		return key.seq;
	}

	function fire(key: Key) {
		onKeyPress(sequenceFor(key));
		if (active) consumeModifier();
	}

	function tick() {
		try {
			navigator.vibrate?.(8);
		} catch {
			// not allowed; fine
		}
	}

	// Auto-repeat for held arrows and paging keys
	let repeatDelay: ReturnType<typeof setTimeout> | null = null;
	let repeatTimer: ReturnType<typeof setInterval> | null = null;

	function stopRepeat() {
		if (repeatDelay) clearTimeout(repeatDelay);
		if (repeatTimer) clearInterval(repeatTimer);
		repeatDelay = null;
		repeatTimer = null;
	}

	function pressKey(e: PointerEvent, key: Key) {
		e.preventDefault(); // keep the terminal focused and the soft keyboard open
		tick();
		fire(key);
		if (key.repeat) {
			stopRepeat();
			repeatDelay = setTimeout(() => {
				repeatTimer = setInterval(() => fire(key), 60);
			}, 400);
		}
	}

	function pressAction(e: PointerEvent, action: () => void) {
		e.preventDefault();
		tick();
		action();
	}

	function keyboardActivate(e: KeyboardEvent, action: () => void) {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			action();
		}
	}

	const keyClass =
		'flex-1 min-w-0 h-9 text-[12px] font-mono rounded-sm border select-none touch-manipulation ' +
		'bg-surface border-border text-text-secondary active:bg-surface-overlay active:text-text-primary';
	const modClass = (mod: Mod) =>
		active === mod
			? locked
				? 'bg-accent text-accent-foreground border-accent'
				: 'bg-accent/20 text-accent border-accent/40'
			: 'bg-surface border-border text-text-secondary';
</script>

<div
	class="flex flex-col gap-[3px] px-1 py-1 bg-surface-raised border-t border-border flex-shrink-0 select-none"
	role="toolbar"
	aria-label="Extra keys"
	tabindex="-1"
	oncontextmenu={(e) => e.preventDefault()}
	onpointerup={stopRepeat}
	onpointercancel={stopRepeat}
	onpointerleave={stopRepeat}
>
	<div class="flex gap-[3px]">
		<button
			type="button"
			class="{keyClass} {modClass('ctl')}"
			onpointerdown={(e) => pressAction(e, () => tapModifier('ctl'))}
			onkeydown={(e) => keyboardActivate(e, () => tapModifier('ctl'))}
			aria-pressed={active === 'ctl'}
			aria-label={locked && active === 'ctl' ? 'Control (locked)' : 'Control'}
			title="Control — applies to the next key; tap twice to lock"
		>
			CTL{active === 'ctl' && locked ? ' 🔒' : ''}
		</button>
		<button
			type="button"
			class="{keyClass} {modClass('alt')}"
			onpointerdown={(e) => pressAction(e, () => tapModifier('alt'))}
			onkeydown={(e) => keyboardActivate(e, () => tapModifier('alt'))}
			aria-pressed={active === 'alt'}
			aria-label={locked && active === 'alt' ? 'Alt (locked)' : 'Alt'}
			title="Alt — applies to the next key; tap twice to lock"
		>
			ALT{active === 'alt' && locked ? ' 🔒' : ''}
		</button>
		{#each row1 as key (key.label)}
			<button
				type="button"
				class={keyClass}
				onpointerdown={(e) => pressKey(e, key)}
				onkeydown={(e) => keyboardActivate(e, () => fire(key))}
				aria-label={key.title ?? key.label}
				title={key.title}
			>
				{key.label}
			</button>
		{/each}
		<button
			type="button"
			class="{keyClass} flex items-center justify-center"
			onpointerdown={(e) => pressAction(e, () => onPalette?.())}
			onkeydown={(e) => keyboardActivate(e, () => onPalette?.())}
			aria-label="Command palette"
			title="Command palette"
		>
			<Command class="w-3.5 h-3.5" />
		</button>
		<button
			type="button"
			class="{keyClass} flex items-center justify-center !flex-none w-8"
			onpointerdown={(e) => pressAction(e, toggleSecondRow)}
			onkeydown={(e) => keyboardActivate(e, toggleSecondRow)}
			aria-label={showSecondRow ? 'Hide extra keys' : 'Show extra keys'}
			aria-expanded={showSecondRow}
		>
			{#if showSecondRow}<ChevronDown class="w-3.5 h-3.5" />{:else}<ChevronUp
					class="w-3.5 h-3.5"
				/>{/if}
		</button>
	</div>
	{#if showSecondRow}
		<div class="flex gap-[3px]">
			{#each row2 as key (key.label)}
				<button
					type="button"
					class={keyClass}
					onpointerdown={(e) => pressKey(e, key)}
					onkeydown={(e) => keyboardActivate(e, () => fire(key))}
					aria-label={key.title ?? key.label}
					title={key.title}
				>
					{key.label}
				</button>
			{/each}
		</div>
	{/if}
</div>
