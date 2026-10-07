<script lang="ts">
	import { onMount } from 'svelte';
	import { KeyRound, Plus, Pencil, Trash2, Sparkles, Copy } from 'lucide-svelte';
	import * as credApi from '#lib/api/credentials.js';
	import Button from '#lib/components/ui/Button.svelte';
	import Input from '#lib/components/ui/Input.svelte';
	import Select from '#lib/components/ui/Select.svelte';
	import Modal from '#lib/components/ui/Modal.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import * as credState from '#lib/state/credentials.svelte.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import type { CredentialType, CreateCredential, UpdateCredential } from '#lib/types/index.js';
	import { CREDENTIAL_TYPE_LABELS } from '#lib/types/credential.js';

	let showModal = $state(false);
	let editingId = $state<number | null>(null);
	// Generate-key modal
	let showGenerate = $state(false);
	let genName = $state('');
	let genDescription = $state('');
	let generating = $state(false);
	// Public key modal (after generating, or from the list)
	let publicKeyFor = $state<{ name: string; key: string } | null>(null);

	// Form fields
	let name = $state('');
	let credType = $state<CredentialType>('ssh_key');
	let value = $state('');
	let description = $state('');
	let username = $state('');
	let passphrase = $state('');
	let sshKeyMode = $state<'path' | 'paste'>('path');

	let credentials = $derived(credState.getCredentials());
	let loading = $derived(credState.isLoading());
	let credError = $derived(credState.getError());

	onMount(() => {
		credState.loadCredentials();
	});

	function resetForm() {
		name = '';
		credType = 'ssh_key';
		value = '';
		description = '';
		username = '';
		passphrase = '';
		editingId = null;
		sshKeyMode = 'path';
	}

	function openCreate() {
		resetForm();
		showModal = true;
	}

	function openEdit(cred: (typeof credentials)[0]) {
		editingId = cred.id;
		name = cred.name;
		credType = cred.credential_type;
		value = ''; // Never show existing value
		description = cred.description ?? '';
		username = cred.username ?? '';
		passphrase = ''; // Never show the existing passphrase
		showModal = true;
	}

	/** Catch the common mistake before the request: a public key or a laptop path. */
	function sshKeyProblem(v: string): string | null {
		const t = v.trim();
		if (!t) return null;
		if (/^(ssh-(ed25519|rsa|dss) |ecdsa-sha2-|sk-)/.test(t)) {
			return 'That is a public key. Paste the PRIVATE key: the file without .pub, which starts with -----BEGIN OPENSSH PRIVATE KEY-----.';
		}
		if (!t.startsWith('-----BEGIN')) {
			if (t.endsWith('.pub')) return 'That path is a public key (.pub). Use the private key file.';
			if (!t.startsWith('/'))
				return 'A key path must be absolute and exist on the SvrCtlRS server (inside its container). To use a key from this machine, paste the private key instead.';
		}
		return null;
	}

	async function handleSubmit() {
		if (credType === 'ssh_key' && value) {
			const problem = sshKeyProblem(value);
			if (problem) {
				toast.error(problem);
				return;
			}
		}
		try {
			if (editingId) {
				const input: UpdateCredential = {
					name: name || undefined,
					value: value || undefined,
					description: description || undefined,
					username: username || undefined,
					passphrase: passphrase || undefined
				};
				await credState.updateCredential(editingId, input);
				toast.success('Credential updated');
			} else {
				const input: CreateCredential = {
					name,
					credential_type: credType,
					value,
					description: description || undefined,
					username: username || undefined,
					passphrase: credType === 'ssh_key' && passphrase ? passphrase : undefined
				};
				await credState.createCredential(input);
				toast.success('Credential created');
			}
			showModal = false;
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Operation failed'));
		}
	}

	async function handleGenerate() {
		if (!genName.trim()) return;
		generating = true;
		try {
			const res = await credApi.generateKey({
				name: genName.trim(),
				description: genDescription || undefined
			});
			await credState.loadCredentials();
			showGenerate = false;
			publicKeyFor = { name: genName.trim(), key: res.public_key };
			genName = '';
			genDescription = '';
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to generate key'));
		} finally {
			generating = false;
		}
	}
	async function showPublicKey(cred: { id: number; name: string }) {
		try {
			const full = await credApi.getCredential(cred.id);
			if (!full.public_key) {
				toast.error('No public key: this credential holds a path, not key material');
				return;
			}
			publicKeyFor = { name: cred.name, key: full.public_key };
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to load the public key'));
		}
	}
	async function copyText(text: string) {
		try {
			await navigator.clipboard.writeText(text);
			toast.success('Copied');
		} catch {
			toast.error('Clipboard unavailable; select and copy the text');
		}
	}
	async function handleDelete(id: number) {
		if (!confirm('Delete this credential?')) return;
		try {
			await credState.deleteCredential(id);
			toast.success('Credential deleted');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Delete failed (may be in use by a server)'));
		}
	}

	const typeVariant: Record<CredentialType, 'info' | 'success' | 'warning' | 'default'> = {
		ssh_key: 'info',
		password: 'warning',
		api_token: 'success',
		certificate: 'default'
	};
