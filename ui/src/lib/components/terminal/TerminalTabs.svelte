<script lang="ts">
	import { Plus, X, Columns2, Rows2, LayoutGrid, Square } from '@lucide/svelte';
	import type { TerminalTab, LayoutMode } from '#lib/types/index.js';

	interface Props {
		tabs: TerminalTab[];
		activeTabId: string | null;
		layout: LayoutMode;
		onSelectTab: (id: string) => void;
		onCloseTab: (id: string) => void;
		onNewTab: () => void;
		onSetLayout: (mode: LayoutMode) => void;
	}

	let { tabs, activeTabId, layout, onSelectTab, onCloseTab, onNewTab, onSetLayout }: Props =
		$props();

	const statusColors: Record<string, string> = {
		connected: 'bg-success',
		connecting: 'bg-warning animate-pulse',
		disconnected: 'bg-text-muted',
		error: 'bg-error'
	};

	/** WAI-ARIA tabs: arrows move the selection, Home/End jump, Enter/Space select. */
	function handleTabKey(e: KeyboardEvent, tabId: string) {
		const index = tabs.findIndex((t) => t.id === tabId);
		let next: number;
		switch (e.key) {
			case 'ArrowRight':
				next = (index + 1) % tabs.length;
				break;
			case 'ArrowLeft':
				next = (index - 1 + tabs.length) % tabs.length;
				break;
			case 'Home':
				next = 0;
				break;
			case 'End':
				next = tabs.length - 1;
				break;
			case 'Enter':
			case ' ':
				e.preventDefault();
				onSelectTab(tabId);
				return;
			default:
				return;
		}
		e.preventDefault();
		onSelectTab(tabs[next].id);
		const list = (e.currentTarget as HTMLElement).parentElement;
		requestAnimationFrame(() => {
			(list?.querySelector('[role="tab"][aria-selected="true"]') as HTMLElement | null)?.focus();
		});
	}

	const layouts: { mode: LayoutMode; icon: typeof Square; label: string }[] = [
		{ mode: 'single', icon: Square, label: 'Single' },
		{ mode: 'split-h', icon: Columns2, label: 'Split Horizontal' },
		{ mode: 'split-v', icon: Rows2, label: 'Split Vertical' },
		{ mode: 'quad', icon: LayoutGrid, label: 'Quad' }
	];
</script>

<div class="flex items-center bg-surface border-b border-border">
	<!-- Tabs -->
	<div class="flex items-center overflow-x-auto flex-1 min-w-0" role="tablist">
		{#each tabs as tab (tab.id)}
			<div
				class="flex items-center gap-1.5 px-2 py-1 md:px-3 md:py-1.5 text-xs border-r border-border whitespace-nowrap transition-colors cursor-pointer
					{tab.id === activeTabId
					? 'bg-surface-raised text-text-primary'
					: 'text-text-muted hover:text-text-secondary hover:bg-surface-raised/50'}"
				role="tab"
				tabindex={tab.id === activeTabId ? 0 : -1}
				aria-selected={tab.id === activeTabId}
				onclick={() => onSelectTab(tab.id)}
				onkeydown={(e) => handleTabKey(e, tab.id)}
			>
				<span class="w-1.5 h-1.5 rounded-full {statusColors[tab.status] ?? 'bg-text-muted'}"></span>
				<span class="max-w-[60px] md:max-w-[120px] truncate">{tab.label}</span>
				<span class="hidden md:inline text-[10px] uppercase text-text-muted">{tab.mode}</span>
				<button
					class="ml-1 p-1 md:p-0.5 rounded-sm hover:bg-error/20 hover:text-error"
					onclick={(e) => {
						e.stopPropagation();
						onCloseTab(tab.id);
					}}
					title="Close tab"
					aria-label="Close tab"
				>
					<X class="w-3 h-3" />
				</button>
			</div>
		{/each}

		<!-- New Tab -->
		<button
			class="flex items-center gap-1 p-2 md:p-1.5 text-xs text-text-muted hover:text-text-primary hover:bg-surface-raised/50"
			onclick={onNewTab}
			title="New terminal (Ctrl+Shift+T)"
			aria-label="New terminal"
		>
			<Plus class="w-3.5 h-3.5" />
		</button>
	</div>

	<!-- Layout buttons -->
	<div class="hidden md:flex items-center gap-0.5 px-2 border-l border-border">
		{#each layouts as l (l.mode)}
			<button
				class="p-1 rounded-sm transition-colors {layout === l.mode
					? 'bg-accent/20 text-accent'
					: 'text-text-muted hover:text-text-primary hover:bg-surface-raised/50'}"
				onclick={() => onSetLayout(l.mode)}
				title={l.label}
				aria-label={l.label}
				aria-pressed={layout === l.mode}
			>
				<l.icon class="w-3.5 h-3.5" />
			</button>
		{/each}
	</div>
</div>
