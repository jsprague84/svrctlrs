<script lang="ts">
	import { onMount } from 'svelte';
	import { KeyRound, Plus, Pencil, Trash2, Sparkles, Copy, ShieldCheck } from '@lucide/svelte';
	import * as credApi from '#lib/api/credentials.js';
	import Button from '#lib/components/ui/Button.svelte';
	import Input from '#lib/components/ui/Input.svelte';
	import Select from '#lib/components/ui/Select.svelte';
	import Textarea from '#lib/components/ui/Textarea.svelte';
	import Modal from '#lib/components/ui/Modal.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import AuthorizeKeyModal from '#lib/components/servers/AuthorizeKeyModal.svelte';
	import * as credState from '#lib/state/credentials.svelte.js';
	import * as serversState from '#lib/state/servers.svelte.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import type {
		Credential,
		CredentialType,
		CreateCredential,
		UpdateCredential,
		Server
	} from '#lib/types/index.js';
	import { CREDENTIAL_TYPE_LABELS } from '#lib/types/credential.js';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';

	let showModal = $state(false);
	let editingId = $state<number | null>(null);
	let editing = $state<Credential | null>(null);
	let saving = $state(false);
	let filter = $state<'all' | CredentialType>('all');
	// Generate-key modal
	let showGenerate = $state(false);
	let genName = $state('');
	let genDescription = $state('');
	let generating = $state(false);
	// Public key modal (after generating, or from the list)
	let publicKeyFor = $state<{ id: number; name: string; key: string } | null>(null);
	let authorizeServerId = $state('');
	let authServer = $state<Server | null>(null);
	let authCredentialId = $state<number | null>(null);

	// Form fields
	let name = $state('');
	let credType = $state<CredentialType>('ssh_key');
	let value = $state('');
	let description = $state('');
	let username = $state('');
	let passphrase = $state('');
	let removePassphrase = $state(false);
	let sshKeyMode = $state<'path' | 'paste'>('paste');

	let credentials = $derived(credState.getCredentials());
	let loading = $derived(credState.isLoading());
	let credError = $derived(credState.getError());
	let servers = $derived(serversState.getServers());
	let remoteServers = $derived(servers);
	let shown = $derived(
		(filter === 'all'
			? credentials
			: credentials.filter((c) => c.credential_type === filter)
		).slice()
	);
	let keyProblem = $derived(credType === 'ssh_key' && value ? sshKeyProblem(value) : null);

	onMount(() => {
		credState.loadCredentials();
		serversState.loadServers();
	});

	function resetForm() {
		name = '';
		credType = 'ssh_key';
		value = '';
		description = '';
		username = '';
		passphrase = '';
		removePassphrase = false;
		editingId = null;
		editing = null;
		sshKeyMode = 'paste';
	}

	function openCreate() {
		resetForm();
		showModal = true;
	}

	function openEdit(cred: Credential) {
		resetForm();
		editingId = cred.id;
		editing = cred;
		name = cred.name;
		credType = cred.credential_type;
		description = cred.description ?? '';
		username = cred.username ?? '';
		sshKeyMode = cred.key_source === 'path' ? 'path' : 'paste';
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
		if (saving || keyProblem) return;
		saving = true;
		try {
			if (editingId) {
				// Empty strings clear description/username; omitted value/passphrase keep theirs
				const input: UpdateCredential = {
					name: name.trim() || undefined,
					value: value || undefined,
					description: description.trim(),
					username: username.trim(),
					passphrase: removePassphrase ? '' : passphrase || undefined
				};
				await credState.updateCredential(editingId, input);
				toast.success('Credential updated');
			} else {
				const input: CreateCredential = {
					name: name.trim(),
					credential_type: credType,
					value,
					description: description.trim() || undefined,
					username: username.trim() || undefined,
					passphrase: credType === 'ssh_key' && passphrase ? passphrase : undefined
				};
				await credState.createCredential(input);
				toast.success('Credential created');
			}
			showModal = false;
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Operation failed'));
		} finally {
			saving = false;
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
			publicKeyFor = { id: res.id, name: genName.trim(), key: res.public_key };
			authorizeServerId = '';
			genName = '';
			genDescription = '';
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to generate key'));
		} finally {
			generating = false;
		}
	}

	async function showPublicKey(cred: Credential) {
		try {
			const full = await credApi.getCredential(cred.id);
			if (!full.public_key) {
				toast.error('No public key: this credential holds a path, not key material');
				return;
			}
			publicKeyFor = { id: cred.id, name: cred.name, key: full.public_key };
			authorizeServerId = '';
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to load the public key'));
		}
	}

	function authorizeFromPublicKey() {
		if (!publicKeyFor || !authorizeServerId) return;
		const server = servers.find((s) => s.id === Number(authorizeServerId));
		if (!server) return;
		authCredentialId = publicKeyFor.id;
		authServer = server;
		publicKeyFor = null;
	}

	async function copyText(text: string) {
		try {
			await navigator.clipboard.writeText(text);
			toast.success('Copied');
		} catch {
			toast.error('Clipboard unavailable; select and copy the text');
		}
	}

	async function handleDelete(cred: Credential) {
		const ok = await confirmDialog({
			title: `Delete ${cred.name}?`,
			message:
				cred.credential_type === 'ssh_key'
					? 'The private key stored here is destroyed. Servers that trust its public half keep working with other keys only.'
					: 'The stored secret is destroyed.',
			confirmLabel: 'Delete',
			destructive: true
		});
		if (!ok) return;
		try {
			await credState.deleteCredential(cred.id);
			toast.success('Credential deleted');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Delete failed'));
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
			{#if credentials.length > 4}
				<select
					bind:value={filter}
					aria-label="Filter by type"
					class="px-2 py-1.5 text-sm bg-input border border-border rounded-sm text-text-primary"
				>
					<option value="all">All types</option>
					<option value="ssh_key">SSH keys</option>
					<option value="password">Passwords</option>
				</select>
			{/if}
			<Button variant="secondary" onclick={openCreate}>
				<Plus class="w-4 h-4" /> Add existing
			</Button>
			<Button onclick={() => (showGenerate = true)}>
				<Sparkles class="w-4 h-4" /> Generate SSH key
			</Button>
		</div>
	</div>

	<div class="flex-1 overflow-y-auto p-4 md:p-6">
		{#if loading && credentials.length === 0}
			<StateBlock kind="loading" title="Loading credentials…" />
		{:else if credError}
			<StateBlock kind="error" title={credError} onRetry={() => credState.loadCredentials()} />
		{:else if credentials.length === 0}
			<StateBlock
				kind="empty"
				icon={KeyRound}
				title="No credentials yet"
				hint="The easy path: generate a key here, then Authorize key on each server."
			>
				<Button onclick={() => (showGenerate = true)}>
					<Sparkles class="w-4 h-4" /> Generate SSH key
				</Button>
				<Button variant="secondary" onclick={openCreate}>Add an existing one</Button>
			</StateBlock>
		{:else}
			<ul class="flex flex-col gap-2">
				{#each shown as cred (cred.id)}
					<li
						class="border border-border/50 rounded-lg px-3 md:px-4 py-3 hover:bg-surface-raised/30 flex flex-col md:flex-row md:items-center gap-2 md:gap-4"
					>
						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-2 min-w-0 flex-wrap">
								<span class="text-text-primary font-medium truncate">{cred.name}</span>
								<Badge variant={typeVariant[cred.credential_type]}>
									{CREDENTIAL_TYPE_LABELS[cred.credential_type]}{cred.key_source === 'path'
										? ' (file path)'
										: ''}
								</Badge>
								{#if cred.has_passphrase}
									<Badge>passphrase</Badge>
								{/if}
								{#if cred.username}
									<span class="text-xs text-text-muted"
										>default user <code>{cred.username}</code></span
									>
								{/if}
							</div>
							{#if cred.fingerprint}
								<div class="mt-0.5 text-[11px] font-mono text-text-secondary truncate">
									{cred.fingerprint}
								</div>
							{/if}
							<div class="mt-0.5 text-xs text-text-muted truncate">
								{#if cred.used_by && cred.used_by.length > 0}
									Used by {cred.used_by.join(', ')}
								{:else}
									Not used by any server
								{/if}
								{#if cred.description}
									· {cred.description}
								{/if}
							</div>
						</div>
						<div class="flex items-center gap-1 flex-wrap md:flex-nowrap md:flex-shrink-0">
							{#if cred.credential_type === 'ssh_key' && cred.key_source !== 'path'}
								<Button
									variant="ghost"
									size="sm"
									class="min-h-11 md:min-h-0"
									onclick={() => showPublicKey(cred)}
									title="Show the public key / authorize it on a server"
								>
									<ShieldCheck class="w-3.5 h-3.5" />
									Public key
								</Button>
							{/if}
							<Button
								variant="ghost"
								size="sm"
								class="min-h-11 min-w-11 md:min-h-0 md:min-w-0"
								onclick={() => openEdit(cred)}
								aria-label="Edit credential"
								title="Edit"
							>
								<Pencil class="w-3.5 h-3.5" />
								<span class="hidden lg:inline">Edit</span>
							</Button>
							<Button
								variant="ghost"
								size="sm"
								class="min-h-11 min-w-11 md:min-h-0 md:min-w-0"
								onclick={() => handleDelete(cred)}
								disabled={cred.in_use}
								aria-label="Delete credential"
								title={cred.in_use
									? `In use by ${cred.used_by?.join(', ')} — change those servers first`
									: 'Delete'}
							>
								<Trash2 class="w-3.5 h-3.5 text-error" />
								<span class="hidden lg:inline">Delete</span>
							</Button>
						</div>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</div>

<Modal
	open={showModal}
	title={editingId ? `Edit ${editing?.name ?? 'credential'}` : 'Add an existing credential'}
	onClose={() => (showModal = false)}
>
	<form
		id="credential-form"
		class="flex flex-col gap-3"
		onsubmit={(e) => {
			e.preventDefault();
			handleSubmit();
		}}
	>
		<Input label="Name" bind:value={name} required placeholder="laptop key" />
		{#if !editingId}
			<Select label="Type" bind:value={credType}>
				<option value="ssh_key">SSH key</option>
				<option value="password">Password</option>
			</Select>
		{/if}

		{#if credType === 'ssh_key'}
			{#if !editingId || editing?.key_source}
				<div class="flex items-center gap-2" role="group" aria-label="Key input mode">
					<span class="text-xs text-text-muted">Provide the key as</span>
					<button
						type="button"
						class="text-xs px-2 py-1 rounded-sm transition-colors {sshKeyMode === 'paste'
							? 'bg-accent text-accent-foreground'
							: 'bg-surface-raised text-text-muted hover:text-text-primary'}"
						aria-pressed={sshKeyMode === 'paste'}
						onclick={() => (sshKeyMode = 'paste')}>Pasted private key</button
					>
					<button
						type="button"
						class="text-xs px-2 py-1 rounded-sm transition-colors {sshKeyMode === 'path'
							? 'bg-accent text-accent-foreground'
							: 'bg-surface-raised text-text-muted hover:text-text-primary'}"
						aria-pressed={sshKeyMode === 'path'}
						onclick={() => (sshKeyMode = 'path')}>File path on the server</button
					>
				</div>
			{/if}
			{#if sshKeyMode === 'path'}
				<Input
					label={editingId ? 'New key path (leave empty to keep)' : 'Key path'}
					bind:value
					placeholder="/home/svrctlrs/.ssh/id_ed25519"
					required={!editingId}
					error={keyProblem ?? ''}
				/>
				<p class="text-[11px] text-text-muted">
					An absolute path inside the SvrCtlRS container (the mounted <code>SSH_KEY_PATH</code>
					directory) — not a path on this machine. Pasting the key is simpler and stores it encrypted.
				</p>
			{:else}
				<Textarea
					label={editingId ? 'New private key (leave empty to keep)' : 'Private key'}
					bind:value
					rows={6}
					mono
					placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
					required={!editingId}
					error={keyProblem ?? ''}
					hint="The file without .pub. Stored encrypted; never shown again."
				/>
			{/if}
			{#if editing?.has_passphrase}
				<label class="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
					<input type="checkbox" bind:checked={removePassphrase} class="rounded" />
					Remove the stored passphrase
				</label>
			{/if}
			{#if !removePassphrase}
				<Input
					label={editingId
						? 'New passphrase (leave empty to keep)'
						: 'Passphrase (only if the key is encrypted)'}
					type="password"
					bind:value={passphrase}
					autocomplete="off"
				/>
			{/if}
		{:else if credType === 'password'}
			<Input
				label={editingId ? 'New password (leave empty to keep)' : 'Password'}
				type="password"
				bind:value
				required={!editingId}
				autocomplete="off"
			/>
		{/if}

		<Input
			label="Default username"
			bind:value={username}
			placeholder="root"
			hint="Used when a server has no username of its own"
		/>
		<Input label="Description" bind:value={description} placeholder="Optional" />
	</form>

	{#snippet footer()}
		<Button variant="secondary" onclick={() => (showModal = false)}>Cancel</Button>
		<Button type="submit" form="credential-form" disabled={saving || !!keyProblem}>
			{saving ? 'Saving…' : editingId ? 'Save' : 'Add'}
		</Button>
	{/snippet}
</Modal>

<Modal open={showGenerate} title="Generate SSH key" onClose={() => (showGenerate = false)}>
	<form
		id="generate-key-form"
		class="flex flex-col gap-3"
		onsubmit={(e) => {
			e.preventDefault();
			handleGenerate();
		}}
	>
		<p class="text-xs text-text-muted">
			An ed25519 key pair is created on the SvrCtlRS server. The private half is stored encrypted
			and never shown; the public half goes into each server's <code>~/.ssh/authorized_keys</code>
			— <em>Authorize key</em> does that for you.
		</p>
		<Input label="Name" bind:value={genName} required placeholder="svrctlrs" />
		<Input label="Description" bind:value={genDescription} placeholder="Optional" />
	</form>
	{#snippet footer()}
		<Button variant="secondary" onclick={() => (showGenerate = false)}>Cancel</Button>
		<Button type="submit" form="generate-key-form" disabled={generating || !genName.trim()}>
			{generating ? 'Generating…' : 'Generate'}
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
			Add this line to <code>~/.ssh/authorized_keys</code> on each server this key should open — or pick
			a server below and SvrCtlRS installs it for you.
		</p>
		<textarea
			readonly
			rows="3"
			class="w-full px-3 py-2 text-xs font-mono bg-input border border-border rounded-sm text-text-primary resize-y"
			value={publicKeyFor.key}></textarea>
		{#if remoteServers.length > 0}
			<div class="flex items-end gap-2 mt-3">
				<div class="flex-1">
					<Select label="Authorize on a server" bind:value={authorizeServerId}>
						<option value="">Choose a server…</option>
						{#each remoteServers as s (s.id)}
							<option value={String(s.id)}>{s.name}</option>
						{/each}
					</Select>
				</div>
				<Button variant="secondary" onclick={authorizeFromPublicKey} disabled={!authorizeServerId}>
					<ShieldCheck class="w-4 h-4" /> Authorize…
				</Button>
			</div>
		{/if}
	{/if}
	{#snippet footer()}
		<Button variant="secondary" onclick={() => (publicKeyFor = null)}>Close</Button>
		<Button onclick={() => publicKeyFor && copyText(publicKeyFor.key)}>
			<Copy class="w-4 h-4" /> Copy
		</Button>
	{/snippet}
</Modal>

<AuthorizeKeyModal
	server={authServer}
	credentialId={authCredentialId}
	onClose={() => (authServer = null)}
	onDone={() => credState.loadCredentials()}
/>
