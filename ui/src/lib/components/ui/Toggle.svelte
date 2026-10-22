<script lang="ts">
	interface Props {
		checked: boolean;
		/** Accessible name; shown next to the switch unless `labelHidden` */
		label: string;
		description?: string;
		labelHidden?: boolean;
		disabled?: boolean;
		onchange?: (checked: boolean) => void;
	}

	let {
		checked = $bindable(false),
		label,
		description,
		labelHidden = false,
		disabled = false,
		onchange
	}: Props = $props();

	const uid = $props.id();
	let descId = $derived(description ? `toggle-desc-${uid}` : undefined);

	function toggle() {
		if (disabled) return;
		checked = !checked;
		onchange?.(checked);
	}
</script>

<div class="flex items-center justify-between gap-4 py-2">
	{#if !labelHidden}
		<div class="min-w-0">
			<p class="text-sm text-text-primary">{label}</p>
			{#if description}
				<p id={descId} class="text-xs text-text-muted">{description}</p>
			{/if}
		</div>
	{/if}
	<button
		type="button"
		role="switch"
		aria-checked={checked}
		aria-label={labelHidden ? label : undefined}
		aria-describedby={descId}
		{disabled}
		onclick={toggle}
		class="relative inline-flex h-5 w-9 flex-shrink-0 items-center rounded-full transition-colors before:absolute before:-inset-3 before:content-[''] md:before:hidden focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:opacity-50 {checked
			? 'bg-success'
			: 'bg-border-strong/55'}"
	>
		<span
			class="inline-block h-4 w-4 rounded-full bg-white shadow transition-transform {checked
				? 'translate-x-4'
				: 'translate-x-0.5'}"
		></span>
	</button>
</div>
