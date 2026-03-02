<script lang="ts">
	import { onMount } from 'svelte';
	import { KeyRound, Plus, Pencil, Trash2 } from 'lucide-svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Input from '$lib/components/ui/Input.svelte';
	import Select from '$lib/components/ui/Select.svelte';
	import Modal from '$lib/components/ui/Modal.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';
	import * as credState from '$lib/state/credentials.svelte.js';
	import * as toast from '$lib/state/toast.svelte.js';
	import type { CredentialType, CreateCredential, UpdateCredential } from '$lib/types/index.js';
	import { CREDENTIAL_TYPE_LABELS } from '$lib/types/credential.js';

	let showModal = $state(false);
	let editingId = $state<number | null>(null);

	// Form fields
	let name = $state('');
	let credType = $state<CredentialType>('ssh_key');
	let value = $state('');
	let description = $state('');
	let username = $state('');
	let sshKeyMode = $state<'path' | 'paste'>('path');

	let credentials = $derived(credState.getCredentials());
	let loading = $derived(credState.isLoading());

	onMount(() => {
		credState.loadCredentials();
	});

	function resetForm() {
		name = ''; credType = 'ssh_key'; value = ''; description = ''; username = '';
		editingId = null; sshKeyMode = 'path';
	}

	function openCreate() {
		resetForm();
		showModal = true;
	}

	function openEdit(cred: typeof credentials[0]) {
		editingId = cred.id;
		name = cred.name;
		credType = cred.credential_type;
		value = ''; // Never show existing value
		description = cred.description ?? '';
		username = cred.username ?? '';
		showModal = true;
	}

	async function handleSubmit() {
		try {
			if (editingId) {
				const input: UpdateCredential = {
					name: name || undefined,
					value: value || undefined,
					description: description || undefined,
					username: username || undefined
				};
				await credState.updateCredential(editingId, input);
				toast.success('Credential updated');
			} else {
				const input: CreateCredential = {
					name,
					credential_type: credType,
					value,
					description: description || undefined,
					username: username || undefined
				};
				await credState.createCredential(input);
				toast.success('Credential created');
			}
			showModal = false;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'Operation failed');
		}
	}

	async function handleDelete(id: number) {
		if (!confirm('Delete this credential?')) return;
		try {
			await credState.deleteCredential(id);
			toast.success('Credential deleted');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'Delete failed (may be in use by a server)');
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
	<div class="flex items-center justify-between px-6 py-4 border-b border-border">
		<div class="flex items-center gap-3">
			<KeyRound class="w-5 h-5 text-accent" />
			<h1 class="text-lg font-semibold text-text-primary">Credentials</h1>
			<Badge>{credentials.length}</Badge>
		</div>
		<Button onclick={openCreate}><Plus class="w-4 h-4" /> Add Credential</Button>
	</div>

	<div class="flex-1 overflow-y-auto">
		{#if loading}
			<div class="p-6 text-text-muted">Loading...</div>
		{:else if credentials.length === 0}
			<div class="p-6 text-center text-text-muted">
				<p>No credentials configured yet.</p>
				<Button variant="secondary" class="mt-3" onclick={openCreate}>Add your first credential</Button>
			</div>
		{:else}
			<table class="w-full text-sm">
				<thead>
					<tr class="border-b border-border text-left text-text-muted">
						<th class="px-4 py-2 font-medium">Name</th>
						<th class="px-4 py-2 font-medium">Type</th>
						<th class="px-4 py-2 font-medium">Username</th>
						<th class="px-4 py-2 font-medium">Description</th>
						<th class="px-4 py-2 font-medium text-right">Actions</th>
					</tr>
				</thead>
				<tbody>
					{#each credentials as cred (cred.id)}
						<tr class="border-b border-border/50 hover:bg-surface-raised/30">
							<td class="px-4 py-2 text-text-primary font-medium">{cred.name}</td>
							<td class="px-4 py-2">
								<Badge variant={typeVariant[cred.credential_type]}>
									{CREDENTIAL_TYPE_LABELS[cred.credential_type]}
								</Badge>
							</td>
							<td class="px-4 py-2 text-text-secondary">{cred.username ?? '—'}</td>
							<td class="px-4 py-2 text-text-muted text-xs truncate max-w-[200px]">{cred.description ?? '—'}</td>
							<td class="px-4 py-2 text-right">
								<div class="flex items-center justify-end gap-1">
									<Button variant="ghost" size="sm" onclick={() => openEdit(cred)}>
										<Pencil class="w-3.5 h-3.5" />
									</Button>
									<Button variant="ghost" size="sm" onclick={() => handleDelete(cred.id)}>
										<Trash2 class="w-3.5 h-3.5 text-error" />
									</Button>
								</div>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>

<Modal open={showModal} title={editingId ? 'Edit Credential' : 'Add Credential'} onClose={() => showModal = false}>
	<form class="flex flex-col gap-3" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
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
					class="text-xs px-2 py-0.5 rounded-sm transition-colors {sshKeyMode === 'path' ? 'bg-accent text-accent-foreground' : 'bg-surface-raised text-text-muted hover:text-text-primary'}"
					onclick={() => { sshKeyMode = 'path'; value = ''; }}
				>File path</button>
				<button
					type="button"
					class="text-xs px-2 py-0.5 rounded-sm transition-colors {sshKeyMode === 'paste' ? 'bg-accent text-accent-foreground' : 'bg-surface-raised text-text-muted hover:text-text-primary'}"
					onclick={() => { sshKeyMode = 'paste'; value = ''; }}
				>Paste key</button>
			</div>
			{#if sshKeyMode === 'path'}
				<Input label={editingId ? 'New SSH Key Path (leave empty to keep)' : 'SSH Key Path'} bind:value={value} placeholder="/home/user/.ssh/id_ed25519" required={!editingId} />
			{:else}
				<label class="flex flex-col gap-1">
					<span class="text-sm font-medium text-text-primary">{editingId ? 'New SSH Key (leave empty to keep)' : 'SSH Key Content'}</span>
					<textarea
						bind:value={value}
						rows={6}
						class="w-full px-3 py-2 text-sm font-mono bg-input border border-border rounded-sm text-text-primary placeholder:text-text-muted resize-y focus:ring-1 focus:ring-ring focus:outline-none"
						placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
						required={!editingId}
					></textarea>
				</label>
			{/if}
		{:else if credType === 'password'}
			<Input label={editingId ? 'New Password (leave empty to keep)' : 'Password'} type="password" bind:value={value} required={!editingId} />
			<Input label="Username" bind:value={username} placeholder="root" />
		{:else if credType === 'api_token'}
			<Input label={editingId ? 'New Token (leave empty to keep)' : 'API Token'} type="password" bind:value={value} required={!editingId} />
		{:else}
			<Input label={editingId ? 'New Certificate Path (leave empty to keep)' : 'Certificate Path'} bind:value={value} required={!editingId} />
		{/if}

		<Input label="Description" bind:value={description} placeholder="Optional description" />
	</form>

	{#snippet footer()}
		<Button variant="secondary" onclick={() => showModal = false}>Cancel</Button>
		<Button onclick={handleSubmit}>{editingId ? 'Save' : 'Create'}</Button>
	{/snippet}
</Modal>
