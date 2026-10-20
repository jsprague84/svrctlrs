<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { Search, ChevronDown, ChevronRight, ScrollText } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import * as auditApi from '#lib/api/audit.js';
	import type { AuditEntry, AuditFilter } from '#lib/api/audit.js';
	import * as usersApi from '#lib/api/users.js';
	import type { UserAccount } from '#lib/api/users.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { timeAgo } from '#lib/utils/timeAgo.js';

	/** One chip per kind of event; `action` is the comma-separated prefix list the API takes */
	const KINDS: Array<{ id: string; label: string; action?: string }> = [
		{ id: 'all', label: 'Everything' },
		{ id: 'logins', label: 'Logins', action: 'login,logout' },
		{ id: 'sessions', label: 'Sessions', action: 'session.,recording.' },
		{ id: 'changes', label: 'Changes', action: 'server.,credential.,user.,token.' }
	];

	let entries = $state<AuditEntry[]>([]);
	let nextBefore = $state<number | null>(null);
	let loading = $state(false);
	let loadError = $state<string | null>(null);
	let kind = $state('all');
	let userId = $state('');
	let query = $state('');
	let users = $state<UserAccount[]>([]);
	const expanded = new SvelteSet<number>();
	let searchTimer: ReturnType<typeof setTimeout> | null = null;

	let filter = $derived.by((): AuditFilter => ({
		action: KINDS.find((k) => k.id === kind)?.action,
		user_id: userId ? Number(userId) : undefined,
		q: query.trim() || undefined
	}));

	onMount(() => {
		load(true);
		usersApi
			.listUsers()
			.then((u) => (users = u))
			.catch(() => {
				// the user filter is optional
			});
	});

	async function load(reset = false) {
		loading = true;
		loadError = null;
		try {
			const page = await auditApi.listAudit(
				100,
				reset ? undefined : (nextBefore ?? undefined),
				filter
			);
			entries = reset ? page.entries : [...entries, ...page.entries];
			nextBefore = page.next_before;
		} catch (e) {
			loadError = extractErrorMessage(e, 'Failed to load the audit log');
		} finally {
			loading = false;
		}
	}

	function setKind(id: string) {
		kind = id;
		load(true);
	}

	function onQueryInput() {
		if (searchTimer) clearTimeout(searchTimer);
		searchTimer = setTimeout(() => load(true), 300);
	}

	function toggle(id: number) {
		if (expanded.has(id)) expanded.delete(id);
		else expanded.add(id);
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

	function targetText(e: AuditEntry): string {
		if (!e.target_type) return '';
		return `${e.target_type} ${e.target_name || `#${e.target_id}`}`;
	}
</script>

<div class="flex flex-col gap-3">
	<p class="text-xs text-text-muted">
		Logins, sessions, commands and changes to servers, credentials, accounts and tokens, kept for
		180 days. Actions taken with an API token name it under the user. Commands are recorded as
		typed; keep secrets out of command lines.
	</p>

	<div class="flex flex-wrap items-center gap-2">
		<div class="flex flex-wrap items-center gap-2 md:gap-1" role="group" aria-label="Kind of event">
			{#each KINDS as k (k.id)}
				<button
					type="button"
					aria-pressed={kind === k.id}
					class="relative before:absolute before:content-[''] md:before:hidden before:-inset-1 min-h-9 md:min-h-0 px-3 md:px-2.5 py-1 text-xs rounded-full border transition-colors {kind ===
					k.id
						? 'bg-accent text-accent-foreground border-accent'
						: 'border-border text-text-secondary hover:text-text-primary'}"
					onclick={() => setKind(k.id)}>{k.label}</button
				>
			{/each}
		</div>
		{#if users.length > 1}
			<select
				bind:value={userId}
				onchange={() => load(true)}
				aria-label="User"
				class="px-2 py-1 min-h-11 md:min-h-0 text-sm md:text-xs bg-input border border-border rounded-sm text-text-primary"
			>
				<option value="">Any user</option>
				{#each users as u (u.id)}
					<option value={String(u.id)}>{u.username}</option>
				{/each}
			</select>
		{/if}
		<div class="relative flex-1 min-w-40">
			<Search class="w-3.5 h-3.5 absolute left-2 top-1/2 -translate-y-1/2 text-text-muted" />
			<input
				type="search"
				bind:value={query}
				oninput={onQueryInput}
				placeholder="Search target, command, address…"
				aria-label="Search the audit log"
				class="w-full pl-7 pr-2 py-1 min-h-11 md:min-h-0 text-base md:text-xs bg-input border border-border rounded-sm text-text-primary"
			/>
		</div>
		<Button variant="secondary" size="sm" onclick={() => load(true)} disabled={loading}
			>Refresh</Button
		>
	</div>

	{#if loadError}
		<StateBlock kind="error" title={loadError} onRetry={() => load(true)} />
	{:else if entries.length === 0 && loading}
		<StateBlock kind="loading" title="Loading the audit log…" />
	{:else if entries.length === 0}
		<StateBlock
			kind="empty"
			icon={ScrollText}
			title={query || kind !== 'all' || userId ? 'Nothing matches these filters' : 'No entries yet'}
		/>
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
						{@const open = expanded.has(e.id)}
						<tr
							class="border-t border-border/40 align-top md:cursor-default cursor-pointer"
							onclick={() => toggle(e.id)}
						>
							<td
								class="py-1.5 pr-3 whitespace-nowrap text-text-secondary"
								title={new Date(e.at).toLocaleString()}
							>
								<span class="md:hidden inline-block w-3 align-middle text-text-muted">
									{#if open}<ChevronDown class="w-3 h-3" />{:else}<ChevronRight
											class="w-3 h-3"
										/>{/if}
								</span>
								{timeAgo(e.at)}
							</td>
							<td class="py-1.5 pr-3 text-text-primary">
								{e.username ?? '—'}
								{#if e.token_name}
									<span class="block text-[11px] text-text-muted" title="API token"
										>via {e.token_name}</span
									>
								{/if}
							</td>
							<td class="py-1.5 pr-3"><Badge variant={variant(e.action)}>{e.action}</Badge></td>
							<td class="py-1.5 pr-3 text-text-primary">{targetText(e) || '—'}</td>
							<td class="py-1.5 pr-3 font-mono text-text-muted hidden md:table-cell"
								>{e.client_ip ?? ''}</td
							>
							<td class="py-1.5 font-mono text-text-muted break-all hidden md:table-cell"
								>{detailText(e)}</td
							>
						</tr>
						{#if open}
							<tr class="md:hidden">
								<td colspan="4" class="pb-2 pl-4 font-mono text-[11px] text-text-muted break-all">
									{new Date(e.at).toLocaleString()}
									{#if e.client_ip}· from {e.client_ip}{/if}
									{#if e.details}<br />{detailText(e)}{/if}
								</td>
							</tr>
						{/if}
					{/each}
				</tbody>
			</table>
		</div>
		{#if nextBefore !== null}
			<div>
				<Button variant="secondary" size="sm" onclick={() => load(false)} disabled={loading}>
					{loading ? 'Loading…' : 'Load more'}
				</Button>
			</div>
		{/if}
	{/if}
</div>
