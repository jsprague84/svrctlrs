<script lang="ts">
	import { onMount } from 'svelte';
	import {
		Server as ServerIcon,
		Plus,
		Pencil,
		Trash2,
		Wifi,
		KeyRound,
		ShieldCheck,
		TerminalSquare,
		Search
	} from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import * as serversApi from '#lib/api/servers.js';
	import { serverStatus } from '#lib/utils/serverStatus.js';
	import Button from '#lib/components/ui/Button.svelte';
	import Input from '#lib/components/ui/Input.svelte';
	import Select from '#lib/components/ui/Select.svelte';
	import Modal from '#lib/components/ui/Modal.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import AuthorizeKeyModal from '#lib/components/servers/AuthorizeKeyModal.svelte';
	import * as serversState from '#lib/state/servers.svelte.js';
	import * as credState from '#lib/state/credentials.svelte.js';
	import * as terminalState from '#lib/state/terminal.svelte.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { CREDENTIAL_TYPE_LABELS } from '#lib/types/credential.js';
	import type {
		Server,
		CreateServer,
		UpdateServer,
		TestConnectionResult,
		HostKey
	} from '#lib/types/index.js';

	let showModal = $state(false);
	let editingId = $state<number | null>(null);
	let saving = $state(false);
	let testing = $state<number | null>(null);
	/** Last Test result per server, shown inline in the row */
	let testResults = $state<Record<number, TestConnectionResult>>({});
	let authServer = $state<Server | null>(null);
	// Host keys modal
	let keysServer = $state<{ id: number; name: string } | null>(null);
	let hostKeys = $state<HostKey[]>([]);
	let keysLoading = $state(false);
	let search = $state('');

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
	let serversError = $derived(serversState.getError());
	let credentials = $derived(credState.getCredentials());
	let credentialName = $derived(
		(id: number | null) => credentials.find((c) => c.id === id)?.name ?? null
	);
	let shown = $derived.by(() => {
		const q = search.trim().toLowerCase();
		const list = q
			? servers.filter((s) =>
					[s.name, s.hostname ?? '', s.username ?? '', s.description ?? '']
						.join(' ')
						.toLowerCase()
						.includes(q)
				)
			: servers;
		return [...list].sort((a, b) => a.name.localeCompare(b.name));
	});
	let formError = $derived(
		!isLocal && !hostname.trim() && name.trim() ? 'A remote server needs a hostname' : ''
	);

	onMount(() => {
		serversState.loadServers();
		credState.loadCredentials();
	});

	function resetForm() {
		name = '';
		hostname = '';
		port = 22;
		username = '';
		credentialId = '';
		description = '';
		isLocal = false;
		enabled = true;
		editingId = null;
	}

	function openCreate() {
		resetForm();
		showModal = true;
	}

	function openEdit(server: Server) {
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
		if (saving || formError) return;
		saving = true;
		try {
			if (editingId) {
				// Empty strings and null clear a field; the API keeps a field only when it is absent
				const input: UpdateServer = {
					name: name.trim(),
					hostname: isLocal ? undefined : hostname.trim(),
					port,
					username: isLocal ? undefined : username.trim(),
					credential_id: isLocal ? undefined : credentialId ? Number(credentialId) : null,
					description: description.trim(),
					enabled
				};
				await serversState.updateServer(editingId, input);
				toast.success('Server updated');
			} else {
				const input: CreateServer = {
					name: name.trim(),
					hostname: isLocal ? undefined : hostname.trim() || undefined,
					port,
					username: isLocal ? undefined : username.trim() || undefined,
					credential_id: isLocal ? undefined : credentialId ? Number(credentialId) : undefined,
					description: description.trim() || undefined,
					is_local: isLocal,
					enabled
				};
				const id = await serversState.createServer(input);
				toast.success('Server created');
				// The natural next step for a remote server without a key: install one
				const created = serversState.getServers().find((s) => s.id === id);
				if (created && !created.is_local && !created.credential_id) {
					showModal = false;
					authServer = created;
					return;
				}
			}
			showModal = false;
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Operation failed'));
		} finally {
			saving = false;
		}
	}

	async function handleDelete(server: Server) {
		const ok = await confirmDialog({
			title: `Delete ${server.name}?`,
			message: `The server entry${server.hostname ? ` (${server.hostname})` : ''} and its stored host keys are removed. Open terminals to it are closed. Credentials are kept.`,
			confirmLabel: 'Delete',
			destructive: true
		});
		if (!ok) return;
		try {
			await serversState.deleteServer(server.id);
			toast.success('Server deleted');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Delete failed'));
		}
	}

	function connect(server: Server) {
		if (terminalState.connectToServer(server.id, server.name)) {
			goto(resolve('/'));
		} else {
			toast.error('Tab limit reached — close a terminal first');
		}
	}

	async function openHostKeys(server: { id: number; name: string }) {
		keysServer = server;
		hostKeys = [];
		keysLoading = true;
		try {
			hostKeys = await serversApi.listHostKeys(server.id);
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to load host keys'));
		} finally {
			keysLoading = false;
		}
	}

	async function forgetHostKeys() {
		if (!keysServer) return;
		const ok = await confirmDialog({
			title: `Forget host keys for ${keysServer.name}?`,
			message:
				'The next connection will trust whatever key the server presents. Only do this when the host key legitimately changed (reinstall, new SSH keys).',
			confirmLabel: 'Forget keys',
			destructive: true
		});
		if (!ok) return;
		try {
			const removed = await serversApi.forgetHostKeys(keysServer.id);
			toast.success(`Forgot ${removed} host key${removed === 1 ? '' : 's'}`);
			hostKeys = [];
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to forget host keys'));
		}
	}

	async function handleTest(id: number) {
		testing = id;
		try {
			const result = await serversState.testConnection(id);
			testResults = { ...testResults, [id]: result };
			if (result.success) {
				toast.success(`Connected${result.latency_ms != null ? ` in ${result.latency_ms} ms` : ''}`);
			} else {
				toast.error(result.message);
			}
			await serversState.loadServers();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Connection test failed'));
		} finally {
			testing = null;
		}
	}

	/** A key should be installed: no credential, or a password one */
	function needsKey(server: Server): boolean {
		if (server.is_local || !server.enabled) return false;
		const cred = credentials.find((c) => c.id === server.credential_id);
		return !cred || cred.credential_type !== 'ssh_key';
	}