</script>

<div class="flex flex-col h-full">
	<div
		class="flex flex-wrap items-center justify-between gap-2 px-4 md:px-6 py-4 border-b border-border"
	>
		<div class="flex items-center gap-3">
			<KeyRound class="w-5 h-5 text-accent" />
			<h1 class="text-lg font-semibold text-text-primary">Credentials</h1>
			<Badge>{credentials.length}</Badge>
		</div>
		<div class="flex items-center gap-2">
			<Button variant="secondary" onclick={() => (showGenerate = true)}>
				<Sparkles class="w-4 h-4" /> Generate SSH key
			</Button>
			<Button onclick={openCreate}><Plus class="w-4 h-4" /> Add Credential</Button>
		</div>
	</div>

	<div class="flex-1 overflow-y-auto p-4 md:p-6">
		{#if loading}
			<div class="text-text-muted">Loading...</div>
		{:else if credError}
			<div class="text-center py-8">
				<p class="text-error">{credError}</p>
				<Button variant="secondary" class="mt-3" onclick={() => credState.loadCredentials()}
					>Retry</Button
				>
			</div>
		{:else if credentials.length === 0}
			<div class="text-center text-text-muted">
				<p>No credentials configured yet.</p>
				<Button variant="secondary" class="mt-3" onclick={openCreate}
					>Add your first credential</Button
				>
			</div>
		{:else}
			<div class="flex flex-col gap-2">
				{#each credentials as cred (cred.id)}
					<div
						class="flex flex-col md:flex-row md:items-center gap-1 md:gap-4 border border-border/50 rounded-lg px-4 py-3 hover:bg-surface-raised/30"
					>
						<div class="flex items-center gap-2 min-w-0 w-full md:w-auto md:flex-1">
							<span class="text-text-primary font-medium truncate">{cred.name}</span>
						</div>
						<div class="w-full md:w-auto md:flex-shrink-0">
							<Badge variant={typeVariant[cred.credential_type]}>
								{CREDENTIAL_TYPE_LABELS[cred.credential_type]}
							</Badge>
						</div>
						{#if cred.username}
							<div class="text-text-secondary text-xs w-full md:w-auto md:flex-1 truncate">
								User: {cred.username}
							</div>
						{/if}
						{#if cred.has_passphrase}
							<div class="w-full md:w-auto md:flex-shrink-0"><Badge>passphrase</Badge></div>
						{/if}
						{#if cred.description}
							<div class="text-text-muted text-xs w-full md:flex-1 truncate">
								{cred.description}
							</div>
						{/if}
						<div class="flex items-center gap-1 w-full md:w-auto md:flex-shrink-0">
							{#if cred.credential_type === 'ssh_key'}
								<Button
									variant="ghost"
									size="sm"
									onclick={() => showPublicKey(cred)}
									title="Show public key"
								>
									<KeyRound class="w-3.5 h-3.5" />
									<span class="hidden md:inline">Public key</span>
								</Button>
							{/if}
							<Button variant="ghost" size="sm" onclick={() => openEdit(cred)}>
								<Pencil class="w-3.5 h-3.5" />
								<span class="hidden md:inline">Edit</span>
							</Button>
							<Button variant="ghost" size="sm" onclick={() => handleDelete(cred.id)}>
								<Trash2 class="w-3.5 h-3.5 text-error" />
								<span class="hidden md:inline">Delete</span>
							</Button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>

<Modal
	open={showModal}
	title={editingId ? 'Edit Credential' : 'Add Credential'}
	onClose={() => (showModal = false)}
>
	<form
		class="flex flex-col gap-3"
		onsubmit={(e) => {
			e.preventDefault();
			handleSubmit();
		}}
	>
		<Input label="Name" bind:value={name} required placeholder="my-ssh-key" />
		{#if !editingId}
			<Select label="Type" bind:value={credType}>
				<option value="ssh_key">SSH Key</option>
				<option value="password">Password</option>
				<option value="api_token">API Token</option>
				<option value="certificate">Certificate</option>
			</Select>
		{/if}

		{#if credType === 'ssh_key'}
			<div class="flex items-center gap-2 mb-1">
				<span class="text-xs text-text-muted">Input mode:</span>
				<button
					type="button"
					class="text-xs px-2 py-0.5 rounded-sm transition-colors {sshKeyMode === 'path'
						? 'bg-accent text-accent-foreground'
						: 'bg-surface-raised text-text-muted hover:text-text-primary'}"
					onclick={() => {
						sshKeyMode = 'path';
						value = '';
					}}>File path</button
				>
				<button
					type="button"
					class="text-xs px-2 py-0.5 rounded-sm transition-colors {sshKeyMode === 'paste'
						? 'bg-accent text-accent-foreground'
						: 'bg-surface-raised text-text-muted hover:text-text-primary'}"
					onclick={() => {
						sshKeyMode = 'paste';
						value = '';
					}}>Paste key</button
				>
			</div>
			<p class="text-xs text-text-muted">
				{#if sshKeyMode === 'paste'}
					Paste the <strong>private</strong> key — the file <em>without</em> <code>.pub</code>,
					starting with
					<code>-----BEGIN OPENSSH PRIVATE KEY-----</code>. It is stored encrypted and never leaves
					the server.
				{:else}
					An absolute path on the <strong>SvrCtlRS server</strong> (inside its container, e.g.
					<code>/home/svrctlrs/.ssh/id_ed25519</code>) — not a path on this machine. Prefer pasting
					the key.
				{/if}
			</p>
			{#if sshKeyMode === 'path'}
				<Input
					label={editingId ? 'New SSH Key Path (leave empty to keep)' : 'SSH Key Path'}
					bind:value
					placeholder="/home/user/.ssh/id_ed25519"
					required={!editingId}
				/>
			{:else}
				<label class="flex flex-col gap-1">
					<span class="text-sm font-medium text-text-primary"
						>{editingId ? 'New SSH Key (leave empty to keep)' : 'SSH Key Content'}</span
					>
					<textarea
						bind:value
						rows={6}
						class="w-full px-3 py-2 text-sm font-mono bg-input border border-border rounded-sm text-text-primary placeholder:text-text-muted resize-y focus:ring-1 focus:ring-ring focus:outline-none"
						placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
						required={!editingId}></textarea>
				</label>
			{/if}
			<Input
				label={editingId
					? 'New Passphrase (leave empty to keep)'
					: 'Passphrase (if the key is encrypted)'}
				type="password"
				bind:value={passphrase}
				placeholder="optional"
			/>
		{:else if credType === 'password'}
			<Input
				label={editingId ? 'New Password (leave empty to keep)' : 'Password'}
				type="password"
				bind:value
				required={!editingId}
			/>
			<Input label="Username" bind:value={username} placeholder="root" />
		{:else if credType === 'api_token'}
			<Input
				label={editingId ? 'New Token (leave empty to keep)' : 'API Token'}
				type="password"
				bind:value
				required={!editingId}
			/>
		{:else}
			<Input
				label={editingId ? 'New Certificate Path (leave empty to keep)' : 'Certificate Path'}
				bind:value
				required={!editingId}
			/>
		{/if}

		<Input label="Description" bind:value={description} placeholder="Optional description" />
	</form>

	{#snippet footer()}
		<Button variant="secondary" onclick={() => (showModal = false)}>Cancel</Button>
		<Button onclick={handleSubmit}>{editingId ? 'Save' : 'Create'}</Button>
	{/snippet}
</Modal>

<Modal open={showGenerate} title="Generate SSH key" onClose={() => (showGenerate = false)}>
	<form
		class="flex flex-col gap-3"
		onsubmit={(e) => {
			e.preventDefault();
			handleGenerate();
		}}
	>
		<p class="text-xs text-text-muted">
			An ed25519 key pair is created on the SvrCtlRS server. The private key is stored encrypted and
			never shown; you get the public key to put in each server's <code>~/.ssh/authorized_keys</code
			>
			— or let <em>Authorize key…</em> on a server do that for you.
		</p>
		<Input label="Name" bind:value={genName} required placeholder="svrctlrs-key" />
		<Input label="Description" bind:value={genDescription} placeholder="Optional" />
	</form>
	{#snippet footer()}
		<Button variant="secondary" onclick={() => (showGenerate = false)}>Cancel</Button>
		<Button onclick={handleGenerate} disabled={generating || !genName.trim()}>
			{generating ? 'Generating...' : 'Generate'}
		</Button>
	{/snippet}
</Modal>

<Modal
	open={publicKeyFor !== null}
	title={`Public key — ${publicKeyFor?.name ?? ''}`}
	onClose={() => (publicKeyFor = null)}
>
	{#if publicKeyFor}
		<p class="text-xs text-text-muted mb-2">
			Add this line to <code>~/.ssh/authorized_keys</code> on each server this key should open, or
			use
			<em>Authorize key…</em> on the server's card to have SvrCtlRS do it.
		</p>
		<textarea
			readonly
			rows="3"
			class="w-full px-3 py-2 text-xs font-mono bg-input border border-border rounded-sm text-text-primary resize-y"
			value={publicKeyFor.key}></textarea>
	{/if}
	{#snippet footer()}
		<Button variant="secondary" onclick={() => (publicKeyFor = null)}>Close</Button>
		<Button onclick={() => publicKeyFor && copyText(publicKeyFor.key)}
			><Copy class="w-4 h-4" /> Copy</Button
		>
	{/snippet}
</Modal>
