<script lang="ts">
	import { onMount } from 'svelte';
	import { Copy, KeyRound, Plus, TriangleAlert } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import Input from '#lib/components/ui/Input.svelte';
	import Modal from '#lib/components/ui/Modal.svelte';
	import RadioCards from '#lib/components/ui/RadioCards.svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import SettingsSection from '#lib/components/ui/SettingsSection.svelte';
	import * as tokensApi from '#lib/api/tokens.js';
	import type { ApiToken, CreatedToken, TokenPreset } from '#lib/api/tokens.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { timeAgo } from '#lib/utils/timeAgo.js';

	let tokens = $state<ApiToken[]>([]);
	let loading = $state(true);
	let loadError = $state<string | null>(null);
	/** Ids with a revoke or remove in flight */
	let busy = $state<number[]>([]);

	// Create dialog
	let dialogOpen = $state(false);
	let name = $state('');
	let preset = $state<string>('servers_keys');
	let lifetime = $state('90');
	let creating = $state(false);
	let created = $state<CreatedToken | null>(null);

	let lifetimeDays = $derived(lifetime === 'never' ? null : Number(lifetime));
	let canCreate = $derived(!creating && name.trim().length > 0 && name.trim().length <= 64);

	onMount(load);

	async function load() {
		loading = true;
		loadError = null;
		try {
			tokens = await tokensApi.listTokens();
		} catch (e) {
			loadError = extractErrorMessage(e, 'Failed to load tokens');
		} finally {
			loading = false;
		}
	}

	function openDialog() {
		name = '';
		preset = 'servers_keys';
		lifetime = '90';
		created = null;
		dialogOpen = true;
	}

	function closeDialog() {
		dialogOpen = false;
		created = null;
	}

	async function create() {
		if (!canCreate) return;
		creating = true;
		try {
			created = await tokensApi.createToken(name.trim(), preset as TokenPreset, lifetimeDays);
			tokens = [created.token, ...tokens];
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to create the token'));
		} finally {
			creating = false;
		}
	}

	async function copySecret() {
		if (!created) return;
		try {
			await navigator.clipboard.writeText(created.secret);
			toast.success('Token copied');
		} catch {
			toast.error('Clipboard unavailable; select and copy the token');
		}
	}

	async function revoke(t: ApiToken) {
		busy = [...busy, t.id];
		try {
			const updated = await tokensApi.revokeToken(t.id);
			tokens = tokens.map((x) => (x.id === t.id ? updated : x));
			toast.success(`Token ${t.name} revoked`);
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to revoke the token'));
		} finally {
			busy = busy.filter((id) => id !== t.id);
		}
	}

	async function remove(t: ApiToken) {
		busy = [...busy, t.id];
		try {
			await tokensApi.deleteToken(t.id);
			tokens = tokens.filter((x) => x.id !== t.id);
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to remove the token'));
		} finally {
			busy = busy.filter((id) => id !== t.id);
		}
	}

	function usedText(t: ApiToken): string {
		return t.last_used_at ? `used ${timeAgo(t.last_used_at)}` : 'never used';
	}

	function expiryText(t: ApiToken): string {
		if (t.status === 'revoked' && t.revoked_at) return `revoked ${timeAgo(t.revoked_at)}`;
		if (!t.expires_at) return 'never expires';
		const at = new Date(t.expires_at);
		if (t.status === 'expired') return `expired ${at.toLocaleDateString()}`;
		const days = Math.ceil((at.getTime() - Date.now()) / 86_400_000);
		return days <= 1 ? 'expires within a day' : `expires in ${days} d`;
	}
</script>

