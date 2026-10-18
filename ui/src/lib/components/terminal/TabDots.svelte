<script lang="ts">
	import type { TerminalTab } from '#lib/types/index.js';

	/** One dot per tab, coloured by that tab's own state; the active one is ringed. Presentational —
	 *  the parent decides what a tap does. */
	interface Props {
		tabs: TerminalTab[];
		activeTabId: string | null;
	}

	let { tabs, activeTabId }: Props = $props();

	const MAX_DOTS = 8;

	function statusColor(tab: TerminalTab): string {
		switch (tab.status) {
			case 'connected':
				return 'bg-success';
			case 'connecting':
				return 'bg-warning animate-pulse';
			case 'error':
				return 'bg-error';
			default:
				return 'bg-text-muted';
		}
	}

	let overflow = $derived(tabs.length > MAX_DOTS ? tabs.length - MAX_DOTS : 0);
	let visibleTabs = $derived(tabs.slice(0, MAX_DOTS));
</script>

{#if tabs.length >= 2}
	<span class="flex items-center gap-1 px-0.5" aria-hidden="true">
		{#each visibleTabs as tab (tab.id)}
			<span
				class="rounded-full flex-shrink-0 w-1.5 h-1.5 {statusColor(tab)} {tab.id === activeTabId
					? 'ring-2 ring-accent/70 ring-offset-1 ring-offset-surface'
					: tab.bell
						? 'ring-2 ring-warning/70 ring-offset-1 ring-offset-surface'
						: 'opacity-70'}"
			></span>
		{/each}
		{#if overflow > 0}
			<span class="text-[9px] text-text-muted ml-0.5">+{overflow}</span>
		{/if}
	</span>
{/if}
