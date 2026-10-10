<script lang="ts">
	interface Props {
		onKeyPress: (sequence: string) => void;
		/** CTL/ALT also apply to the next character typed on the soft keyboard */
		onModifier?: (mod: 'ctl' | 'alt' | null) => void;
	}

	let { onKeyPress, onModifier }: Props = $props();

	/** Called by the pane once a typed character consumed the modifier */
	export function clearModifiers() {
		ctlActive = false;
		altActive = false;
	}

	let ctlActive = $state(false);
	let altActive = $state(false);

	function sendKey(key: string, sequence: string) {
		if (ctlActive) {
			// Ctrl+key: convert to control character
			const code = key.toUpperCase().charCodeAt(0) - 64;
			if (code >= 0 && code <= 31) {
				onKeyPress(String.fromCharCode(code));
			} else {
				onKeyPress(sequence);
			}
			ctlActive = false;
			altActive = false;
			onModifier?.(null);
			return;
		}
		if (altActive) {
			// Alt+key: prepend ESC
			onKeyPress('\x1b' + key);
			altActive = false;
			ctlActive = false;
			onModifier?.(null);
			return;
		}
		onKeyPress(sequence);
	}

	function toggleCtl() {
		ctlActive = !ctlActive;
		if (ctlActive) altActive = false;
		onModifier?.(ctlActive ? 'ctl' : null);
	}

	function toggleAlt() {
		altActive = !altActive;
		if (altActive) ctlActive = false;
		onModifier?.(altActive ? 'alt' : null);
	}

	const keys = [
		{ label: '^C', action: () => onKeyPress('\x03') },
		{ label: 'ESC', action: () => sendKey('', '\x1b') },
		{ label: 'TAB', action: () => sendKey('', '\t') },
		{ label: 'CTL', action: toggleCtl, toggle: true, active: () => ctlActive },
		{ label: 'ALT', action: toggleAlt, toggle: true, active: () => altActive },
		{ label: '↑', action: () => sendKey('', '\x1b[A') },
		{ label: '↓', action: () => sendKey('', '\x1b[B') },
		{ label: '←', action: () => sendKey('', '\x1b[D') },
		{ label: '→', action: () => sendKey('', '\x1b[C') },
		{ label: '|', action: () => sendKey('|', '|') }
	];
</script>

<div class="flex gap-[2px] px-1 py-1 bg-surface-raised border-t border-border flex-shrink-0">
	{#each keys as key (key.label)}
		<button
			class="flex-1 py-1.5 text-[11px] font-mono rounded-sm border transition-colors
				{key.toggle && key.active?.()
				? 'bg-accent/20 text-accent border-accent/40'
				: 'bg-surface border-border text-text-secondary hover:text-text-primary hover:bg-surface-overlay'}"
			onpointerdown={(e) => e.preventDefault()}
			onclick={key.action}
			aria-label={key.label}
		>
			{key.label}
		</button>
	{/each}
</div>
