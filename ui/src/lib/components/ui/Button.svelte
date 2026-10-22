<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	type Variant = 'primary' | 'secondary' | 'destructive' | 'ghost';
	type Size = 'sm' | 'md' | 'lg';

	interface Props extends HTMLButtonAttributes {
		variant?: Variant;
		size?: Size;
		children: Snippet;
	}

	let {
		variant = 'primary',
		size = 'md',
		children,
		class: className = '',
		...rest
	}: Props = $props();

	const variantClasses: Record<Variant, string> = {
		// One filled button per area; secondary is outlined; ghost for row and toolbar actions
		primary: 'bg-primary text-primary-foreground border-transparent hover:bg-primary/90',
		secondary:
			'bg-secondary text-secondary-foreground border-border hover:bg-secondary-hover shadow-xs',
		destructive:
			'bg-destructive text-destructive-foreground border-transparent hover:bg-destructive/90',
		ghost: 'text-text-secondary border-transparent hover:bg-surface-raised hover:text-text-primary'
	};

	const sizeClasses: Record<Size, string> = {
		sm: 'px-2 py-1 text-xs',
		md: 'px-3 py-1.5 text-sm',
		lg: 'px-4 py-2 text-sm'
	};
</script>

<button
	class="inline-flex items-center justify-center gap-1.5 min-h-11 md:min-h-0 rounded-md border font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:opacity-50 disabled:cursor-not-allowed {variantClasses[
		variant
	]} {sizeClasses[size]} {className}"
	{...rest}
>
	{@render children()}
</button>
