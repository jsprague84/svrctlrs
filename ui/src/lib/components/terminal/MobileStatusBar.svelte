<script lang="ts">
	import { Menu, Plus, Settings2 } from '@lucide/svelte';
	import TabDots from './TabDots.svelte';
	import type { TerminalTab, ConnectionStatus } from '#lib/types/index.js';

	interface Props {
		serverName: string | null;
		connectionStatus: ConnectionStatus;
		tabs: TerminalTab[];
		activeTabId: string | null;
		/** Open the tab list sheet */
		onTapDots?: () => void;
		/** Open the server picker for a new tab */
		onNewTab?: () => void;
		onOpenPrefs?: () => void;
	}

	let { serverName, connectionStatus, tabs, activeTabId, onTapDots, onNewTab, onOpenPrefs }: Props =
		$props();

	function toggleSidebar() {
		window.dispatchEvent(new CustomEvent('toggle-sidebar'));
	}

	let statusDotColor = $derived.by(() => {
		switch (connectionStatus) {
			case 'connected':
				return 'bg-success';
			case 'connecting':
				return 'bg-warning animate-pulse';
			case 'error':
				return 'bg-error';
			default:
				return 'bg-text-muted';
		}
	});
	let statusLabel = $derived(
		connectionStatus === 'connected'
			? 'connected'
			: connectionStatus === 'connecting'
				? 'connecting'
				: connectionStatus === 'error'
					? 'failed'
					: 'disconnected'
	);

	// 40px targets throughout; the bar is the only chrome on a phone
	const btn =
		'flex items-center justify-center w-10 h-10 rounded-md text-text-muted active:bg-surface-raised active:text-text-primary select-none';
</script>

<div
	class="flex items-center justify-between px-1 pt-[env(safe-area-inset-top,0px)] bg-surface border-b border-border select-none"
	role="toolbar"
	aria-label="Terminal"
>
	<div class="flex items-center min-w-0 flex-1">
		<button type="button" class={btn} onclick={toggleSidebar} aria-label="Open menu">
			<Menu class="w-5 h-5" />
		</button>
		<!-- Server + state: tapping opens the tab list, where everything else lives -->
		<button
			type="button"
			class="flex items-center gap-2 min-w-0 h-10 px-1.5 rounded-md active:bg-surface-raised"
			onclick={() => onTapDots?.()}
			aria-label="Tabs — {serverName ?? 'no server'}, {statusLabel}"
		>
			<span class="w-2 h-2 rounded-full flex-shrink-0 {statusDotColor}"></span>
			<span class="text-[13px] text-text-primary font-medium truncate">
				{serverName ?? 'No server'}
			</span>
			<TabDots {tabs} {activeTabId} />
		</button>
	</div>

	<div class="flex items-center flex-shrink-0">
		<button
			type="button"
			class={btn}
			onclick={() => onOpenPrefs?.()}
			aria-label="Terminal settings"
		>
			<Settings2 class="w-[18px] h-[18px]" />
		</button>
		<button type="button" class={btn} onclick={() => onNewTab?.()} aria-label="New terminal">
			<Plus class="w-5 h-5" />
		</button>
	</div>
</div>
