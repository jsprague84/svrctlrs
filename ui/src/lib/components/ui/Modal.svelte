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

	let dialogEl: HTMLDivElement | undefined = $state();
	let contentEl: HTMLDivElement | undefined = $state();
	let previousFocus: Element | null = null;

	const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

	$effect(() => {
		if (open && dialogEl) {
			previousFocus = document.activeElement;

			// Focus first focusable element inside modal content (skip backdrop button)
			requestAnimationFrame(() => {
				const first = contentEl?.querySelector<HTMLElement>(FOCUSABLE);
				first?.focus();
			});

			return () => {
				if (previousFocus instanceof HTMLElement) {
					previousFocus.focus();
				}
			};
		}
	});

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose();
			return;
		}

		if (e.key === 'Tab' && contentEl) {
			const focusable = Array.from(contentEl.querySelectorAll<HTMLElement>(FOCUSABLE));
			if (focusable.length === 0) return;

			const first = focusable[0];
			const last = focusable[focusable.length - 1];

			if (e.shiftKey) {
				if (document.activeElement === first) {
					e.preventDefault();
					last.focus();
				}
			} else {
				if (document.activeElement === last) {
					e.preventDefault();
					first.focus();
				}
			}
		}
	}
</script>

{#if open}
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		bind:this={dialogEl}
		class="fixed inset-0 z-50 flex items-center justify-center"
		role="dialog"
		aria-modal="true"
		aria-label={title}
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
		<div bind:this={contentEl} class="relative bg-surface border border-border rounded-lg shadow-lg w-[calc(100%-1rem)] md:w-full max-w-lg mx-auto max-h-[90vh] flex flex-col">
			<!-- Header -->
			<div class="flex items-center justify-between px-4 py-3 border-b border-border">
				<h2 class="text-sm font-semibold text-text-primary">{title}</h2>
				<button class="p-1 text-text-muted hover:text-text-primary rounded-sm" onclick={onClose} aria-label="Close dialog">
					<X class="w-4 h-4" />
				</button>
			</div>

			<!-- Body -->
			<div class="flex-1 overflow-y-auto px-4 py-4">
				{@render children()}
			</div>

			<!-- Footer -->
			{#if footer}
				<div class="flex items-center justify-end gap-2 px-4 py-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] border-t border-border">
					{@render footer()}
				</div>
			{/if}
		</div>
	</div>
{/if}
