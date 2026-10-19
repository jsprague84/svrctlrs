<script lang="ts">
	import type { HTMLSelectAttributes } from 'svelte/elements';
	import type { Snippet } from 'svelte';

	interface Props extends HTMLSelectAttributes {
		label?: string;
		error?: string;
		value?: string;
		children: Snippet;
	}

	let {
		label,
		error,
		value = $bindable(''),
		children,
		class: className = '',
		id,
		...rest
	}: Props = $props();

	const uid = $props.id();
	let selectId = $derived(id ?? `select-${uid}`);
</script>

<div class="flex flex-col gap-1">
	{#if label}
		<label for={selectId} class="text-sm text-text-secondary">{label}</label>
	{/if}
	<select
		id={selectId}
		bind:value
		class="px-3 py-1.5 min-h-11 md:min-h-0 text-base md:text-sm bg-input border rounded-sm text-text-primary
			{error ? 'border-error' : 'border-border'} {className}"
		{...rest}
	>
		{@render children()}
	</select>
	{#if error}
		<p class="text-xs text-error">{error}</p>
	{/if}
</div>
