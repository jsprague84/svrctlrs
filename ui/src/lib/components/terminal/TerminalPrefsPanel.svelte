<script lang="ts">
	import { X } from 'lucide-svelte';
	import * as terminalPrefs from '$lib/state/terminalPrefs.svelte.js';
	import type { CursorStyle, FontWeightOption } from '$lib/types/index.js';

	interface Props {
		open: boolean;
		onClose: () => void;
	}

	let { open, onClose }: Props = $props();

	let prefs = $derived(terminalPrefs.getPrefs());

	const fontFamilies = terminalPrefs.FONT_FAMILIES;

	function currentFontLabel(): string {
		for (const [label, value] of Object.entries(fontFamilies)) {
			if (value === prefs.fontFamily) return label;
		}
		return 'JetBrains Mono';
	}

	function handleReset() {
		if (confirm('Reset all terminal preferences to defaults?')) {
			terminalPrefs.resetPrefs();
		}
	}
</script>

<!-- Backdrop (mobile only) -->
{#if open}
	<button
		tabindex="-1"
		aria-label="Close preferences"
		class="fixed inset-0 bg-black/40 z-20 md:hidden"
		onclick={onClose}
	></button>
{/if}

<div
	class="fixed bottom-0 left-0 right-0 h-[70vh] rounded-t-xl z-30
		md:absolute md:top-0 md:bottom-0 md:right-0 md:left-auto md:h-auto md:w-72 md:rounded-none md:z-20
		bg-surface border-t border-border md:border-t-0 md:border-l
		flex flex-col transition-transform duration-200 ease-in-out overflow-hidden
		{open ? '' : 'translate-y-full md:translate-y-0 md:translate-x-full'}"
>
	<!-- Drag handle (mobile only) -->
	<div class="flex justify-center pt-2 pb-1 md:hidden">
		<div class="w-10 h-1 rounded-full bg-text-muted/30"></div>
	</div>

	<!-- Header -->
	<div class="flex items-center justify-between px-3 py-2 border-b border-border">
		<span class="text-sm font-semibold text-text-primary">Terminal Preferences</span>
		<button class="p-1 text-text-muted hover:text-text-primary" onclick={onClose}>
			<X class="w-4 h-4" />
		</button>
	</div>

	<!-- Scrollable body -->
	<div class="flex-1 overflow-y-auto px-3 py-2 space-y-4">
		<!-- Font -->
		<section>
			<h3 class="text-[10px] uppercase tracking-wider text-text-muted mb-2">Font</h3>
			<div class="space-y-2">
				<label class="block">
					<span class="text-xs text-text-secondary">Family</span>
					<select
						class="mt-0.5 w-full px-2 py-1 text-xs bg-input border border-border rounded-sm text-text-primary"
						value={currentFontLabel()}
						onchange={(e) => {
							const label = (e.target as HTMLSelectElement).value;
							terminalPrefs.updatePref('fontFamily', fontFamilies[label]);
						}}
					>
						{#each Object.keys(fontFamilies) as label}
							<option value={label}>{label}</option>
						{/each}
					</select>
				</label>

				<label class="block">
					<span class="text-xs text-text-secondary">Size: {prefs.fontSize}px</span>
					<input
						type="range"
						min="8"
						max="24"
						step="1"
						value={prefs.fontSize}
						oninput={(e) => terminalPrefs.updatePref('fontSize', Number((e.target as HTMLInputElement).value))}
						class="mt-0.5 w-full accent-accent"
					/>
				</label>

				<label class="block">
					<span class="text-xs text-text-secondary">Weight</span>
					<select
						class="mt-0.5 w-full px-2 py-1 text-xs bg-input border border-border rounded-sm text-text-primary"
						value={prefs.fontWeight}
						onchange={(e) => terminalPrefs.updatePref('fontWeight', Number((e.target as HTMLSelectElement).value) as FontWeightOption)}
					>
						<option value={300}>300 (Light)</option>
						<option value={400}>400 (Regular)</option>
						<option value={500}>500 (Medium)</option>
						<option value={600}>600 (Semi-Bold)</option>
						<option value={700}>700 (Bold)</option>
					</select>
				</label>

				<label class="block">
					<span class="text-xs text-text-secondary">Bold Weight</span>
					<select
						class="mt-0.5 w-full px-2 py-1 text-xs bg-input border border-border rounded-sm text-text-primary"
						value={prefs.fontWeightBold}
						onchange={(e) => terminalPrefs.updatePref('fontWeightBold', Number((e.target as HTMLSelectElement).value) as FontWeightOption)}
					>
						<option value={300}>300 (Light)</option>
						<option value={400}>400 (Regular)</option>
						<option value={500}>500 (Medium)</option>
						<option value={600}>600 (Semi-Bold)</option>
						<option value={700}>700 (Bold)</option>
					</select>
				</label>

				<label class="block">
					<span class="text-xs text-text-secondary">Line Height: {prefs.lineHeight.toFixed(1)}</span>
					<input
						type="range"
						min="1.0"
						max="2.0"
						step="0.1"
						value={prefs.lineHeight}
						oninput={(e) => terminalPrefs.updatePref('lineHeight', Number((e.target as HTMLInputElement).value))}
						class="mt-0.5 w-full accent-accent"
					/>
				</label>

				<label class="block">
					<span class="text-xs text-text-secondary">Letter Spacing: {prefs.letterSpacing.toFixed(1)}px</span>
					<input
						type="range"
						min="-1"
						max="5"
						step="0.5"
						value={prefs.letterSpacing}
						oninput={(e) => terminalPrefs.updatePref('letterSpacing', Number((e.target as HTMLInputElement).value))}
						class="mt-0.5 w-full accent-accent"
					/>
				</label>
			</div>
		</section>

		<!-- Cursor -->
		<section>
			<h3 class="text-[10px] uppercase tracking-wider text-text-muted mb-2">Cursor</h3>
			<div class="space-y-2">
				<label class="block">
					<span class="text-xs text-text-secondary">Style</span>
					<select
						class="mt-0.5 w-full px-2 py-1 text-xs bg-input border border-border rounded-sm text-text-primary"
						value={prefs.cursorStyle}
						onchange={(e) => terminalPrefs.updatePref('cursorStyle', (e.target as HTMLSelectElement).value as CursorStyle)}
					>
						<option value="block">Block</option>
						<option value="underline">Underline</option>
						<option value="bar">Bar</option>
					</select>
				</label>

				<label class="flex items-center gap-2">
					<input
						type="checkbox"
						checked={prefs.cursorBlink}
						onchange={(e) => terminalPrefs.updatePref('cursorBlink', (e.target as HTMLInputElement).checked)}
						class="accent-accent"
					/>
					<span class="text-xs text-text-secondary">Cursor Blink</span>
				</label>
			</div>
		</section>

		<!-- Scrolling -->
		<section>
			<h3 class="text-[10px] uppercase tracking-wider text-text-muted mb-2">Scrolling</h3>
			<div class="space-y-2">
				<label class="block">
					<span class="text-xs text-text-secondary">Scrollback Lines</span>
					<input
						type="number"
						min="1000"
						max="100000"
						step="1000"
						value={prefs.scrollback}
						onchange={(e) => terminalPrefs.updatePref('scrollback', Number((e.target as HTMLInputElement).value))}
						class="mt-0.5 w-full px-2 py-1 text-xs bg-input border border-border rounded-sm text-text-primary"
					/>
				</label>

				<label class="block">
					<span class="text-xs text-text-secondary">Scroll Sensitivity: {prefs.scrollSensitivity}</span>
					<input
						type="range"
						min="1"
						max="10"
						step="1"
						value={prefs.scrollSensitivity}
						oninput={(e) => terminalPrefs.updatePref('scrollSensitivity', Number((e.target as HTMLInputElement).value))}
						class="mt-0.5 w-full accent-accent"
					/>
				</label>

				<label class="block">
					<span class="text-xs text-text-secondary">Fast Scroll Sensitivity: {prefs.fastScrollSensitivity}</span>
					<input
						type="range"
						min="1"
						max="20"
						step="1"
						value={prefs.fastScrollSensitivity}
						oninput={(e) => terminalPrefs.updatePref('fastScrollSensitivity', Number((e.target as HTMLInputElement).value))}
						class="mt-0.5 w-full accent-accent"
					/>
				</label>

				<label class="block">
					<span class="text-xs text-text-secondary">Smooth Scroll: {prefs.smoothScrollDuration}ms</span>
					<input
						type="range"
						min="0"
						max="300"
						step="25"
						value={prefs.smoothScrollDuration}
						oninput={(e) => terminalPrefs.updatePref('smoothScrollDuration', Number((e.target as HTMLInputElement).value))}
						class="mt-0.5 w-full accent-accent"
					/>
				</label>
			</div>
		</section>

		<!-- Rendering -->
		<section>
			<h3 class="text-[10px] uppercase tracking-wider text-text-muted mb-2">Rendering</h3>
			<div class="space-y-2">
				<label class="flex items-center gap-2">
					<input
						type="checkbox"
						checked={prefs.drawBoldTextInBrightColors}
						onchange={(e) => terminalPrefs.updatePref('drawBoldTextInBrightColors', (e.target as HTMLInputElement).checked)}
						class="accent-accent"
					/>
					<span class="text-xs text-text-secondary">Bold as Bright Colors</span>
				</label>

				<label class="block">
					<span class="text-xs text-text-secondary">Minimum Contrast Ratio</span>
					<select
						class="mt-0.5 w-full px-2 py-1 text-xs bg-input border border-border rounded-sm text-text-primary"
						value={prefs.minimumContrastRatio}
						onchange={(e) => terminalPrefs.updatePref('minimumContrastRatio', Number((e.target as HTMLSelectElement).value))}
					>
						<option value={1}>1 (Off)</option>
						<option value={4.5}>4.5 (AA)</option>
						<option value={7}>7 (AAA)</option>
						<option value={21}>21 (Maximum)</option>
					</select>
				</label>
			</div>
		</section>

		<!-- Behavior -->
		<section>
			<h3 class="text-[10px] uppercase tracking-wider text-text-muted mb-2">Behavior</h3>
			<div class="space-y-2">
				<label class="block">
					<span class="text-xs text-text-secondary">Tab Stop Width</span>
					<select
						class="mt-0.5 w-full px-2 py-1 text-xs bg-input border border-border rounded-sm text-text-primary"
						value={prefs.tabStopWidth}
						onchange={(e) => terminalPrefs.updatePref('tabStopWidth', Number((e.target as HTMLSelectElement).value))}
					>
						<option value={2}>2</option>
						<option value={4}>4</option>
						<option value={8}>8</option>
					</select>
				</label>

				<label class="flex items-center gap-2">
					<input
						type="checkbox"
						checked={prefs.scrollOnUserInput}
						onchange={(e) => terminalPrefs.updatePref('scrollOnUserInput', (e.target as HTMLInputElement).checked)}
						class="accent-accent"
					/>
					<span class="text-xs text-text-secondary">Scroll on User Input</span>
				</label>
			</div>
		</section>
	</div>

	<!-- Footer -->
	<div class="px-3 py-2 border-t border-border">
		<button
			class="w-full px-3 py-1.5 text-xs bg-destructive/10 text-destructive rounded-sm hover:bg-destructive/20"
			onclick={handleReset}
		>
			Reset to Defaults
		</button>
	</div>
</div>
