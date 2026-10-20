<script lang="ts">
	import { CircleCheck, CircleX, CircleMinus, RefreshCw } from '@lucide/svelte';
	import Modal from '#lib/components/ui/Modal.svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import * as serversApi from '#lib/api/servers.js';
	import type { Diagnosis } from '#lib/api/servers.js';
	import { extractErrorMessage } from '#lib/utils/error.js';

	interface Props {
		/** The server to diagnose; null closes the dialog */
		server: { id: number; name: string } | null;
		onClose: () => void;
		/** After each run, so the page can refresh the server's status */
		onDone?: () => void;
	}

	let { server, onClose, onDone }: Props = $props();

	let report = $state<Diagnosis | null>(null);
	let running = $state(false);
	let error = $state<string | null>(null);

	const LABELS: Record<string, string> = {
		config: 'Configuration',
		dns: 'Name lookup',
		tcp: 'TCP connection',
		jump_host: 'Jump host',
		tunnel: 'Tunnel from the jump host',
		handshake: 'SSH handshake',
		host_key: 'Host key',
		login: 'Login'
	};

	async function run() {
		if (!server) return;
		running = true;
		error = null;
		try {
			report = await serversApi.diagnose(server.id);
			onDone?.();
		} catch (e) {
			error = extractErrorMessage(e, 'The diagnosis could not run');
		} finally {
			running = false;
		}
	}

	// A new server opens with a fresh run
	let lastId: number | null = null;
	$effect(() => {
		const id = server?.id ?? null;
		if (id !== lastId) {
			lastId = id;
			report = null;
			if (id !== null) run();
		}
	});
</script>

<Modal open={server !== null} title={`Diagnose ${server?.name ?? ''}`} {onClose}>
	{#if running && !report}
		<StateBlock kind="loading" title="Connecting step by step…" compact />
	{:else if error}
		<StateBlock kind="error" title={error} onRetry={run} compact />
	{:else if report}
		<p
			class="text-sm mb-3 px-3 py-2 rounded-sm border [overflow-wrap:anywhere] {report.ok
				? 'border-success/40 bg-success/10 text-success'
				: 'border-error/40 bg-error/10 text-error'}"
			role="status"
		>
			{report.verdict}
		</p>
		<div class="flex flex-col gap-4">
			{#each report.hops as hop (hop.server_id)}
				<section aria-label={`Steps for ${hop.server_name}`}>
					<h3 class="text-xs font-semibold text-text-secondary mb-1.5">
						{hop.server_name}
						<span class="font-mono font-normal text-text-muted break-all">{hop.address}</span>
					</h3>
					<ol class="flex flex-col gap-1.5">
						{#each hop.steps as s, i (i)}
							<li class="flex gap-2">
								{#if s.status === 'ok'}
									<CircleCheck
										class="w-4 h-4 mt-0.5 flex-shrink-0 text-success"
										aria-label="passed"
									/>
								{:else if s.status === 'failed'}
									<CircleX class="w-4 h-4 mt-0.5 flex-shrink-0 text-error" aria-label="failed" />
								{:else}
									<CircleMinus
										class="w-4 h-4 mt-0.5 flex-shrink-0 text-text-muted"
										aria-label="skipped"
									/>
								{/if}
								<div class="min-w-0 flex-1">
									<div class="text-sm text-text-primary">
										{LABELS[s.step] ?? s.step}
										{#if s.status !== 'skipped'}<span class="text-[11px] text-text-muted"
												>{s.duration_ms} ms</span
											>{/if}
									</div>
									<div class="text-xs text-text-muted [overflow-wrap:anywhere]">{s.detail}</div>
									{#if s.fix}
										<div
											class="mt-1 text-xs text-text-secondary bg-surface-raised border border-border rounded-sm px-2 py-1.5 [overflow-wrap:anywhere]"
										>
											<strong class="text-text-primary">Fix:</strong>
											{s.fix}
										</div>
									{/if}
								</div>
							</li>
						{/each}
					</ol>
				</section>
			{/each}
		</div>
	{/if}
	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>Close</Button>
		<Button onclick={run} disabled={running || !server}>
			<RefreshCw class="w-4 h-4 {running ? 'animate-spin' : ''}" /> Run again
		</Button>
	{/snippet}
</Modal>
