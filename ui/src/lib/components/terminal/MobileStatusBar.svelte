<script lang="ts">
	import { Menu, Plus, X, ChevronUp } from 'lucide-svelte';
	import TabDots from './TabDots.svelte';
	import type { TerminalTab, ConnectionStatus } from '$lib/types/index.js';

	interface Props {
		serverName: string | null;
		connectionStatus: ConnectionStatus;
		tabs: TerminalTab[];
		activeTabId: string | null;
		onTapDots?: () => void;
		onNewTab?: () => void;
		onCloseTab?: () => void;
		onOpenPalette?: () => void;
	}

	let { serverName, connectionStatus, tabs, activeTabId, onTapDots, onNewTab, onCloseTab, onOpenPalette }: Props = $props();

	function toggleSidebar() {
		window.dispatchEvent(new CustomEvent('toggle-sidebar'));
	}

	let statusDotColor = $derived.by(() => {
		switch (connectionStatus) {
			case 'connected': return 'bg-success';
			case 'connecting': return 'bg-warning animate-pulse';
			case 'error': return 'bg-error';
			default: return 'bg-text-muted';
		}
	});
</script>

<div class="flex items-center justify-between px-2 pt-[max(0.25rem,env(safe-area-inset-top))] pb-0.5 bg-surface border-b border-border">
	<!-- Left: hamburger + connection dot + server name -->
	<div class="flex items-center gap-1.5 min-w-0 flex-1">
		<button
			class="p-1 text-text-muted hover:text-text-primary flex-shrink-0"
			onclick={toggleSidebar}
			aria-label="Open menu"
		>
			<Menu class="w-4 h-4" />
		</button>
		<span class="w-1.5 h-1.5 rounded-full flex-shrink-0 {statusDotColor}"></span>
		<span class="text-[11px] text-accent font-medium truncate max-w-[140px]">
			{serverName ?? 'No server'}
		</span>
	</div>

	<!-- Right: actions + tab dots -->
	<div class="flex items-center gap-1 flex-shrink-0">
		<button
			class="p-1 text-text-muted hover:text-accent"
			onclick={onOpenPalette}
			aria-label="Command palette"
		>
			<ChevronUp class="w-3.5 h-3.5" />
		</button>
		<button
			class="p-1 text-text-muted hover:text-text-primary"
			onclick={onNewTab}
			aria-label="New tab"
		>
			<Plus class="w-3.5 h-3.5" />
		</button>
		<button
			class="p-1 text-text-muted hover:text-error"
			onclick={onCloseTab}
			aria-label="Close tab"
		>
			<X class="w-3.5 h-3.5" />
		</button>
		<TabDots {tabs} {activeTabId} {onTapDots} />
	</div>
</div>
