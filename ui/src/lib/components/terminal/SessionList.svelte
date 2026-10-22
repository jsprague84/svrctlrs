<script lang="ts">
	/**
	 * The user's shells still running on the server, split into those no tab in this window
	 * holds (resume or end them) and those already open here (jump to the tab, or end them).
	 * Shared by the desktop side panel and the phone bottom sheet.
	 */
	import { RotateCw, Play, Square, ArrowRight } from '@lucide/svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import KeepTimeSelect from './KeepTimeSelect.svelte';
	import type { LiveSession } from '#lib/types/index.js';
	import { classifySessions, describeSession } from '#lib/utils/sessions.js';

	interface Props {
		sessions: LiveSession[];
		/** session id → tab id for the sessions tabs in this window hold */
		held: Map<string, string>;
		loading: boolean;
		error: string | null;
		/** Something has been fetched at least once; before that the empty state is not shown */
		loaded: boolean;
		onRefresh: () => void;
		onResume: (session: LiveSession) => void;
		onEnd: (session: LiveSession) => void;
		onGoToTab: (tabId: string) => void;
	}

	let { sessions, held, loading, error, loaded, onRefresh, onResume, onEnd, onGoToTab }: Props =
		$props();

	let groups = $derived(classifySessions(sessions, held));
	/** Re-render ages while the list is open */
	let now = $state(new Date());
	$effect(() => {
		const t = setInterval(() => (now = new Date()), 30_000);
		return () => clearInterval(t);
	});

	const iconBtn =
		'w-11 h-11 md:w-7 md:h-7 flex items-center justify-center rounded-md text-text-muted hover:text-text-primary hover:bg-surface-raised active:bg-surface-raised disabled:opacity-50';
</script>

<div class="flex flex-col gap-3 text-sm" aria-live="polite">
	{#if error}
		<div
			class="flex items-center justify-between gap-2 rounded-md border border-error/40 bg-error/10 px-3 py-2 text-xs text-error"
			role="alert"
		>
			<span class="min-w-0 truncate">{error}</span>
			<button type="button" class="underline flex-shrink-0" onclick={onRefresh}>Retry</button>
		</div>
	{/if}

	{#if !loaded && loading && !error}
		<StateBlock kind="loading" title="Loading sessions…" compact />
	{:else if loaded && sessions.length === 0 && !error}
		<StateBlock
			kind="empty"
			title="No shells running"
			hint="Shells you leave stay here for a while, ready to resume."
			compact
		/>
	{/if}

	{#if groups.resumable.length > 0}
		<section aria-label="Sessions you can resume">
			<h3 class="px-1 pb-1 text-xs font-medium text-text-muted">Not open here</h3>
			<ul class="flex flex-col gap-1">
				{#each groups.resumable as row (row.session.id)}
					<li
						class="flex items-center gap-2 rounded-md border border-border bg-surface-raised/40 px-2 py-1.5"
					>
						<span class="flex-1 min-w-0">
							<span class="block truncate text-text-primary"
								>{row.session.server_name}{#if row.session.recorded}<span
										class="ml-1.5 text-[10px] font-semibold text-error"
										title="Its output is being recorded">● REC</span
									>{/if}{#if row.session.persistent}<span
										class="ml-1.5 text-[10px] font-semibold text-text-muted"
										title="Runs in tmux on the server: it outlives a restart of SvrCtlRS">tmux</span
									>{/if}</span
							>
							<span class="block truncate text-[11px] text-text-muted">
								{describeSession(row, now)}
							</span>
						</span>
						<button
							type="button"
							class="inline-flex items-center gap-1 rounded-sm bg-primary px-2.5 py-1.5 md:py-1 text-xs font-medium text-primary-foreground hover:opacity-90"
							onclick={() => onResume(row.session)}
							title="Open a tab attached to this shell"
						>
							<Play class="w-3.5 h-3.5" /> Resume
						</button>
						<button
							type="button"
							class="{iconBtn} hover:text-error"
							onclick={() => onEnd(row.session)}
							title="End this shell"
							aria-label="End session on {row.session.server_name}"
						>
							<Square class="w-3.5 h-3.5" />
						</button>
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	{#if groups.open.length > 0}
		<section aria-label="Sessions open in this window">
			<h3 class="px-1 pb-1 text-xs font-medium text-text-muted">Open in this window</h3>
			<ul class="flex flex-col gap-1">
				{#each groups.open as row (row.session.id)}
					<li class="flex items-center gap-2 rounded-md px-2 py-1.5">
						<span class="flex-1 min-w-0">
							<span class="block truncate text-text-secondary"
								>{row.session.server_name}{#if row.session.recorded}<span
										class="ml-1.5 text-[10px] font-semibold text-error"
										title="Its output is being recorded">● REC</span
									>{/if}{#if row.session.persistent}<span
										class="ml-1.5 text-[10px] font-semibold text-text-muted"
										title="Runs in tmux on the server: it outlives a restart of SvrCtlRS">tmux</span
									>{/if}</span
							>
							<span class="block truncate text-[11px] text-text-muted">
								{describeSession(row, now)}
							</span>
						</span>
						<button
							type="button"
							class={iconBtn}
							onclick={() => row.tabId && onGoToTab(row.tabId)}
							title="Go to its tab"
							aria-label="Go to the tab for {row.session.server_name}"
						>
							<ArrowRight class="w-4 h-4" />
						</button>
						<button
							type="button"
							class="{iconBtn} hover:text-error"
							onclick={() => onEnd(row.session)}
							title="End this shell"
							aria-label="End session on {row.session.server_name}"
						>
							<Square class="w-3.5 h-3.5" />
						</button>
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	<div class="flex items-center justify-between gap-2 px-1 text-[11px] text-text-muted">
		<span class="min-w-0">
			<KeepTimeSelect />
		</span>
		<button
			type="button"
			class="inline-flex items-center gap-1 flex-shrink-0 rounded-sm px-1.5 py-1 hover:text-text-primary disabled:opacity-50"
			onclick={onRefresh}
			disabled={loading}
			aria-label="Refresh sessions"
		>
			<RotateCw class="w-3 h-3 {loading ? 'animate-spin' : ''}" /> Refresh
		</button>
	</div>
</div>
