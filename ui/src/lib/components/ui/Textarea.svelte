<script lang="ts">
	import type { HTMLTextareaAttributes } from 'svelte/elements';

	interface Props extends HTMLTextareaAttributes {
		label?: string;
		error?: string;
		hint?: string;
		value?: string;
		mono?: boolean;
	}

	let {
		label,
		error,
		hint,
		value = $bindable(''),
		mono = false,
		class: className = '',
		id,
		...rest
	}: Props = $props();

	const uid = $props.id();
	let areaId = $derived(id ?? `textarea-${uid}`);
</script>

<div class="flex flex-col gap-1">
	{#if label}
		<label for={areaId} class="text-sm text-text-secondary">{label}</label>
	{/if}
	<textarea
		id={areaId}
		bind:value
		class="px-3 py-2 text-base md:text-sm bg-input border rounded-md text-text-primary placeholder:text-text-muted resize-y focus:ring-1 focus:ring-ring focus:outline-none
			{mono ? 'font-mono' : ''} {error ? 'border-error' : 'border-border-strong'} {className}"
		aria-invalid={error ? 'true' : undefined}
		{...rest}></textarea>
	{#if error}
		<p class="text-xs text-error">{error}</p>
	{:else if hint}
		<p class="text-xs text-text-muted">{hint}</p>
	{/if}
</div>
