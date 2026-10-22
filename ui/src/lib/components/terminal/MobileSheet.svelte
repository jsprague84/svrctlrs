<script lang="ts">
	/**
	 * Bottom sheet for phones: the tab list (switch, close, open a new one), the server picker
	 * that a new tab starts from, and the shells still running on the server (resume or end).
	 * The platform back gesture closes it.
	 */
	import { X, Plus, ChevronLeft, History, FolderOpen } from '@lucide/svelte';
	import * as serversState from '#lib/state/servers.svelte.js';
	import { serverStatus } from '#lib/utils/serverStatus.js';
	import { allTags, matchesTag } from '#lib/utils/tags.js';
	import TagChips from '#lib/components/servers/TagChips.svelte';
	import { watchClose } from '#lib/utils/closeWatcher.js';
	import { resolve } from '$app/paths';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import { overlay } from '#lib/utils/overlay.js';
	import SessionList from './SessionList.svelte';
	import FileBrowser from './FileBrowser.svelte';
	import type { TerminalTab, Server, LiveSession } from '#lib/types/index.js';

	export type SheetView = 'tabs' | 'servers' | 'sessions' | 'files';

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
		onShowSessions: () => void;
		/** Opens the file browser for the active tab's server (the view then handles the sheet) */
		onShowFiles: () => void;
		filesAvailable: boolean;
		onBackToTabs: () => void;
		/** Server-side sessions, for the `sessions` view and the count on its entry */
		sessions: LiveSession[];
		held: Map<string, string>;
		sessionsLoading: boolean;
		sessionsError: string | null;
		sessionsLoaded: boolean;
		onRefreshSessions: () => void;
		onResumeSession: (session: LiveSession) => void;
		onEndSession: (session: LiveSession) => void;
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
		onShowSessions,
		onShowFiles,
		filesAvailable,
		onBackToTabs,
		sessions,
		held,
		sessionsLoading,
		sessionsError,
		sessionsLoaded,
		onRefreshSessions,
		onResumeSession,
		onEndSession
	}: Props = $props();

	let allServers = $derived(serversState.getServers().filter((s) => s.enabled));
	let tags = $derived(allTags(allServers));
	let tagFilter = $state<string | null>(null);
	let servers = $derived(allServers.filter((s) => matchesTag(s, tagFilter)));
	/** Shells no tab here holds — the ones worth a badge */
	let resumableCount = $derived(sessions.filter((s) => !held.has(s.id)).length);
	const titles: Record<SheetView, (n: number) => string> = {
		tabs: (n) => `Terminals (${n})`,
		servers: () => 'Connect to…',
		sessions: () => 'Sessions',
		files: () => 'Files'
	};

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
		class="fixed inset-0 bg-scrim z-40"
		onclick={onClose}
		aria-label="Close"
		tabindex="-1"
	></button>
	<div
		class="fixed bottom-0 inset-x-0 z-50 bg-surface border-t border-border rounded-t-xl pb-[env(safe-area-inset-bottom,0px)] max-h-[70dvh] flex flex-col"
		role="dialog"
		aria-modal="true"
		{@attach overlay({ onEscape: () => onClose() })}
		aria-label={view === 'tabs'
			? 'Terminals'
			: view === 'servers'
				? 'Connect to a server'
				: view === 'files'
					? 'Files on the server'
					: 'Sessions on the server'}
	>
		<div class="flex justify-center pt-2 pb-1">
			<div class="w-10 h-1 rounded-full bg-text-muted/30"></div>
		</div>
		<div class="flex items-center justify-between px-3 py-1">
			<div class="flex items-center gap-1">
				{#if view !== 'tabs' && tabs.length > 0}
					<button
						type="button"
						class="w-11 h-11 flex items-center justify-center rounded-md text-text-muted active:bg-surface-raised"
						onclick={onBackToTabs}
						aria-label="Back to terminals"
					>
						<ChevronLeft class="w-5 h-5" />
					</button>
				{/if}
				<h2 class="text-sm font-semibold text-text-primary">
					{titles[view](tabs.length)}
				</h2>
			</div>
			<button
				type="button"
				class="w-11 h-11 flex items-center justify-center rounded-md text-text-muted active:bg-surface-raised"
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
				<button
					type="button"
					class="mt-1 w-full h-11 flex items-center justify-center gap-2 rounded-md text-sm text-text-secondary active:bg-surface-raised disabled:opacity-40"
					onclick={onShowFiles}
					disabled={!filesAvailable}
				>
					<FolderOpen class="w-4 h-4" /> Files on this server
				</button>
				<!-- Shells still running on the server; a lost tab is recovered from here -->
				<button
					type="button"
					class="mt-1 w-full h-11 flex items-center justify-center gap-2 rounded-md text-sm text-text-secondary active:bg-surface-raised"
					onclick={onShowSessions}
				>
					<History class="w-4 h-4" /> Sessions on the server
					{#if resumableCount > 0}
						<span
							class="ml-1 min-w-5 px-1.5 py-0.5 rounded-full bg-accent/20 text-accent text-[11px] font-medium"
							aria-label="{resumableCount} to resume"
						>
							{resumableCount}
						</span>
					{/if}
				</button>
			{:else if view === 'files'}
				<div class="h-[55dvh]">
					<FileBrowser />
				</div>
			{:else if view === 'sessions'}
				<div class="px-1 py-1">
					<SessionList
						{sessions}
						{held}
						loading={sessionsLoading}
						error={sessionsError}
						loaded={sessionsLoaded}
						onRefresh={onRefreshSessions}
						onResume={onResumeSession}
						onEnd={onEndSession}
						onGoToTab={onSelectTab}
					/>
				</div>
			{:else}
				{#if tags.length > 0}
					<div class="px-3 pb-2">
						<TagChips {tags} selected={tagFilter} onSelect={(t) => (tagFilter = t)} />
					</div>
				{/if}
				{#if allServers.length === 0}
					<StateBlock kind="empty" title="No servers yet" compact>
						<a
							href={resolve('/servers')}
							class="text-sm text-accent underline min-h-11 inline-flex items-center"
							>Add one on the Servers page</a
						>
					</StateBlock>
				{:else if servers.length === 0}
					<StateBlock kind="empty" title="No server has that tag" compact />
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
											{server.username ?? 'root'}@{server.hostname}
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
