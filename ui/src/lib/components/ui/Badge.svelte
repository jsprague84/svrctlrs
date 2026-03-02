<script lang="ts">
	import type { Snippet } from 'svelte';

	type Variant = 'default' | 'success' | 'warning' | 'error' | 'info';

	interface Props {
		variant?: Variant;
		children: Snippet;
	}

	let { variant = 'default', children }: Props = $props();

	const variantClasses: Record<Variant, string> = {
		default: 'bg-surface-raised text-text-secondary border-border',
		success: 'bg-success/10 text-success border-success/30',
		warning: 'bg-warning/10 text-warning border-warning/30',
		error: 'bg-error/10 text-error border-error/30',
		info: 'bg-info/10 text-info border-info/30'
	};
</script>

<span class="inline-flex items-center px-1.5 py-0.5 text-[11px] rounded-sm border {variantClasses[variant]}">
	{@render children()}
</span>
