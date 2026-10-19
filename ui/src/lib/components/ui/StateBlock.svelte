<script lang="ts">
	/**
	 * The one way a list says "loading", "failed" or "nothing here": the same spacing, icon
	 * treatment and Retry everywhere. `compact` is for panels, sheets and the sidebar.
	 */
	import type { Snippet } from 'svelte';
	import { CircleAlert, Inbox, type Pencil } from '@lucide/svelte';
	import Button from './Button.svelte';

	interface Props {
		kind: 'loading' | 'error' | 'empty';
		/** loading: "Loading servers…"; error: what failed; empty: "No servers yet" */
		title: string;
		/** A second line: what to do next */
		hint?: string;
		/** empty only; defaults to an inbox */
		icon?: typeof Pencil;
		/** error only: shows a Retry button */
		onRetry?: () => void;
		compact?: boolean;
		/** Actions under the text (an "Add server" button, a link) */
		children?: Snippet;
	}

	let { kind, title, hint, icon, onRetry, compact = false, children }: Props = $props();
	const Icon = $derived(icon ?? Inbox);
</script>

<div
	class="flex flex-col items-center text-center {compact
		? 'gap-1.5 py-4 px-2'
		: 'gap-2 py-10 px-4'}"
	role={kind === 'error' ? 'alert' : kind === 'loading' ? 'status' : undefined}
>
	{#if kind === 'loading'}
		<span
			class="w-5 h-5 border-2 border-text-muted/40 border-t-accent rounded-full animate-spin"
			aria-hidden="true"
		></span>
		<p class="text-sm text-text-muted">{title}</p>
	{:else if kind === 'error'}
		<CircleAlert class="{compact ? 'w-5 h-5' : 'w-6 h-6'} text-error" aria-hidden="true" />
		<p class="text-sm text-error max-w-md break-words">{title}</p>
		{#if hint}<p class="text-xs text-text-muted max-w-md">{hint}</p>{/if}
		{#if onRetry}
			<Button size="sm" variant="secondary" class="mt-1 min-h-11 md:min-h-0" onclick={onRetry}
				>Retry</Button
			>
		{/if}
	{:else}
		{#if !compact}
			<span
				class="w-11 h-11 flex items-center justify-center rounded-full bg-surface-raised text-text-muted"
				aria-hidden="true"
			>
				<Icon class="w-5 h-5" />
			</span>
		{/if}
		<p class="text-sm text-text-primary font-medium">{title}</p>
		{#if hint}<p class="text-xs text-text-muted max-w-md">{hint}</p>{/if}
	{/if}
	{#if children}
		<div class="mt-2 flex flex-wrap items-center justify-center gap-2">{@render children()}</div>
	{/if}
</div>
