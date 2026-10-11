<script lang="ts">
	/**
	 * Bottom sheet for phones: the tab list (switch, close, open a new one) and the server picker
	 * that a new tab starts from. The platform back gesture closes it.
	 */
	import { X, Plus, ChevronLeft } from '@lucide/svelte';
	import * as serversState from '#lib/state/servers.svelte.js';
	import { serverStatus } from '#lib/utils/serverStatus.js';
	import { watchClose } from '#lib/utils/closeWatcher.js';
	import type { TerminalTab, Server } from '#lib/types/index.js';

	export type SheetView = 'tabs' | 'servers';

	interface Props {
		open: boolean;
		view: SheetView;
		tabs: TerminalTab[];
		activeTabId: string | null;
		onClose: () => void;
		onSelectTab: (id: string) => void;
		onCloseTab: (id: string) => void;
		onPickServer: (server: Server) => void;
		onShowServers: () => void;
		onBackToTabs: () => void;
	}

	let {
		open,
		view,
		tabs,
		activeTabId,
		onClose,
		onSelectTab,
		onCloseTab,
		onPickServer,
		onShowServers,
		onBackToTabs
	}: Props = $props();

	let servers = $derived(serversState.getServers().filter((s) => s.enabled));

	$effect(() => {
		if (open) return watchClose(onClose);
	});

	const dot: Record<string, string> = {
		connected: 'bg-success',
		connecting: 'bg-warning animate-pulse',
		error: 'bg-error',
		disconnected: 'bg-text-muted'
	};
</script>

{#if open}
	<button
		type="button"
		class="fixed inset-0 bg-black/40 z-40"
		onclick={onClose}
		aria-label="Close"
		tabindex="-1"
	></button>
	<div
		class="fixed bottom-0 inset-x-0 z-50 bg-surface border-t border-border rounded-t-xl pb-[env(safe-area-inset-bottom,0px)] max-h-[70dvh] flex flex-col"
		role="dialog"
		aria-modal="true"
		aria-label={view === 'tabs' ? 'Terminals' : 'Connect to a server'}
	>
		<div class="flex justify-center pt-2 pb-1">
			<div class="w-10 h-1 rounded-full bg-text-muted/30"></div>
		</div>
		<div class="flex items-center justify-between px-3 py-1">
			<div class="flex items-center gap-1">
				{#if view === 'servers' && tabs.length > 0}
					<button
						type="button"
						class="w-9 h-9 flex items-center justify-center rounded-md text-text-muted active:bg-surface-raised"
						onclick={onBackToTabs}
						aria-label="Back to terminals"
					>
						<ChevronLeft class="w-5 h-5" />
					</button>
				{/if}
				<h2 class="text-sm font-semibold text-text-primary">
					{view === 'tabs' ? `Terminals (${tabs.length})` : 'Connect to…'}
				</h2>
			</div>
			<button
				type="button"
				class="w-9 h-9 flex items-center justify-center rounded-md text-text-muted active:bg-surface-raised"
				onclick={onClose}
				aria-label="Close"
			>
				<X class="w-5 h-5" />
			</button>
		</div>

		<div class="flex-1 overflow-y-auto px-2 pb-2">
			{#if view === 'tabs'}
				<ul class="flex flex-col gap-1">
					{#each tabs as tab (tab.id)}
						<li class="flex items-center gap-1">
							<button
								type="button"
								class="flex-1 min-w-0 flex items-center gap-3 h-12 px-3 rounded-md text-left {tab.id ===
								activeTabId
									? 'bg-surface-raised text-text-primary'
									: 'text-text-secondary active:bg-surface-raised'}"
								onclick={() => onSelectTab(tab.id)}
								aria-current={tab.id === activeTabId ? 'true' : undefined}
							>
								<span class="w-2 h-2 rounded-full flex-shrink-0 {dot[tab.status]}"></span>
								<span class="flex-1 min-w-0">
									<span class="block text-sm truncate">{tab.label}</span>
									<span class="block text-[11px] text-text-muted truncate">
										{tab.serverName ?? 'no server'} · {tab.status}
									</span>
								</span>
							</button>
							<button
								type="button"
								class="w-11 h-12 flex items-center justify-center rounded-md text-text-muted active:text-error active:bg-surface-raised"
								onclick={() => onCloseTab(tab.id)}
								aria-label="Close {tab.label}"
							>
								<X class="w-4 h-4" />
							</button>
						</li>
					{/each}
				</ul>
				<button
					type="button"
					class="mt-2 w-full h-11 flex items-center justify-center gap-2 rounded-md border border-dashed border-border text-sm text-text-secondary active:bg-surface-raised"
					onclick={onShowServers}
				>
					<Plus class="w-4 h-4" /> New terminal
				</button>
			{:else}
				{#if servers.length === 0}
					<p class="text-sm text-text-muted px-3 py-6 text-center">
						No servers yet — add one under Servers.
					</p>
				{:else}
					<ul class="flex flex-col gap-1">
						{#each servers as server (server.id)}
							{@const st = serverStatus(server)}
							<li>
								<button
									type="button"
									class="w-full flex items-center gap-3 h-12 px-3 rounded-md text-left text-text-secondary active:bg-surface-raised"
									onclick={() => onPickServer(server)}
								>
									<span class="w-2 h-2 rounded-full flex-shrink-0 {st.dot}" title={st.label}></span>
									<span class="flex-1 min-w-0">
										<span class="block text-sm text-text-primary truncate">{server.name}</span>
										<span class="block text-[11px] text-text-muted truncate font-mono">
											{server.is_local
												? 'local'
												: `${server.username ?? 'root'}@${server.hostname ?? '—'}`}
										</span>
									</span>
								</button>
							</li>
						{/each}
					</ul>
				{/if}
			{/if}
		</div>
	</div>
{/if}