</script>

<div class="flex flex-col h-full">
	<div
		class="flex flex-wrap items-center justify-between gap-2 px-4 md:px-6 py-4 border-b border-border"
	>
		<div class="flex items-center gap-3">
			<ServerIcon class="w-5 h-5 text-accent" />
			<h1 class="text-lg font-semibold text-text-primary">Servers</h1>
			<Badge>{servers.length}</Badge>
		</div>
		<div class="flex items-center gap-2">
			{#if servers.length > 6}
				<div class="relative">
					<Search class="w-3.5 h-3.5 absolute left-2 top-1/2 -translate-y-1/2 text-text-muted" />
					<input
						type="search"
						bind:value={search}
						placeholder="Filter…"
						aria-label="Filter servers"
						class="pl-7 pr-2 py-1.5 text-sm bg-input border border-border rounded-sm text-text-primary w-40"
					/>
				</div>
			{/if}
			<Button onclick={openCreate}><Plus class="w-4 h-4" /> Add Server</Button>
		</div>
	</div>

	<div class="flex-1 overflow-y-auto p-4 md:p-6">
		{#if loading && servers.length === 0}
			<div class="text-text-muted text-sm py-8 text-center">Loading servers…</div>
		{:else if serversError}
			<div class="text-center py-8">
				<p class="text-error">{serversError}</p>
				<Button variant="secondary" class="mt-3" onclick={() => serversState.loadServers()}
					>Retry</Button
				>
			</div>
		{:else if servers.length === 0}
			<div class="text-center text-text-muted py-8">
				<p>No servers yet.</p>
				<p class="text-xs mt-1">
					Add a server, then install an SSH key on it with <em>Authorize key</em>.
				</p>
				<Button class="mt-3" onclick={openCreate}
					><Plus class="w-4 h-4" /> Add your first server</Button
				>
			</div>
		{:else if shown.length === 0}
			<div class="text-center text-text-muted py-8 text-sm">Nothing matches “{search}”.</div>
		{:else}
			<ul class="flex flex-col gap-2">
				{#each shown as server (server.id)}
					{@const st = serverStatus(server)}
					{@const test = testResults[server.id]}
					{@const cred = credentialName(server.credential_id)}
					<li
						class="border border-border/50 rounded-lg px-3 md:px-4 py-3 hover:bg-surface-raised/30 flex flex-col md:flex-row md:items-center gap-2 md:gap-4"
					>
						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-2 min-w-0">
								<span
									class="w-2 h-2 rounded-full flex-shrink-0 {st.dot}"
									title={st.label}
									aria-label={st.label}
								></span>
								<span class="text-text-primary font-medium truncate">{server.name}</span>
								{#if server.is_local}
									<Badge variant="info">local</Badge>
								{:else if !server.enabled}
									<Badge variant="warning">disabled</Badge>
								{/if}
								{#if cred}
									<span
										class="hidden md:inline-flex items-center gap-1 text-[11px] text-text-muted"
									>
										<KeyRound class="w-3 h-3" />
										{cred}
									</span>
								{:else if needsKey(server)}
									<Badge variant="warning">no key</Badge>
								{/if}
							</div>
							<div class="mt-0.5 text-xs text-text-secondary font-mono truncate">
								{#if server.is_local}
									inside the SvrCtlRS container
								{:else}
									{server.username ?? 'root'}@{server.hostname ?? '—'}{server.port !== 22
										? `:${server.port}`
										: ''}
								{/if}
								{#if server.description}
									<span class="font-sans text-text-muted"> · {server.description}</span>
								{/if}
							</div>
							{#if test}
								<div class="mt-0.5 text-xs {test.success ? 'text-success' : 'text-error'} truncate">
									{test.success
										? `Test OK${test.latency_ms != null ? ` · ${test.latency_ms} ms` : ''}`
										: `Test failed: ${test.message}`}
								</div>
							{:else if !server.is_local}
								<div
									class="mt-0.5 text-xs truncate {st.kind === 'unreachable'
										? 'text-error'
										: 'text-text-muted'}"
								>
									{st.label}
								</div>
							{/if}
						</div>

						<div class="flex items-center gap-1 flex-wrap md:flex-nowrap md:flex-shrink-0">
							<Button
								size="sm"
								class="min-h-9 md:min-h-0"
								onclick={() => connect(server)}
								disabled={!server.enabled}
							>
								<TerminalSquare class="w-3.5 h-3.5" /> Connect
							</Button>
							{#if !server.is_local}
								<Button
									variant="ghost"
									size="sm"
									class="min-h-9 min-w-9 md:min-h-0 md:min-w-0"
									onclick={() => handleTest(server.id)}
									disabled={testing === server.id}
									aria-label="Test connection"
									title="Test connection"
								>
									{#if testing === server.id}
										<span
											class="w-3.5 h-3.5 border border-current border-t-transparent rounded-full animate-spin"
										></span>
									{:else}
										<Wifi class="w-3.5 h-3.5" />
									{/if}
									<span class="hidden lg:inline">Test</span>
								</Button>
								<Button
									variant={needsKey(server) ? 'secondary' : 'ghost'}
									size="sm"
									class="min-h-9 min-w-9 md:min-h-0 md:min-w-0 {needsKey(server)
										? 'text-warning'
										: ''}"
									onclick={() => (authServer = server)}
									aria-label="Authorize an SSH key on this server"
									title="Install an SSH key on this server"
								>
									<ShieldCheck class="w-3.5 h-3.5" />
									<span class="hidden lg:inline">Authorize key</span>
								</Button>
								<Button
									variant="ghost"
									size="sm"
									class="min-h-9 min-w-9 md:min-h-0 md:min-w-0"
									onclick={() => openHostKeys(server)}
									aria-label="Stored host keys"
									title="Stored SSH host keys"
								>
									<KeyRound class="w-3.5 h-3.5" />
									<span class="hidden lg:inline">Keys</span>
								</Button>
							{/if}
							<Button
								variant="ghost"
								size="sm"
								class="min-h-9 min-w-9 md:min-h-0 md:min-w-0"
								onclick={() => openEdit(server)}
								aria-label="Edit server"
								title="Edit"
							>
								<Pencil class="w-3.5 h-3.5" />
								<span class="hidden lg:inline">Edit</span>
							</Button>
							<Button
								variant="ghost"
								size="sm"
								class="min-h-9 min-w-9 md:min-h-0 md:min-w-0"
								onclick={() => handleDelete(server)}
								aria-label="Delete server"
								title="Delete"
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
	title={editingId ? 'Edit Server' : 'Add Server'}
	onClose={() => (showModal = false)}
>
	<form
		id="server-form"
		class="flex flex-col gap-3"
		onsubmit={(e) => {
			e.preventDefault();
			handleSubmit();
		}}
	>
		<Input label="Name" bind:value={name} required placeholder="production-web-01" />
		{#if isLocal}
			<p class="text-xs text-text-muted">
				This is a legacy local server. Local shells were retired: SvrCtlRS connects over SSH only.
				Give it a hostname (localhost works) and a credential, or delete it.
			</p>
		{/if}
		{#if !isLocal}
			<Input
				label="Hostname"
				bind:value={hostname}
				placeholder="192.168.1.10 or web-01.example.net"
				required
				error={formError}
			/>
			<div class="grid grid-cols-2 gap-3">
				<Input label="Port" type="number" bind:value={port} min={1} max={65535} />
				<Input label="Username" bind:value={username} placeholder="root" />
			</div>
			<Select label="Credential" bind:value={credentialId}>
				<option value="">None (server's default keys)</option>
				{#each credentials as cred (cred.id)}
					<option value={String(cred.id)}
						>{cred.name} — {CREDENTIAL_TYPE_LABELS[cred.credential_type]}</option
					>
				{/each}
			</Select>
			{#if !credentialId}
				<p class="text-[11px] text-text-muted">
					Without a credential, the SvrCtlRS server's own <code>~/.ssh/id_*</code> keys are tried.
					Leave this empty for now and use <em>Authorize key</em> after creating.
				</p>
			{/if}
		{/if}
		<Input label="Description" bind:value={description} placeholder="Optional" />
		<label class="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
			<input type="checkbox" bind:checked={enabled} class="rounded" />
			Enabled
		</label>
	</form>

	{#snippet footer()}
		<Button variant="secondary" onclick={() => (showModal = false)}>Cancel</Button>
		<Button type="submit" form="server-form" disabled={saving || !!formError}>
			{saving ? 'Saving…' : editingId ? 'Save' : 'Create'}
		</Button>
	{/snippet}
</Modal>

<Modal
	open={keysServer !== null}
	title={`Host keys — ${keysServer?.name ?? ''}`}
	onClose={() => (keysServer = null)}
>
	<p class="text-xs text-text-muted mb-3">
		Keys trusted on first contact. A connection is refused when the server presents a key that
		matches none of these. If the server's key legitimately changed, forget them and connect again.
	</p>
	{#if keysLoading}
		<div class="text-text-muted text-sm">Loading…</div>
	{:else if hostKeys.length === 0}
		<div class="text-text-muted text-sm">
			No host key stored yet — the first connection will record one.
		</div>
	{:else}
		<ul class="flex flex-col gap-2">
			{#each hostKeys as key (key.id)}
				<li class="border border-border/50 rounded-sm px-3 py-2">
					<div class="flex items-center gap-2">
						<Badge variant="info">{key.key_type}</Badge>
						<span class="font-mono text-xs text-text-primary break-all">{key.fingerprint}</span>
					</div>
					<div class="text-[11px] text-text-muted mt-1">
						first seen {new Date(key.first_seen_at).toLocaleString()} · last seen {new Date(
							key.last_seen_at
						).toLocaleString()}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
	{#snippet footer()}
		<Button variant="secondary" onclick={() => (keysServer = null)}>Close</Button>
		<Button variant="destructive" onclick={forgetHostKeys} disabled={hostKeys.length === 0}
			>Forget all keys</Button
		>
	{/snippet}
</Modal>

<AuthorizeKeyModal server={authServer} onClose={() => (authServer = null)} />
