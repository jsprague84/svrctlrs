<script lang="ts">
	import { untrack } from 'svelte';
	import { Sparkles } from '@lucide/svelte';
	import * as serversApi from '#lib/api/servers.js';
	import * as credApi from '#lib/api/credentials.js';
	import * as credState from '#lib/state/credentials.svelte.js';
	import * as serversState from '#lib/state/servers.svelte.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import Modal from '#lib/components/ui/Modal.svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Input from '#lib/components/ui/Input.svelte';
	import Select from '#lib/components/ui/Select.svelte';
	import type { Server } from '#lib/types/index.js';

	interface Props {
		/** The server to install the key on; `null` keeps the modal closed */
		server: Pick<Server, 'id' | 'name' | 'hostname' | 'username' | 'credential_id'> | null;
		/** Preselect this key (e.g. one just generated) */
		credentialId?: number | null;
		onClose: () => void;
		/** Called after the key is installed and the server switched to it */
		onDone?: () => void;
	}

	let { server, credentialId = null, onClose, onDone }: Props = $props();

	let credentials = $derived(credState.getCredentials());
	let keyCredentials = $derived(credentials.filter((c) => c.credential_type === 'ssh_key'));
	let bootstrapCredentials = $derived(
		credentials.filter((c) => c.credential_type === 'ssh_key' || c.credential_type === 'password')
	);
	let loginAs = $derived(server?.username || 'root');

	let keyId = $state('');
	let mode = $state<'password' | 'credential'>('password');
	let password = $state('');
	let bootstrapId = $state('');
	let busy = $state(false);
	let generating = $state(false);

	/** The bootstrap login to suggest: the server's own credential, else any other usable one,
	 *  never the key being installed */
	function suggestBootstrap(forKey: string, serverCredential: number | null): string {
		if (serverCredential && String(serverCredential) !== forKey) return String(serverCredential);
		const other = bootstrapCredentials.find((c) => String(c.id) !== forKey);
		return other ? String(other.id) : '';
	}

	// Reset the form when the modal opens for a server (or is handed a key to preselect).
	// Only those two are tracked: reading the form's own fields here made every pick in the
	// key list re-run the reset and snap back to the first key.
	$effect(() => {
		const target = server;
		const preselect = credentialId;
		if (!target) return;
		untrack(() => {
			keyId = preselect ? String(preselect) : keyCredentials[0] ? String(keyCredentials[0].id) : '';
			mode = 'password';
			password = '';
			bootstrapId = suggestBootstrap(keyId, target.credential_id);
		});
	});

	// Credentials that finish loading after the modal opened fill the empty choices only
	$effect(() => {
		const keys = keyCredentials;
		if (!server) return;
		untrack(() => {
			if (!keyId && keys[0]) keyId = String(keys[0].id);
			if (!bootstrapId) bootstrapId = suggestBootstrap(keyId, server?.credential_id ?? null);
		});
	});

	let canSubmit = $derived(
		!busy && !!keyId && (mode === 'password' ? password.length > 0 : bootstrapId.length > 0)
	);

	async function generateNow() {
		if (!server) return;
		generating = true;
		try {
			const name = `svrctlrs-${new Date().toISOString().slice(0, 10)}`;
			const res = await credApi.generateKey({ name, description: `Generated for ${server.name}` });
			await credState.loadCredentials();
			keyId = String(res.id);
			toast.success(`Key "${name}" generated`);
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to generate key'));
		} finally {
			generating = false;
		}
	}

	async function submit() {
		if (!server || !canSubmit) return;
		busy = true;
		try {
			const res = await serversApi.authorizeKey(server.id, {
				credential_id: Number(keyId),
				password: mode === 'password' ? password : undefined,
				bootstrap_credential_id: mode === 'credential' ? Number(bootstrapId) : undefined
			});
			toast.success(res.message);
			password = '';
			await serversState.loadServers();
			onDone?.();
			onClose();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Authorizing the key failed'));
		} finally {
			busy = false;
		}
	}
</script>

<Modal open={server !== null} title={`Authorize key on ${server?.name ?? ''}`} {onClose}>
	<form
		id="authorize-key-form"
		class="flex flex-col gap-3"
		onsubmit={(e) => {
			e.preventDefault();
			submit();
		}}
	>
		<p class="text-xs text-text-muted">
			SvrCtlRS logs in once as <code>{loginAs}@{server?.hostname ?? '…'}</code>, appends the key's
			public half to <code>~/.ssh/authorized_keys</code> (like <code>ssh-copy-id</code>), checks
			that the key logs in, and switches this server to it. The password is used once and never
			stored.
		</p>

		{#if keyCredentials.length === 0}
			<div
				class="flex items-center justify-between gap-2 rounded-sm border border-warning/40 bg-warning/10 px-3 py-2"
			>
				<span class="text-sm text-warning">No SSH key yet.</span>
				<Button
					type="button"
					size="sm"
					variant="secondary"
					onclick={generateNow}
					disabled={generating}
				>
					<Sparkles class="w-3.5 h-3.5" />
					{generating ? 'Generating…' : 'Generate one now'}
				</Button>
			</div>
		{:else}
			<div class="flex items-end gap-2">
				<div class="flex-1">
					<Select label="Key to authorize" bind:value={keyId}>
						{#each keyCredentials as c (c.id)}
							<option value={String(c.id)}>{c.name}</option>
						{/each}
					</Select>
				</div>
				<Button
					type="button"
					size="sm"
					variant="ghost"
					onclick={generateNow}
					disabled={generating}
					title="Generate a new ed25519 key"
				>
					<Sparkles class="w-3.5 h-3.5" />
					{generating ? 'Generating…' : 'New key'}
				</Button>
			</div>
		{/if}

		<fieldset class="flex flex-col gap-2">
			<legend class="text-xs text-text-muted mb-1">Log in once with</legend>
			<div class="flex items-center gap-2" role="group" aria-label="Bootstrap method">
				<button
					type="button"
					class="text-xs px-2 py-1 rounded-sm transition-colors {mode === 'password'
						? 'bg-accent text-accent-foreground'
						: 'bg-surface-raised text-text-muted hover:text-text-primary'}"
					aria-pressed={mode === 'password'}
					onclick={() => (mode = 'password')}>Password</button
				>
				<button
					type="button"
					class="text-xs px-2 py-1 rounded-sm transition-colors {mode === 'credential'
						? 'bg-accent text-accent-foreground'
						: 'bg-surface-raised text-text-muted hover:text-text-primary'}"
					aria-pressed={mode === 'credential'}
					onclick={() => (mode = 'credential')}>A credential that already works</button
				>
			</div>
			{#if mode === 'password'}
				<Input
					label={`Password for ${loginAs}@${server?.hostname ?? ''}`}
					type="password"
					bind:value={password}
					autocomplete="off"
				/>
				<p class="text-[11px] text-text-muted">
					Hosts with <code>PasswordAuthentication no</code> refuse this; use a credential that
					already works instead, or add the public key to <code>authorized_keys</code> by hand.
				</p>
			{:else}
				<Select label="Credential" bind:value={bootstrapId}>
					<option value="">Choose…</option>
					{#each bootstrapCredentials as c (c.id)}
						<option value={String(c.id)}>{c.name}</option>
					{/each}
				</Select>
			{/if}
		</fieldset>
	</form>
	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>Cancel</Button>
		<Button type="submit" form="authorize-key-form" disabled={!canSubmit}>
			{busy ? 'Authorizing…' : 'Authorize'}
		</Button>
	{/snippet}
</Modal>
