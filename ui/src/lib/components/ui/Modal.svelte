<script lang="ts">
	import type { Snippet } from 'svelte';
	import { X } from 'lucide-svelte';

	interface Props {
		open: boolean;
		title: string;
		onClose: () => void;
		children: Snippet;
		footer?: Snippet;
	}

	let { open, title, onClose, children, footer }: Props = $props();

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onClose();
	}
</script>

{#if open}
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class="fixed inset-0 z-50 flex items-center justify-center"
		role="dialog"
		aria-modal="true"
		tabindex="-1"
		onkeydown={handleKeydown}
	>
		<!-- Backdrop -->
		<button
			class="absolute inset-0 bg-black/60"
			onclick={onClose}
			tabindex="-1"
			aria-label="Close"
		></button>

		<!-- Content -->
		<div class="relative bg-surface border border-border rounded-lg shadow-lg w-[calc(100%-1rem)] md:w-full max-w-lg mx-auto max-h-[90vh] flex flex-col">
			<!-- Header -->
			<div class="flex items-center justify-between px-4 py-3 border-b border-border">
				<h2 class="text-sm font-semibold text-text-primary">{title}</h2>
				<button class="p-1 text-text-muted hover:text-text-primary rounded-sm" onclick={onClose}>
					<X class="w-4 h-4" />
				</button>
			</div>

			<!-- Body -->
			<div class="flex-1 overflow-y-auto px-4 py-4">
				{@render children()}
			</div>

			<!-- Footer -->
			{#if footer}
				<div class="flex items-center justify-end gap-2 px-4 py-3 border-t border-border">
					{@render footer()}
				</div>
			{/if}
		</div>
	</div>
{/if}
