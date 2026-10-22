<script lang="ts">
	import type { ConnectionStatus } from '#lib/types/index.js';

	interface Props {
		status: ConnectionStatus;
	}

	let { status }: Props = $props();

	const config: Record<
		ConnectionStatus,
		{ color: string; bg: string; border: string; label: string }
	> = {
		connected: {
			color: 'text-success',
			bg: 'bg-success/10',
			border: 'border-success/30',
			label: 'Connected'
		},
		connecting: {
			color: 'text-warning',
			bg: 'bg-warning/10',
			border: 'border-warning/30',
			label: 'Connecting...'
		},
		disconnected: {
			color: 'text-text-secondary',
			bg: 'bg-surface',
			border: 'border-border-strong/60',
			label: 'Disconnected'
		},
		error: { color: 'text-error', bg: 'bg-error/10', border: 'border-error/30', label: 'Error' }
	};

	let c = $derived(config[status]);
</script>

<span
	class="inline-flex items-center gap-1.5 px-2 py-0.5 text-xs font-medium rounded-full border whitespace-nowrap {c.border} {c.color} {c.bg}"
>
	<span
		class="w-1.5 h-1.5 rounded-full {status === 'disconnected'
			? 'border border-current'
			: c.color.replace('text-', 'bg-')}"
		class:animate-pulse={status === 'connecting'}
	></span>
	{c.label}
</span>