{#snippet statusBadge(t: ApiToken)}
	{#if t.status === 'revoked'}
		<Badge variant="error">revoked</Badge>
	{:else if t.status === 'expired'}
		<Badge variant="warning">expired</Badge>
	{/if}
{/snippet}

{#snippet action(t: ApiToken)}
	{#if t.status === 'active'}
		<Button
			variant="secondary"
			size="sm"
			class="min-w-20"
			onclick={() => revoke(t)}
			disabled={busy.includes(t.id)}
			aria-label={`Revoke token ${t.name}`}>Revoke</Button
		>
	{:else}
		<Button
			variant="ghost"
			size="sm"
			class="min-w-20"
			onclick={() => remove(t)}
			disabled={busy.includes(t.id)}
			aria-label={`Remove token ${t.name}`}>Remove</Button
		>
	{/if}
{/snippet}

<SettingsSection
	title="Agent tokens"
	description="For agents and scripts (svrctl, Hermes, Claude Code), sent as Authorization: Bearer. A token acts as you within its access, and the audit log names it on everything it does."
	card
>
	{#snippet actions()}
		<Button size="sm" onclick={openDialog}>
			<Plus class="w-3.5 h-3.5" /> New token
		</Button>
	{/snippet}

	{#if loading}
		<StateBlock kind="loading" title="Loading tokens…" compact />
	{:else if loadError}
		<StateBlock kind="error" title={loadError} onRetry={load} compact />
	{:else if tokens.length === 0}
		<StateBlock
			kind="empty"
			icon={KeyRound}
			title="No tokens yet"
			hint="Create one for each agent, so each can be revoked on its own."
			compact
		/>
	{:else}
		<!-- Phones: one card per token -->
		<ul class="md:hidden flex flex-col gap-2">
			{#each tokens as t (t.id)}
				<li
					class="flex items-center gap-3 border border-border/50 rounded-sm px-3 py-2 {t.status !==
					'active'
						? 'opacity-70'
						: ''}"
				>
					<div class="flex-1 min-w-0">
						<div class="flex items-center gap-1.5 min-w-0">
							<span class="text-sm text-text-primary font-medium truncate">{t.name}</span>
							{@render statusBadge(t)}
						</div>
						<div class="text-[11px] text-text-muted truncate">
							{tokensApi.presetLabel(t.preset)} · <span class="font-mono">{t.prefix}…</span>
						</div>
						<div class="text-[11px] text-text-muted truncate">
							{usedText(t)} · {expiryText(t)}
						</div>
					</div>
					{@render action(t)}
				</li>
			{/each}
		</ul>

		<!-- Wider screens: a table -->
		<table class="hidden md:table w-full text-xs">
			<thead class="text-text-muted text-left">
				<tr>
					<th class="py-1 pr-3 font-medium">Name</th>
					<th class="py-1 pr-3 font-medium">Access</th>
					<th class="py-1 pr-3 font-medium">Token</th>
					<th class="py-1 pr-3 font-medium">Last used</th>
					<th class="py-1 pr-3 font-medium">Expires</th>
					<th class="py-1 font-medium"><span class="sr-only">Actions</span></th>
				</tr>
			</thead>
			<tbody>
				{#each tokens as t (t.id)}
					<tr class="border-t border-border/40 {t.status !== 'active' ? 'opacity-70' : ''}">
						<td class="py-1.5 pr-3 text-text-primary">
							<span class="font-medium">{t.name}</span>
							{@render statusBadge(t)}
						</td>
						<td class="py-1.5 pr-3 text-text-secondary">{tokensApi.presetLabel(t.preset)}</td>
						<td class="py-1.5 pr-3 font-mono text-text-muted">{t.prefix}…</td>
						<td
							class="py-1.5 pr-3 text-text-secondary"
							title={t.last_used_ip ? `from ${t.last_used_ip}` : undefined}
						>
							{t.last_used_at ? timeAgo(t.last_used_at) : 'never'}
						</td>
						<td class="py-1.5 pr-3 text-text-secondary">{expiryText(t)}</td>
						<td class="py-1.5 text-right">{@render action(t)}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</SettingsSection>

<Modal open={dialogOpen} title={created ? 'Token created' : 'New token'} onClose={closeDialog}>
	{#if created}
		<p class="text-xs text-text-secondary mb-2">
			Copy <strong class="text-text-primary">{created.token.name}</strong> now: it is shown once and SvrCtlRS
			keeps only a hash.
		</p>
		<textarea
			readonly
			rows="2"
			aria-label="The new token"
			class="w-full px-3 py-2 text-sm font-mono bg-input border border-border rounded-sm text-text-primary resize-none break-all"
			onfocus={(e) => e.currentTarget.select()}
			value={created.secret}></textarea>
		<p class="text-xs text-text-muted mt-3 mb-1">Use it as</p>
		<pre
			class="text-[11px] font-mono bg-surface-raised border border-border rounded-sm px-2 py-1.5 overflow-x-auto text-text-secondary">export SVRCTLRS_TOKEN={created
				.token.prefix}…
curl -H "Authorization: Bearer $SVRCTLRS_TOKEN" \
  {location.origin}/api/v1/servers</pre>
	{:else}
		<form
			id="new-token-form"
			class="flex flex-col gap-4"
			onsubmit={(e) => {
				e.preventDefault();
				create();
			}}
		>
			<Input
				label="Name"
				bind:value={name}
				required
				maxlength={64}
				placeholder="hermes"
				autocomplete="off"
				hint="Which agent or script holds it"
			/>
			<RadioCards
				label="Access"
				options={tokensApi.PRESETS.map((p) => ({ value: p.id, label: p.label, detail: p.detail }))}
				bind:value={preset}
			/>
			<RadioCards
				label="Lifetime"
				layout="row"
				options={tokensApi.LIFETIMES.map((l) => ({
					value: l.days === null ? 'never' : String(l.days),
					label: l.label
				}))}
				bind:value={lifetime}
			/>
			{#if preset === 'full_admin' && lifetimeDays === null}
				<p class="flex gap-2 text-xs text-warning">
					<TriangleAlert class="w-4 h-4 flex-shrink-0" />
					<span
						>A full-admin token that never expires opens a shell on every server for as long as it
						exists. Keep it in a secret store and revoke it here if it leaks.</span
					>
				</p>
			{/if}
		</form>
	{/if}
	{#snippet footer()}
		{#if created}
			<Button variant="secondary" onclick={closeDialog}>Done</Button>
			<Button onclick={copySecret}><Copy class="w-4 h-4" /> Copy token</Button>
		{:else}
			<Button variant="secondary" onclick={closeDialog}>Cancel</Button>
			<Button type="submit" form="new-token-form" disabled={!canCreate}>
				{creating ? 'Creating…' : 'Create token'}
			</Button>
		{/if}
	{/snippet}
</Modal>
