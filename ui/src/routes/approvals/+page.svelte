<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { ShieldCheck, RefreshCw } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import * as approvalsApi from '#lib/api/approvals.js';
	import type { Approval, ApprovalStatus } from '#lib/api/approvals.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { timeAgo } from '#lib/utils/timeAgo.js';

	let items = $state<Approval[]>([]);
	let loading = $state(true);
	let loadError = $state<string | null>(null);
	/** Ids with a decision in flight */
	let busy = $state<string[]>([]);

	/** The request the link pointed at (`?id=`), shown first and large */
	let focusId = $derived(page.url?.searchParams.get('id') ?? null);
	let focused = $derived(items.find((a) => a.id === focusId) ?? null);
	let others = $derived(items.filter((a) => a.id !== focusId));

	onMount(load);

	async function load() {
		loading = true;
		loadError = null;
		try {
			const res = await approvalsApi.listApprovals();
			items = res.approvals;
			// A link to a request older than the list still opens it
			if (focusId && !items.some((a) => a.id === focusId)) {
				items = [await approvalsApi.getApproval(focusId), ...items];
			}
		} catch (e) {
			loadError = extractErrorMessage(e, 'Could not load approval requests');
		} finally {
			loading = false;
		}
	}

	async function decide(a: Approval, yes: boolean) {
		busy = [...busy, a.id];
		try {
			const updated = yes ? await approvalsApi.approve(a.id) : await approvalsApi.deny(a.id);
			items = items.map((x) => (x.id === a.id ? updated : x));
			if (updated.status === 'done') toast.success(`Approved: ${a.summary}`);
			else if (updated.status === 'denied') toast.success('Denied; nothing was done');
			else
				toast.error(
					approvalsApi.resultMessage(updated) ?? `The call answered ${updated.result_status}`
				);
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Could not record the decision'));
			await load();
		} finally {
			busy = busy.filter((id) => id !== a.id);
		}
	}

	const STATUS: Record<
		ApprovalStatus,
		{ label: string; variant: 'default' | 'success' | 'warning' | 'error' | 'info' }
	> = {
		pending: { label: 'waiting', variant: 'info' },
		expired: { label: 'expired', variant: 'default' },
		denied: { label: 'denied', variant: 'warning' },
		approved: { label: 'running', variant: 'info' },
		done: { label: 'done', variant: 'success' },
		failed: { label: 'failed', variant: 'error' }
	};

	function expiresText(a: Approval): string {
		const min = Math.ceil((new Date(a.expires_at).getTime() - Date.now()) / 60_000);
		return min <= 1 ? 'expires within a minute' : `expires in ${min} min`;
	}
</script>

{#snippet who(a: Approval)}
	token <strong class="text-text-primary font-medium">{a.token_name}</strong>
	{#if a.client_ip}from <span class="font-mono">{a.client_ip}</span>{/if}
	· {timeAgo(a.created_at)}
{/snippet}

{#snippet outcome(a: Approval)}
	{#if a.status === 'failed'}
		<p class="text-xs text-error">
			{approvalsApi.resultMessage(a) ?? `The call answered ${a.result_status}`}
		</p>
	{:else if a.status === 'done'}
		<p class="text-xs text-success">Done {a.decided_at ? timeAgo(a.decided_at) : ''}</p>
	{/if}
{/snippet}

<div class="flex flex-col h-full">
	<div class="flex items-center justify-between gap-2 px-4 md:px-6 py-4 border-b border-border">
		<div class="flex items-center gap-3">
			<ShieldCheck class="w-5 h-5 text-accent" />
			<h1 class="text-lg font-semibold text-text-primary">Approvals</h1>
		</div>
		<Button
			variant="ghost"
			size="sm"
			class="min-w-11 md:min-w-0"
			onclick={load}
			disabled={loading}
			aria-label="Refresh"
		>
			<RefreshCw class="w-4 h-4" />
		</Button>
	</div>

	<div class="flex-1 overflow-y-auto p-4 md:p-6">
		<div class="flex flex-col gap-4 max-w-2xl">
			<p class="text-xs text-text-muted">
				Tokens set to ask first wait here before deleting a server or a credential, or forgetting a
				server's host keys. Only you, signed in, can decide; a request expires after an hour.
			</p>

			{#if loading && items.length === 0}
				<StateBlock kind="loading" title="Loading approval requests…" />
			{:else if loadError}
				<StateBlock kind="error" title={loadError} onRetry={load} />
			{:else}
				{#if focused}
					<section
						class="p-4 bg-surface border rounded-lg {focused.status === 'pending'
							? 'border-accent'
							: 'border-border'}"
						aria-label="Requested action"
					>
						<div class="flex items-start justify-between gap-3 mb-1">
							<h2 class="text-base font-semibold text-text-primary">{focused.summary}</h2>
							<Badge variant={STATUS[focused.status].variant}>{STATUS[focused.status].label}</Badge>
						</div>
						<p class="text-xs text-text-muted mb-3">
							{@render who(focused)}
							{#if focused.status === 'pending'}· {expiresText(focused)}{/if}
						</p>
						{#if focused.status === 'pending'}
							<div class="grid grid-cols-2 gap-2">
								<Button
									variant="secondary"
									size="lg"
									onclick={() => decide(focused!, false)}
									disabled={busy.includes(focused.id)}>Deny</Button
								>
								<Button
									variant="destructive"
									size="lg"
									onclick={() => decide(focused!, true)}
									disabled={busy.includes(focused.id)}>Approve</Button
								>
							</div>
						{:else}
							{@render outcome(focused)}
						{/if}
					</section>
				{:else if focusId}
					<StateBlock kind="empty" icon={ShieldCheck} title="That request was not found" />
				{/if}

				{#if others.length > 0}
					<ul
						class="flex flex-col divide-y divide-border border border-border rounded-md"
						aria-label="Approval requests"
					>
						{#each others as a (a.id)}
							<li class="px-3 py-2">
								<div class="flex items-start gap-3">
									<div class="flex-1 min-w-0">
										<div class="flex items-center gap-1.5 min-w-0">
											<span class="text-sm text-text-primary truncate">{a.summary}</span>
											<Badge variant={STATUS[a.status].variant}>{STATUS[a.status].label}</Badge>
										</div>
										<div class="text-[11px] text-text-muted truncate">{@render who(a)}</div>
										{@render outcome(a)}
									</div>
									{#if a.status === 'pending'}
										<div class="flex gap-1.5 flex-shrink-0">
											<Button
												variant="secondary"
												size="sm"
												onclick={() => decide(a, false)}
												disabled={busy.includes(a.id)}
												aria-label={`Deny: ${a.summary}`}>Deny</Button
											>
											<Button
												variant="destructive"
												size="sm"
												onclick={() => decide(a, true)}
												disabled={busy.includes(a.id)}
												aria-label={`Approve: ${a.summary}`}>Approve</Button
											>
										</div>
									{/if}
								</div>
							</li>
						{/each}
					</ul>
				{:else if !focused}
					<StateBlock
						kind="empty"
						icon={ShieldCheck}
						title="No approval requests"
						hint="Tokens set to ask first show up here when they want to delete something."
					/>
				{/if}
			{/if}
		</div>
	</div>
</div>
