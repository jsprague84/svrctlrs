<script lang="ts">
	import { X, CheckCircle, AlertCircle, Info, AlertTriangle } from '@lucide/svelte';
	import { getToasts, removeToast, type ToastType } from '#lib/state/toast.svelte.js';

	const icons: Record<ToastType, typeof CheckCircle> = {
		success: CheckCircle,
		error: AlertCircle,
		info: Info,
		warning: AlertTriangle
	};

	const colors: Record<ToastType, string> = {
		success: 'border-success/40 bg-success/10 text-success',
		error: 'border-error/40 bg-error/10 text-error',
		info: 'border-info/40 bg-info/10 text-info',
		warning: 'border-warning/40 bg-warning/10 text-warning'
	};

	let toasts = $derived(getToasts());
</script>

{#if toasts.length > 0}
	<div
		class="fixed z-[100] flex flex-col gap-2 left-3 right-3 bottom-[calc(var(--toast-offset,env(safe-area-inset-bottom,0px))+0.5rem)] md:left-auto md:right-4 md:bottom-4 md:max-w-sm pointer-events-none"
		role="status"
		aria-live="polite"
	>
		{#each toasts as toast (toast.id)}
			{@const Icon = icons[toast.type]}
			<!-- Opaque under the tint, so a toast over the terminal stays readable -->
			<div class="pointer-events-auto rounded-md bg-surface shadow-lg">
				<div class="flex items-start gap-2 px-3 py-2 rounded-md border {colors[toast.type]}">
					<Icon class="w-4 h-4 flex-shrink-0 mt-0.5" />
					<p class="flex-1 text-sm">{toast.message}</p>
					<button
						class="-my-3.5 -mr-3 w-11 h-11 flex flex-shrink-0 items-center justify-center md:m-0 md:w-auto md:h-auto md:p-0.5 hover:opacity-70"
						onclick={() => removeToast(toast.id)}
						aria-label="Dismiss notification"
					>
						<X class="w-4 h-4 md:w-3 md:h-3" />
					</button>
				</div>
			</div>
		{/each}
	</div>
{/if}
