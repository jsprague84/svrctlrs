<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';

	interface Props extends HTMLInputAttributes {
		label?: string;
		error?: string;
		/** Muted helper text under the field (hidden while an error shows) */
		hint?: string;
		value?: string | number;
	}

	let {
		label,
		error,
		hint,
		value = $bindable(''),
		class: className = '',
		id,
		...rest
	}: Props = $props();

	// Stable per instance, and tracks a later change to the `id` prop
	const uid = $props.id();
	let inputId = $derived(id ?? `input-${uid}`);
</script>

<div class="flex flex-col gap-1">
	{#if label}
		<label for={inputId} class="text-sm text-text-secondary">{label}</label>
	{/if}
	<input
		id={inputId}
		bind:value
		class="px-3 py-1.5 min-h-11 md:min-h-0 text-base md:text-sm bg-input border rounded-sm text-text-primary placeholder:text-text-muted
			{error ? 'border-error' : 'border-border'} {className}"
		aria-invalid={error ? 'true' : undefined}
		{...rest}
	/>
	{#if error}
		<p class="text-xs text-error">{error}</p>
	{:else if hint}
		<p class="text-xs text-text-muted">{hint}</p>
	{/if}
</div>
