<script lang="ts">
	/** A row of tag filter chips; one tag at a time, or none. Renders nothing without tags. */
	interface Props {
		tags: string[];
		selected: string | null;
		onSelect: (tag: string | null) => void;
		/** Sidebar styling (darker surface) */
		compact?: boolean;
	}

	let { tags, selected, onSelect, compact = false }: Props = $props();

	const base = $derived(
		compact
			? 'px-1.5 py-0.5 text-[10px] rounded-sm border transition-colors'
			: 'px-2 py-0.5 text-xs rounded-full border transition-colors min-h-7 md:min-h-0'
	);
	const on = 'bg-accent/20 border-accent/40 text-accent';
	const off = $derived(
		compact
			? 'border-sidebar-border text-sidebar-muted hover:text-sidebar-foreground'
			: 'border-border text-text-secondary hover:text-text-primary hover:bg-surface-raised'
	);
</script>

{#if tags.length > 0}
	<div class="flex flex-wrap gap-1" role="group" aria-label="Filter by tag">
		<button
			type="button"
			class="{base} {selected === null ? on : off}"
			aria-pressed={selected === null}
			onclick={() => onSelect(null)}>All</button
		>
		{#each tags as tag (tag)}
			<button
				type="button"
				class="{base} {selected?.toLowerCase() === tag.toLowerCase() ? on : off}"
				aria-pressed={selected?.toLowerCase() === tag.toLowerCase()}
				onclick={() => onSelect(selected?.toLowerCase() === tag.toLowerCase() ? null : tag)}
				>{tag}</button
			>
		{/each}
	</div>
{/if}
