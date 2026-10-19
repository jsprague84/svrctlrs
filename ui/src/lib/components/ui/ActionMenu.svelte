<script lang="ts" module>
	import type { Pencil } from '@lucide/svelte';

	export interface MenuItem {
		label: string;
		icon?: typeof Pencil;
		onSelect: () => void;
		destructive?: boolean;
		disabled?: boolean;
		/** Draws attention (a warning dot): the action is what the row needs next */
		attention?: boolean;
	}
</script>

<script lang="ts">
	/**
	 * A "⋯" button that opens a short list of actions. Rows use it on narrow screens instead of
	 * a strip of icon buttons. The menu is positioned against the viewport so a scrolling list
	 * cannot clip it, opens upwards near the bottom of the screen, and closes on Escape, a tap
	 * outside, scrolling or resizing.
	 */
	import { Ellipsis } from '@lucide/svelte';
	import { overlay } from '#lib/utils/overlay.js';

	interface Props {
		items: MenuItem[];
		/** Accessible name of the trigger and the menu, e.g. "Actions for web-01" */
		label: string;
		class?: string;
	}

	let { items, label, class: className = '' }: Props = $props();

	let open = $state(false);
	let trigger: HTMLButtonElement | undefined = $state();
	let place = $state<{ right: number; top?: number; bottom?: number }>({ right: 0 });

	const ITEM_HEIGHT = 44;

	function toggle() {
		if (open || !trigger) {
			open = false;
			return;
		}
		const r = trigger.getBoundingClientRect();
		const needed = items.length * ITEM_HEIGHT + 12;
		const below = window.innerHeight - r.bottom;
		const right = Math.max(8, window.innerWidth - r.right);
		place =
			below < needed && r.top > below
				? { right, bottom: window.innerHeight - r.top + 4 }
				: { right, top: r.bottom + 4 };
		open = true;
	}

	function choose(item: MenuItem) {
		open = false;
		item.onSelect();
	}

	function onMenuKeydown(e: KeyboardEvent) {
		if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) return;
		e.preventDefault();
		const menu = e.currentTarget as HTMLElement;
		const entries = Array.from(
			menu.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not([disabled])')
		);
		if (entries.length === 0) return;
		const i = entries.indexOf(document.activeElement as HTMLButtonElement);
		const next =
			e.key === 'Home'
				? 0
				: e.key === 'End'
					? entries.length - 1
					: e.key === 'ArrowDown'
						? (i + 1) % entries.length
						: (i - 1 + entries.length) % entries.length;
		entries[next].focus();
	}

	$effect(() => {
		if (!open) return;
		const close = () => (open = false);
		window.addEventListener('scroll', close, { capture: true, passive: true });
		window.addEventListener('resize', close);
		return () => {
			window.removeEventListener('scroll', close, { capture: true });
			window.removeEventListener('resize', close);
		};
	});
</script>

<button
	bind:this={trigger}
	type="button"
	class="relative w-11 h-11 flex items-center justify-center rounded-md text-text-secondary hover:bg-surface-raised active:bg-surface-raised focus-visible:outline-2 focus-visible:outline-accent {className}"
	aria-haspopup="menu"
	aria-expanded={open}
	aria-label={label}
	title={label}
	onclick={toggle}
>
	<Ellipsis class="w-5 h-5" />
	{#if items.some((i) => i.attention)}
		<span class="absolute top-2 right-2 w-2 h-2 rounded-full bg-warning" aria-hidden="true"></span>
	{/if}
</button>

{#if open}
	<button
		type="button"
		class="fixed inset-0 z-40 cursor-default"
		tabindex="-1"
		aria-label="Close menu"
		onclick={() => (open = false)}
	></button>
	<div
		role="menu"
		aria-label={label}
		tabindex="-1"
		class="fixed z-50 min-w-52 max-w-[calc(100vw-1rem)] py-1 bg-surface border border-border rounded-lg shadow-lg"
		style:right="{place.right}px"
		style:top={place.top !== undefined ? `${place.top}px` : undefined}
		style:bottom={place.bottom !== undefined ? `${place.bottom}px` : undefined}
		onkeydown={onMenuKeydown}
		{@attach overlay({ onEscape: () => (open = false) })}
	>
		{#each items as item (item.label)}
			<button
				type="button"
				role="menuitem"
				class="flex w-full items-center gap-3 px-3 min-h-11 text-sm text-left disabled:opacity-50
					{item.destructive
					? 'text-error hover:bg-error/10'
					: 'text-text-primary hover:bg-surface-raised'} focus-visible:bg-surface-raised focus-visible:outline-none"
				disabled={item.disabled}
				onclick={() => choose(item)}
			>
				{#if item.icon}
					<item.icon class="w-4 h-4 flex-shrink-0 {item.destructive ? '' : 'text-text-muted'}" />
				{/if}
				<span class="flex-1">{item.label}</span>
				{#if item.attention}
					<span class="w-2 h-2 rounded-full bg-warning" aria-label="recommended"></span>
				{/if}
			</button>
		{/each}
	</div>
{/if}
