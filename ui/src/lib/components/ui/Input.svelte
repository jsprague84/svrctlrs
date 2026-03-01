<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';

	interface Props extends HTMLInputAttributes {
		label?: string;
		error?: string;
		value?: string | number;
	}

	let { label, error, value = $bindable(''), class: className = '', id, ...rest }: Props = $props();

	const inputId = id ?? `input-${Math.random().toString(36).slice(2, 8)}`;
</script>

<div class="flex flex-col gap-1">
	{#if label}
		<label for={inputId} class="text-sm text-text-secondary">{label}</label>
	{/if}
	<input
		id={inputId}
		bind:value
		class="px-3 py-1.5 text-sm bg-input border rounded-sm text-text-primary placeholder:text-text-muted
			{error ? 'border-error' : 'border-border'} {className}"
		{...rest}
	/>
	{#if error}
		<p class="text-xs text-error">{error}</p>
	{/if}
</div>
