<script lang="ts">
	import type { TerminalTab } from '$lib/types/index.js';

	interface Props {
		tabs: TerminalTab[];
		activeTabId: string | null;
		onTapDots?: () => void;
	}

	let { tabs, activeTabId, onTapDots }: Props = $props();

	const MAX_DOTS = 8;

	let statusColor = (tab: TerminalTab) => {
		if (tab.id === activeTabId) return 'bg-accent';
		switch (tab.status) {
			case 'connected': return 'bg-success';
			case 'connecting': return 'bg-warning animate-pulse';
			case 'error': return 'bg-error';
			default: return 'bg-text-muted';
		}
	};

	let overflow = $derived(tabs.length > MAX_DOTS ? tabs.length - MAX_DOTS : 0);
	let visibleTabs = $derived(tabs.slice(0, MAX_DOTS));
</script>

{#if tabs.length >= 2}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="flex items-center gap-[3px] cursor-pointer px-1"
		onclick={() => onTapDots?.()}
		role="button"
		tabindex="0"
		aria-label="Open tab list ({tabs.length} tabs)"
		onkeydown={(e) => { if (e.key === 'Enter') onTapDots?.(); }}
	>
		{#each visibleTabs as tab (tab.id)}
			<span
				class="rounded-full flex-shrink-0 {statusColor(tab)} {tab.id === activeTabId ? 'w-1.5 h-1.5' : 'w-1 h-1'}"
			></span>
		{/each}
		{#if overflow > 0}
			<span class="text-[9px] text-text-muted ml-0.5">+{overflow}</span>
		{/if}
	</div>
{/if}
