<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		children?: Snippet;
	}

	let { children }: Props = $props();
</script>

<header class="flex items-center gap-3 px-4 py-2 bg-surface border-b border-border min-h-[40px]">
	{#if children}
		{@render children()}
	{/if}
</header>
