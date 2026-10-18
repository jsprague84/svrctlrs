<script lang="ts">
	/** Desktop side panel with the remote file browser for the active tab's server. */
	import { X } from '@lucide/svelte';
	import { watchClose } from '#lib/utils/closeWatcher.js';
	import FileBrowser from './FileBrowser.svelte';
	import * as files from '#lib/state/files.svelte.js';

	interface Props {
		open: boolean;
		onClose: () => void;
	}

	let { open, onClose }: Props = $props();
	let serverName = $derived(files.getServerName());

	$effect(() => {
		if (open) return watchClose(onClose);
	});
</script>

<div
	class="absolute top-0 bottom-0 right-0 w-[26rem] max-w-full z-20 bg-surface border-l border-border
		flex flex-col transition-transform duration-200 ease-in-out overflow-hidden
		{open ? '' : 'translate-x-full'}"
	role="dialog"
	aria-label="Files"
	aria-hidden={!open}
>
	<div class="flex items-center justify-between px-3 py-2 border-b border-border">
		<span class="text-sm font-semibold text-text-primary truncate">
			Files{serverName ? ` · ${serverName}` : ''}
		</span>
		<button
			class="p-1 text-text-muted hover:text-text-primary"
			onclick={onClose}
			aria-label="Close files"
		>
			<X class="w-4 h-4" />
		</button>
	</div>
	<div class="flex-1 min-h-0">
		{#if open}
			<FileBrowser />
		{/if}
	</div>
</div>
