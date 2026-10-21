<script lang="ts">
	/** Desktop side panel listing the server-side sessions (see `SessionList`). */
	import { X } from '@lucide/svelte';
	import { watchClose } from '#lib/utils/closeWatcher.js';
	import { overlay } from '#lib/utils/overlay.js';
	import SessionList from './SessionList.svelte';
	import type { LiveSession } from '#lib/types/index.js';

	interface Props {
		open: boolean;
		sessions: LiveSession[];
		held: Map<string, string>;
		loading: boolean;
		error: string | null;
		loaded: boolean;
		onClose: () => void;
		onRefresh: () => void;
		onResume: (session: LiveSession) => void;
		onEnd: (session: LiveSession) => void;
		onGoToTab: (tabId: string) => void;
	}

	let {
		open,
		sessions,
		held,
		loading,
		error,
		loaded,
		onClose,
		onRefresh,
		onResume,
		onEnd,
		onGoToTab
	}: Props = $props();

	$effect(() => {
		if (open) return watchClose(onClose);
	});
</script>

<div
	class="absolute top-0 bottom-0 right-0 w-80 max-w-full z-20 bg-surface border-l border-border
		flex flex-col transition-transform duration-200 ease-in-out overflow-hidden
		{open ? '' : 'translate-x-full'}"
	role="dialog"
	aria-label="Sessions"
	aria-hidden={!open}
	inert={!open}
	{@attach open ? overlay({ onEscape: () => onClose(), trap: false }) : null}
>
	<div class="flex items-center justify-between px-3 py-2 border-b border-border">
		<span class="text-sm font-semibold text-text-primary">Sessions</span>
		<button
			class="p-1 text-text-muted hover:text-text-primary"
			onclick={onClose}
			aria-label="Close sessions"
		>
			<X class="w-4 h-4" />
		</button>
	</div>
	<div class="flex-1 overflow-y-auto p-3">
		{#if open}
			<SessionList
				{sessions}
				{held}
				{loading}
				{error}
				{loaded}
				{onRefresh}
				{onResume}
				{onEnd}
				{onGoToTab}
			/>
		{/if}
	</div>
</div>
