<script lang="ts">
	import type { Snippet } from 'svelte';
	import { X } from '@lucide/svelte';
	import { overlay } from '#lib/utils/overlay.js';

	interface Props {
		open: boolean;
		title: string;
		onClose: () => void;
		children: Snippet;
		footer?: Snippet;
	}

	let { open, title, onClose, children, footer }: Props = $props();
</script>

{#if open}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center"
		role="dialog"
		aria-modal="true"
		aria-label={title}
		tabindex="-1"
		{@attach overlay({ onEscape: () => onClose() })}
	>
		<!-- Backdrop -->
		<button class="absolute inset-0 bg-black/60" onclick={onClose} tabindex="-1" aria-label="Close"
		></button>

		<!-- Content -->
		<div
			class="relative bg-surface border border-border rounded-lg shadow-lg w-[calc(100%-1rem)] md:w-full max-w-lg mx-auto max-h-[90dvh] flex flex-col"
		>
			<!-- Header -->
			<div class="flex items-center justify-between px-4 py-3 border-b border-border">
				<h2 class="text-sm font-semibold text-text-primary">{title}</h2>
				<button
					class="w-11 h-11 -mr-3 md:w-auto md:h-auto md:mr-0 md:p-1 flex items-center justify-center text-text-muted hover:text-text-primary rounded-sm"
					onclick={onClose}
					aria-label="Close dialog"
				>
					<X class="w-4 h-4" />
				</button>
			</div>

			<!-- Body -->
			<div class="flex-1 overflow-y-auto px-4 py-4">
				{@render children()}
			</div>

			<!-- Footer -->
			{#if footer}
				<div
					class="flex items-center justify-end gap-2 px-4 py-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] border-t border-border"
				>
					{@render footer()}
				</div>
			{/if}
		</div>
	</div>
{/if}
