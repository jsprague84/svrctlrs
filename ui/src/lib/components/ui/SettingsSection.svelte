<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		description?: string;
		/** Draw as a bordered card instead of a bare section */
		card?: boolean;
		/** Right-aligned actions next to the heading */
		actions?: Snippet;
		children: Snippet;
	}

	let { title, description, card = false, actions, children }: Props = $props();
</script>

<section class={card ? 'p-4 bg-surface border border-border rounded-lg' : ''}>
	<div class="flex items-start justify-between gap-3 mb-3">
		<div>
			<h3 class="text-[11px] uppercase tracking-wider text-text-muted font-semibold">{title}</h3>
			{#if description}
				<p class="text-xs text-text-muted mt-1">{description}</p>
			{/if}
		</div>
		{#if actions}
			<div class="flex items-center gap-2 flex-shrink-0">{@render actions()}</div>
		{/if}
	</div>
	{@render children()}
</section>
