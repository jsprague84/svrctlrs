<script lang="ts">
	import { Menu } from 'lucide-svelte';
	import TabDots from './TabDots.svelte';
	import type { TerminalTab, ConnectionStatus } from '$lib/types/index.js';

	interface Props {
		serverName: string | null;
		connectionStatus: ConnectionStatus;
		tabs: TerminalTab[];
		activeTabId: string | null;
		onTapDots?: () => void;
	}

	let { serverName, connectionStatus, tabs, activeTabId, onTapDots }: Props = $props();

	function toggleSidebar() {
		window.dispatchEvent(new CustomEvent('toggle-sidebar'));
	}

	let statusDotColor = $derived(() => {
		switch (connectionStatus) {
			case 'connected': return 'bg-success';
			case 'connecting': return 'bg-warning animate-pulse';
			case 'error': return 'bg-error';
			default: return 'bg-text-muted';
		}
	});
</script>

<div class="flex items-center justify-between px-2 pt-[max(0.25rem,env(safe-area-inset-top))] pb-0.5 bg-surface border-b border-border">
	<!-- Left: connection dot + server name -->
	<div class="flex items-center gap-1.5 min-w-0 flex-1">
		<span class="w-1.5 h-1.5 rounded-full flex-shrink-0 {statusDotColor()}"></span>
		<span class="text-[11px] text-accent font-medium truncate max-w-[140px]">
			{serverName ?? 'No server'}
		</span>
	</div>

	<!-- Center/Right: tab dots + hamburger -->
	<div class="flex items-center gap-2 flex-shrink-0">
		<TabDots {tabs} {activeTabId} {onTapDots} />
		<button
			class="p-1 text-text-muted hover:text-text-primary"
			onclick={toggleSidebar}
			aria-label="Open menu"
		>
			<Menu class="w-4 h-4" />
		</button>
	</div>
</div>
