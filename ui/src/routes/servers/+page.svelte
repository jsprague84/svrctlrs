<script lang="ts">
	import { onMount } from 'svelte';
	import { Server, Plus, Pencil, Trash2, Wifi, WifiOff } from 'lucide-svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Input from '$lib/components/ui/Input.svelte';
	import Select from '$lib/components/ui/Select.svelte';
	import Modal from '$lib/components/ui/Modal.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';
	import * as serversState from '$lib/state/servers.svelte.js';
	import * as credState from '$lib/state/credentials.svelte.js';
	import * as toast from '$lib/state/toast.svelte.js';
	import type { CreateServer, UpdateServer, TestConnectionResult } from '$lib/types/index.js';

	let showModal = $state(false);
	let editingId = $state<number | null>(null);
	let testing = $state<number | null>(null);
	let testResult = $state<TestConnectionResult | null>(null);

	// Form fields
	let name = $state('');
	let hostname = $state('');
	let port = $state(22);
	let username = $state('');
	let credentialId = $state<string>('');
	let description = $state('');
	let isLocal = $state(false);
	let enabled = $state(true);

	let servers = $derived(serversState.getServers());
	let loading = $derived(serversState.isLoading());
	let credentials = $derived(credState.getCredentials());

	onMount(() => {
		serversState.loadServers();
		credState.loadCredentials();
	});

	function resetForm() {
		name = ''; hostname = ''; port = 22; username = '';
		credentialId = ''; description = ''; isLocal = false; enabled = true;
		editingId = null;
	}

	function openCreate() {
		resetForm();
		showModal = true;
	}

	function openEdit(server: typeof servers[0]) {
		editingId = server.id;
		name = server.name;
		hostname = server.hostname ?? '';
		port = server.port;
		username = server.username ?? '';
		credentialId = server.credential_id ? String(server.credential_id) : '';
		description = server.description ?? '';
		isLocal = server.is_local;
		enabled = server.enabled;
		showModal = true;
	}

	async function handleSubmit() {
		try {
			if (editingId) {
				const input: UpdateServer = { name, hostname: hostname || undefined, port, username: username || undefined, credential_id: credentialId ? Number(credentialId) : undefined, description: description || undefined, enabled };
				await serversState.updateServer(editingId, input);
				toast.success('Server updated');
			} else {
				const input: CreateServer = { name, hostname: hostname || undefined, port, username: username || undefined, credential_id: credentialId ? Number(credentialId) : undefined, description: description || undefined, is_local: isLocal, enabled };
				await serversState.createServer(input);
				toast.success('Server created');
			}
			showModal = false;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'Operation failed');
		}
	}

	async function handleDelete(id: number) {
		if (!confirm('Delete this server?')) return;
		try {
			await serversState.deleteServer(id);
			toast.success('Server deleted');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'Delete failed');
		}
	}

	async function handleTest(id: number) {
		testing = id;
		testResult = null;
		try {
			testResult = await serversState.testConnection(id);
			if (testResult.success) {
				const latency = testResult.latency_ms != null ? ` (${testResult.latency_ms}ms)` : '';
				toast.success(`Connected${latency}`);
			} else {
				toast.error(testResult.message);
			}
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'Connection test failed');
		} finally {
			testing = null;
		}
	}
</script>

<div class="flex flex-col h-full">
	<div class="flex items-center justify-between px-6 py-4 border-b border-border">
		<div class="flex items-center gap-3">
			<Server class="w-5 h-5 text-accent" />
			<h1 class="text-lg font-semibold text-text-primary">Servers</h1>
			<Badge>{servers.length}</Badge>
		</div>
		<Button onclick={openCreate}><Plus class="w-4 h-4" /> Add Server</Button>
	</div>

	<div class="flex-1 overflow-y-auto">
		{#if loading}
			<div class="p-6 text-text-muted">Loading...</div>
		{:else if servers.length === 0}
			<div class="p-6 text-center text-text-muted">
				<p>No servers configured yet.</p>
				<Button variant="secondary" class="mt-3" onclick={openCreate}>Add your first server</Button>
			</div>
		{:else}
			<table class="w-full text-sm">
				<thead>
					<tr class="border-b border-border text-left text-text-muted">
						<th class="px-4 py-2 font-medium">Name</th>
						<th class="px-4 py-2 font-medium">Host</th>
						<th class="px-4 py-2 font-medium">User</th>
						<th class="px-4 py-2 font-medium">Status</th>
						<th class="px-4 py-2 font-medium text-right">Actions</th>
					</tr>
				</thead>
				<tbody>
					{#each servers as server (server.id)}
						<tr class="border-b border-border/50 hover:bg-surface-raised/30">
							<td class="px-4 py-2 text-text-primary font-medium">
								{server.name}
								{#if server.is_local}
									<Badge variant="info">local</Badge>
								{/if}
							</td>
							<td class="px-4 py-2 text-text-secondary font-mono text-xs">
								{server.hostname ?? '—'}:{server.port}
							</td>
							<td class="px-4 py-2 text-text-secondary">{server.username ?? '—'}</td>
							<td class="px-4 py-2">
								{#if server.enabled}
									<Badge variant="success">Enabled</Badge>
								{:else}
									<Badge variant="warning">Disabled</Badge>
								{/if}
							</td>
							<td class="px-4 py-2 text-right">
								<div class="flex items-center justify-end gap-1">
									<Button variant="ghost" size="sm" onclick={() => handleTest(server.id)} disabled={testing === server.id}>
										{#if testing === server.id}
											<span class="w-3 h-3 border border-current border-t-transparent rounded-full animate-spin"></span>
										{:else}
											<Wifi class="w-3.5 h-3.5" />
										{/if}
										Test
									</Button>
									<Button variant="ghost" size="sm" onclick={() => openEdit(server)}>
										<Pencil class="w-3.5 h-3.5" />
									</Button>
									<Button variant="ghost" size="sm" onclick={() => handleDelete(server.id)}>
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

<Modal open={showModal} title={editingId ? 'Edit Server' : 'Add Server'} onClose={() => showModal = false}>
	<form class="flex flex-col gap-3" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
		<Input label="Name" bind:value={name} required placeholder="production-web-01" />
		<div class="flex items-center gap-2">
			<label class="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
				<input type="checkbox" bind:checked={isLocal} class="rounded" />
				Local server
			</label>
		</div>
		{#if !isLocal}
			<Input label="Hostname" bind:value={hostname} placeholder="192.168.1.10" />
			<div class="grid grid-cols-2 gap-3">
				<Input label="Port" type="number" bind:value={port} min={1} max={65535} />
				<Input label="Username" bind:value={username} placeholder="root" />
			</div>
			<Select label="Credential" bind:value={credentialId}>
				<option value="">None</option>
				{#each credentials as cred}
					<option value={String(cred.id)}>{cred.name} ({cred.credential_type})</option>
				{/each}
			</Select>
		{/if}
		<Input label="Description" bind:value={description} placeholder="Optional description" />
		<label class="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
			<input type="checkbox" bind:checked={enabled} class="rounded" />
			Enabled
		</label>
	</form>

	{#snippet footer()}
		<Button variant="secondary" onclick={() => showModal = false}>Cancel</Button>
		<Button onclick={handleSubmit}>{editingId ? 'Save' : 'Create'}</Button>
	{/snippet}
</Modal>
