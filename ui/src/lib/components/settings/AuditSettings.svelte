<script lang="ts">
	import { onMount } from 'svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import * as auditApi from '#lib/api/audit.js';
	import type { AuditEntry } from '#lib/api/audit.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';

	let entries = $state<AuditEntry[]>([]);
	let nextBefore = $state<number | null>(null);
	let loading = $state(false);

	onMount(() => load(true));

	async function load(reset = false) {
		loading = true;
		try {
			const page = await auditApi.listAudit(100, reset ? undefined : (nextBefore ?? undefined));
			entries = reset ? page.entries : [...entries, ...page.entries];
			nextBefore = page.next_before;
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to load the audit log'));
		} finally {
			loading = false;
		}
	}

	function variant(action: string): 'info' | 'success' | 'warning' | 'default' {
		if (action.startsWith('login.')) return 'warning';
		if (action === 'login' || action.startsWith('session.')) return 'success';
		if (action.endsWith('.deleted') || action.endsWith('forgotten')) return 'warning';
		return 'info';
	}

	function detailText(entry: AuditEntry): string {
		if (!entry.details) return '';
		try {
			const d = JSON.parse(entry.details) as Record<string, unknown>;
			return Object.entries(d)
				.map(([k, v]) => `${k}=${typeof v === 'string' ? v : JSON.stringify(v)}`)
				.join('  ');
		} catch {
			return entry.details;
		}
	}
</script>

<div class="flex flex-col gap-3">
	<div class="flex items-center justify-between">
		<p class="text-xs text-text-muted">
			Logins, sessions, commands and changes to servers, credentials and accounts. Entries are kept
			for 180 days. Commands are recorded as typed; keep secrets out of command lines.
		</p>
		<Button variant="secondary" size="sm" onclick={() => load(true)} disabled={loading}
			>Refresh</Button
		>
	</div>

	{#if entries.length === 0 && !loading}
		<div class="text-text-muted text-sm">No entries yet.</div>
	{:else}
		<div class="overflow-x-auto">
			<table class="w-full text-xs">
				<thead class="text-text-muted text-left">
					<tr>
						<th class="py-1 pr-3 font-medium">When</th>
						<th class="py-1 pr-3 font-medium">Who</th>
						<th class="py-1 pr-3 font-medium">Action</th>
						<th class="py-1 pr-3 font-medium">Target</th>
						<th class="py-1 pr-3 font-medium hidden md:table-cell">From</th>
						<th class="py-1 font-medium hidden md:table-cell">Details</th>
					</tr>
				</thead>
				<tbody>
					{#each entries as e (e.id)}
						<tr class="border-t border-border/40 align-top">
							<td class="py-1.5 pr-3 whitespace-nowrap text-text-secondary"
								>{new Date(e.at).toLocaleString()}</td
							>
							<td class="py-1.5 pr-3 text-text-primary">{e.username ?? '—'}</td>
							<td class="py-1.5 pr-3"><Badge variant={variant(e.action)}>{e.action}</Badge></td>
							<td class="py-1.5 pr-3 text-text-primary">
								{#if e.target_type}{e.target_type} {e.target_name || `#${e.target_id}`}{:else}—{/if}
							</td>
							<td class="py-1.5 pr-3 font-mono text-text-muted hidden md:table-cell"
								>{e.client_ip ?? ''}</td
							>
							<td class="py-1.5 font-mono text-text-muted break-all hidden md:table-cell"
								>{detailText(e)}</td
							>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		{#if nextBefore !== null}
			<div>
				<Button variant="secondary" size="sm" onclick={() => load(false)} disabled={loading}
					>Load more</Button
				>
			</div>
		{/if}
	{/if}
</div>
