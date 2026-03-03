<script lang="ts">
	interface Props {
		onSubmit: (command: string) => void;
		onHistoryUp?: () => string | null | undefined;
		onHistoryDown?: () => string;
		disabled?: boolean;
	}

	let { onSubmit, onHistoryUp, onHistoryDown, disabled = false }: Props = $props();

	let value = $state('');

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			if (value.trim()) {
				onSubmit(value.trim());
				value = '';
			}
		} else if (e.key === 'ArrowUp' && onHistoryUp) {
			e.preventDefault();
			const prev = onHistoryUp();
			if (prev !== null && prev !== undefined) value = prev;
		} else if (e.key === 'ArrowDown' && onHistoryDown) {
			e.preventDefault();
			value = onHistoryDown();
		}
	}
</script>

<form class="flex items-center gap-2 px-2 md:px-3 py-1 md:py-1.5 bg-surface-raised border-b border-border" onsubmit={e => { e.preventDefault(); }}>
	<span class="text-text-muted text-sm font-mono">$</span>
	<input
		type="text"
		bind:value
		onkeydown={handleKeydown}
		{disabled}
		class="flex-1 min-w-0 px-2 py-1.5 md:py-1 text-sm font-mono bg-input border border-border rounded-sm text-text-primary placeholder:text-text-muted disabled:opacity-50"
		placeholder="Enter command..."
	/>
	<button
		type="button"
		onclick={() => { if (value.trim()) { onSubmit(value.trim()); value = ''; } }}
		{disabled}
		class="px-3 py-1.5 md:py-1 text-sm bg-primary text-primary-foreground rounded-sm hover:opacity-90 disabled:opacity-50"
	>
		Run
	</button>
</form>
