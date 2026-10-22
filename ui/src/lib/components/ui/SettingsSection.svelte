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
			<h3 class="text-sm font-semibold text-text-primary">{title}</h3>
			{#if description}
				<p class="text-xs text-text-secondary mt-0.5 max-w-prose">{description}</p>
			{/if}
		</div>
		{#if actions}
			<div class="flex items-center gap-2 flex-shrink-0">{@render actions()}</div>
		{/if}
	</div>
	{@render children()}
</section>
