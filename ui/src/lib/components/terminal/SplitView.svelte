<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { LayoutMode } from '$lib/types/index.js';
	import { isMobile } from '$lib/state/mobile.svelte.js';

	interface Props {
		layout: LayoutMode;
		slotCount: number;
		children: Snippet;
	}

	let { layout, slotCount, children }: Props = $props();

	let gridClass = $derived.by(() => {
		if (isMobile() || slotCount <= 1 || layout === 'single') return 'grid-cols-1 grid-rows-1';
		if (layout === 'split-h') return 'grid-cols-2 grid-rows-1';
		if (layout === 'split-v') return 'grid-cols-1 grid-rows-2';
		return 'grid-cols-2 grid-rows-2'; // quad
	});
</script>

<div class="grid h-full w-full gap-px bg-border overflow-hidden {gridClass}">
	{@render children()}
</div>
