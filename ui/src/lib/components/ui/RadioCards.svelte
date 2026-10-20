<script lang="ts">
	/**
	 * A radio group drawn as tappable cards: stacked (with a line of detail each) or in one row
	 * (a segmented control). Arrow keys, Home and End move the choice, as in a native group.
	 */
	interface Option {
		value: string;
		label: string;
		detail?: string;
	}

	interface Props {
		label: string;
		options: Option[];
		value?: string;
		layout?: 'stack' | 'row';
	}

	let { label, options, value = $bindable(''), layout = 'stack' }: Props = $props();

	const uid = $props.id();
	let buttons: HTMLButtonElement[] = $state([]);
	let focusIndex = $derived(
		Math.max(
			0,
			options.findIndex((o) => o.value === value)
		)
	);

	function onKeydown(e: KeyboardEvent, i: number) {
		const n = options.length;
		let next: number;
		if (e.key === 'ArrowDown' || e.key === 'ArrowRight') next = (i + 1) % n;
		else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') next = (i - 1 + n) % n;
		else if (e.key === 'Home') next = 0;
		else if (e.key === 'End') next = n - 1;
		else return;
		e.preventDefault();
		value = options[next].value;
		buttons[next]?.focus();
	}
</script>

<div>
	<p id="radio-cards-{uid}" class="text-sm text-text-secondary mb-1.5">{label}</p>
	<div
		role="radiogroup"
		aria-labelledby="radio-cards-{uid}"
		class={layout === 'row' ? 'grid gap-1' : 'flex flex-col gap-2'}
		style={layout === 'row'
			? `grid-template-columns: repeat(${options.length}, minmax(0, 1fr))`
			: undefined}
	>
		{#each options as o, i (o.value)}
			{@const on = value === o.value}
			<button
				bind:this={buttons[i]}
				type="button"
				role="radio"
				aria-checked={on}
				tabindex={i === focusIndex ? 0 : -1}
				class="min-h-11 border rounded-sm transition-colors focus-visible:outline-2 focus-visible:outline-accent {layout ===
				'row'
					? 'px-1 text-sm text-center md:min-h-9'
					: 'px-3 py-2 text-left'} {on
					? 'border-accent bg-accent/10 text-text-primary'
					: 'border-border text-text-secondary hover:border-text-muted'}"
				onclick={() => (value = o.value)}
				onkeydown={(e) => onKeydown(e, i)}
			>
				{#if layout === 'row'}
					{o.label}
				{:else}
					<span class="flex items-start gap-2.5">
						<span
							class="mt-1 w-3.5 h-3.5 flex-shrink-0 rounded-full border {on
								? 'border-accent border-4'
								: 'border-text-muted'}"
							aria-hidden="true"
						></span>
						<span>
							<span class="block text-sm text-text-primary">{o.label}</span>
							{#if o.detail}
								<span class="block text-xs text-text-muted">{o.detail}</span>
							{/if}
						</span>
					</span>
				{/if}
			</button>
		{/each}
	</div>
</div>
