<script lang="ts">
	import { X, CheckCircle, AlertCircle, Info, AlertTriangle } from 'lucide-svelte';
	import { getToasts, removeToast, type ToastType } from '$lib/state/toast.svelte.js';

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
	<div class="fixed bottom-4 right-4 z-[100] flex flex-col gap-2 max-w-sm">
		{#each toasts as toast (toast.id)}
			{@const Icon = icons[toast.type]}
			<div class="flex items-start gap-2 px-3 py-2 rounded-md border shadow-lg {colors[toast.type]}">
				<Icon class="w-4 h-4 flex-shrink-0 mt-0.5" />
				<p class="flex-1 text-sm">{toast.message}</p>
				<button class="p-0.5 hover:opacity-70" onclick={() => removeToast(toast.id)}>
					<X class="w-3 h-3" />
				</button>
			</div>
		{/each}
	</div>
{/if}
