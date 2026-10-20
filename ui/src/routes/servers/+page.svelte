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
		Search,
		FileInput,
		MonitorPlay,
		Stethoscope,
		RotateCw
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
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import ActionMenu, { type MenuItem } from '#lib/components/ui/ActionMenu.svelte';
	import AuthorizeKeyModal from '#lib/components/servers/AuthorizeKeyModal.svelte';
	import DiagnoseModal from '#lib/components/servers/DiagnoseModal.svelte';
	import * as serversState from '#lib/state/servers.svelte.js';
	import * as credState from '#lib/state/credentials.svelte.js';
	import * as terminalState from '#lib/state/terminal.svelte.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { allTags, matchesTag, parseTagInput } from '#lib/utils/tags.js';
	import TagChips from '#lib/components/servers/TagChips.svelte';
	import Textarea from '#lib/components/ui/Textarea.svelte';
	import Toggle from '#lib/components/ui/Toggle.svelte';
	import { CREDENTIAL_TYPE_LABELS } from '#lib/types/credential.js';
	import type {
		Server,
		CreateServer,
		UpdateServer,
		TestConnectionResult,
		HostKey,
		ImportResult
	} from '#lib/types/index.js';

	let showModal = $state(false);
	let editingId = $state<number | null>(null);
	let saving = $state(false);
	let testing = $state<number | null>(null);
	/** Last Test result per server, shown inline in the row */
	let testResults = $state<Record<number, TestConnectionResult>>({});
	let authServer = $state<Server | null>(null);
	let diagnoseServer = $state<Server | null>(null);
	let rotating = $state<number | null>(null);
	// Host keys modal
	let keysServer = $state<{ id: number; name: string } | null>(null);
	let hostKeys = $state<HostKey[]>([]);
	let keysLoading = $state(false);
	let search = $state('');
	let tagFilter = $state<string | null>(null);
	// Import modal
	let showImport = $state(false);
	let importText = $state('');
	let importCredentialId = $state<string>('');
	let importTags = $state('');
	let importing = $state(false);
	let importResult = $state<ImportResult | null>(null);
	const IMPORT_PLACEHOLDER =
		'Host web\n  HostName web.example.net\n  User deploy\n  Port 2222\n\nHost db\n  ProxyJump web';

	// Form fields
	let name = $state('');
	let hostname = $state('');
	let port = $state(22);
	let username = $state('');
	let credentialId = $state<string>('');
	let viaServerId = $state<string>('');
	let tagsText = $state('');
	let startupCommand = $state('');
	let recordSessions = $state(false);
	let description = $state('');
	let enabled = $state(true);

	let servers = $derived(serversState.getServers());
	let loading = $derived(serversState.isLoading());
	let serversError = $derived(serversState.getError());
	let credentials = $derived(credState.getCredentials());
	let credentialName = $derived(
		(id: number | null) => credentials.find((c) => c.id === id)?.name ?? null
	);
	let serverName = $derived((id: number | null) => servers.find((s) => s.id === id)?.name ?? null);
	/** Candidates for the jump-host select: every other server */
	let jumpCandidates = $derived(servers.filter((s) => s.id !== editingId));
	let tags = $derived(allTags(servers));
	let shown = $derived.by(() => {
		const q = search.trim().toLowerCase();
		const list = servers.filter(
			(s) =>
				matchesTag(s, tagFilter) &&
				(!q ||
					[s.name, s.hostname, s.username ?? '', s.description ?? '', ...s.tags]
						.join(' ')
						.toLowerCase()
						.includes(q))
		);
		return [...list].sort((a, b) => a.name.localeCompare(b.name));
	});
	let formError = $derived(!hostname.trim() && name.trim() ? 'A server needs a hostname' : '');

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
		viaServerId = '';
		tagsText = '';
		startupCommand = '';
		recordSessions = false;
		description = '';
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
		hostname = server.hostname;
		port = server.port;
		username = server.username ?? '';
		credentialId = server.credential_id ? String(server.credential_id) : '';
		viaServerId = server.via_server_id ? String(server.via_server_id) : '';
		tagsText = server.tags.join(', ');
		startupCommand = server.startup_command ?? '';
		recordSessions = server.record_sessions;
		description = server.description ?? '';
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
					hostname: hostname.trim(),
					port,
					username: username.trim(),
					credential_id: credentialId ? Number(credentialId) : null,
					via_server_id: viaServerId ? Number(viaServerId) : null,
					tags: parseTagInput(tagsText),
					startup_command: startupCommand.trim(),
					record_sessions: recordSessions,
					description: description.trim(),
					enabled
				};
				await serversState.updateServer(editingId, input);
				toast.success('Server updated');
			} else {
				const input: CreateServer = {
					name: name.trim(),
					hostname: hostname.trim(),
					port,
					username: username.trim() || undefined,
					credential_id: credentialId ? Number(credentialId) : undefined,
					via_server_id: viaServerId ? Number(viaServerId) : undefined,
					tags: parseTagInput(tagsText),
					startup_command: startupCommand.trim() || undefined,
					record_sessions: recordSessions,
					description: description.trim() || undefined,
					enabled
				};
				const id = await serversState.createServer(input);
				toast.success('Server created');
				// The natural next step for a remote server without a key: install one
				const created = serversState.getServers().find((s) => s.id === id);
				if (created && !created.credential_id) {
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
			message: `The server entry (${server.hostname}) and its stored host keys are removed. Open terminals to it are closed. Credentials are kept.`,
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

	/** Trust the key recorded on first contact, from the Keys modal or a failed Test. */
	async function trustHostKey(serverId: number, fingerprint: string): Promise<boolean> {
		try {
			await serversApi.trustHostKey(serverId, fingerprint);
			toast.success('Host key trusted');
			if (keysServer?.id === serverId) hostKeys = await serversApi.listHostKeys(serverId);
			return true;
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Could not trust the host key'));
			return false;
		}
	}

	async function trustAndRetest(test: TestConnectionResult) {
		if (!test.details) return;
		if (await trustHostKey(test.details.server_id, test.details.fingerprint)) {
			await handleTest(test.server_id);
		}
	}

	async function forgetHostKeys() {
		if (!keysServer) return;
		const ok = await confirmDialog({
			title: `Forget host keys for ${keysServer.name}?`,
			message:
				'The next connection will show the key the server presents and ask you to confirm it again. Do this when the host key legitimately changed (reinstall, new SSH keys).',
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

	function openImport() {
		importText = '';
		importCredentialId = '';
		importTags = '';
		importResult = null;
		showImport = true;
	}

	async function handleImport() {
		if (importing || !importText.trim()) return;
		importing = true;
		try {
			importResult = await serversApi.importServers({
				config: importText,
				credential_id: importCredentialId ? Number(importCredentialId) : undefined,
				tags: parseTagInput(importTags)
			});
			await serversState.loadServers();
			const n = importResult.created.length;
			toast.success(`Imported ${n} server${n === 1 ? '' : 's'}`);
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Import failed'));
		} finally {
			importing = false;
		}
	}

	/** The row's actions behind the phone's "⋯" menu, in the order of the desktop strip */
	function rowActions(server: Server): MenuItem[] {
		return [
			{
				label: 'Test connection',
				icon: Wifi,
				onSelect: () => handleTest(server.id),
				disabled: testing === server.id
			},
			{ label: 'Diagnose', icon: Stethoscope, onSelect: () => (diagnoseServer = server) },
			{
				label: 'Authorize key',
				icon: ShieldCheck,
				onSelect: () => (authServer = server),
				attention: needsKey(server)
			},
			...(canRotate(server)
				? [
						{
							label: 'Rotate key',
							icon: RotateCw,
							onSelect: () => rotateKey(server),
							disabled: rotating === server.id
						}
					]
				: []),
			{ label: 'Host keys', icon: KeyRound, onSelect: () => openHostKeys(server) },
			...(server.record_sessions
				? [{ label: 'Recordings', icon: MonitorPlay, onSelect: () => openRecordings(server) }]
				: []),
			{ label: 'Edit', icon: Pencil, onSelect: () => openEdit(server) },
			{ label: 'Delete', icon: Trash2, onSelect: () => handleDelete(server), destructive: true }
		];
	}

	function openRecordings(server: Server) {
		goto(`${resolve('/recordings')}?server=${server.id}`);
	}

	/** A key should be installed: no credential, or a password one */
	/** A stored key (not a path) can be rotated */
	function canRotate(server: Server): boolean {
		const cred = credentials.find((c) => c.id === server.credential_id);
		return !!cred && cred.credential_type === 'ssh_key' && cred.key_source === 'key';
	}

	async function rotateKey(server: Server) {
		const ok = await confirmDialog({
			title: `Rotate the key of ${server.name}?`,
			message:
				'SvrCtlRS generates a new key, installs it with the current one, checks that it logs in, switches the server to it and removes the old public key from the server. The old credential stays under Credentials.',
			confirmLabel: 'Rotate key'
		});
		if (!ok) return;
		rotating = server.id;
		try {
			const result = await serversApi.rotateKey(server.id);
			const failed = result.steps.find((s) => s.status === 'failed');
			if (result.done) toast.success(`New key on ${server.name}. ${result.next ?? ''}`.trim());
			else
				toast.error(
					failed ? `${failed.detail}. ${result.next ?? ''}` : (result.next ?? 'Not done')
				);
			await Promise.all([serversState.loadServers(), credState.loadCredentials()]);
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to rotate the key'));
		} finally {
			rotating = null;
		}
	}

	function needsKey(server: Server): boolean {
		if (!server.enabled) return false;
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
			<Button variant="secondary" onclick={openImport} title="Create servers from ~/.ssh/config"
				><FileInput class="w-4 h-4" /> <span class="hidden sm:inline">Import</span></Button
			>
			<Button onclick={openCreate}><Plus class="w-4 h-4" /> Add Server</Button>
		</div>
	</div>
	{#if tags.length > 0}
		<div class="px-4 md:px-6 py-2 border-b border-border/60">
			<TagChips {tags} selected={tagFilter} onSelect={(t) => (tagFilter = t)} />
		</div>
	{/if}

	<div class="flex-1 overflow-y-auto p-4 md:p-6">
		{#if loading && servers.length === 0}
			<StateBlock kind="loading" title="Loading servers…" />
		{:else if serversError}
			<StateBlock kind="error" title={serversError} onRetry={() => serversState.loadServers()} />
		{:else if servers.length === 0}
			<StateBlock
				kind="empty"
				icon={ServerIcon}
				title="No servers yet"
				hint="Add a server (or import your ~/.ssh/config), then install an SSH key on it with Authorize key."
			>
				<Button onclick={openCreate}><Plus class="w-4 h-4" /> Add your first server</Button>
				<Button variant="secondary" onclick={openImport}
					><FileInput class="w-4 h-4" /> Import</Button
				>
			</StateBlock>
		{:else if shown.length === 0}
			<StateBlock
				kind="empty"
				icon={Search}
				title={search.trim() ? `Nothing matches “${search}”` : `No server is tagged ${tagFilter}`}
			>
				<Button
					variant="secondary"
					onclick={() => {
						search = '';
						tagFilter = null;
					}}>Show all servers</Button
				>
			</StateBlock>
		{:else}
			<ul class="flex flex-col gap-2">
				{#each shown as server (server.id)}
					{@const st = serverStatus(server)}
					{@const test = testResults[server.id]}
					{@const cred = credentialName(server.credential_id)}
					<li
						class="border border-border/50 rounded-lg pl-3 pr-1.5 md:px-4 py-2.5 md:py-3 hover:bg-surface-raised/30 flex items-center gap-2 md:gap-4"
					>
						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-2 min-w-0">
								<span
									class="w-2 h-2 rounded-full flex-shrink-0 {st.dot}"
									title={st.label}
									aria-label={st.label}
								></span>
								<span class="text-text-primary font-medium truncate">{server.name}</span>
								{#if !server.enabled}
									<Badge variant="warning">disabled</Badge>
								{/if}
								{#if server.record_sessions}
									<span class="md:hidden" title="Shells opened here are recorded"
										><Badge variant="error">rec</Badge></span
									>
									<button
										type="button"
										class="hidden md:inline-flex"
										onclick={() => openRecordings(server)}
										title="Shells opened here are recorded; show the recordings"
										aria-label="Recorded server; show its recordings"
										><Badge variant="error">rec</Badge></button
									>
								{/if}
								{#each server.tags as tag (tag)}
									<button
										type="button"
										class="hidden md:inline-flex"
										onclick={() => (tagFilter = tag)}
										title="Show servers tagged {tag}"><Badge>{tag}</Badge></button
									>
								{/each}
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
								{server.username ?? 'root'}@{server.hostname}{server.port !== 22
									? `:${server.port}`
									: ''}
								{#if server.via_server_id !== null}
									<span class="font-sans text-text-muted">
										· via {serverName(server.via_server_id) ?? `#${server.via_server_id}`}</span
									>
								{/if}
								{#if server.description}
									<span class="font-sans text-text-muted"> · {server.description}</span>
								{/if}
							</div>
							{#if server.tags.length > 0}
								<div class="md:hidden mt-1 flex flex-wrap gap-1">
									{#each server.tags as tag (tag)}<Badge>{tag}</Badge>{/each}
								</div>
							{/if}
							{#if test && !test.success && test.code === 'HOST_KEY_UNCONFIRMED' && test.details}
								<div class="mt-1 text-xs text-warning flex flex-wrap items-center gap-x-2 gap-y-1">
									<span
										>Host key of {test.details.server_id === server.id
											? 'this server'
											: test.details.server_name} not confirmed:</span
									>
									<span class="font-mono text-text-primary break-all select-all"
										>{test.details.key_type} {test.details.fingerprint}</span
									>
									<Button size="sm" variant="secondary" onclick={() => trustAndRetest(test)}
										>Trust key</Button
									>
								</div>
							{:else if testing === server.id}
								<div class="mt-0.5 text-xs text-text-muted">Testing…</div>
							{:else if test}
								<div class="mt-0.5 text-xs {test.success ? 'text-success' : 'text-error'} truncate">
									{test.success
										? `Test OK${test.latency_ms != null ? ` · ${test.latency_ms} ms` : ''}`
										: `Test failed: ${test.message}`}
								</div>
							{:else}
								<div
									class="mt-0.5 text-xs truncate {st.kind === 'unreachable'
										? 'text-error'
										: 'text-text-muted'}"
								>
									{st.label}
								</div>
							{/if}
						</div>

						<!-- Phone: Connect plus a menu; the strip below needs a wider screen -->
						<div class="flex md:hidden items-center gap-0.5 flex-shrink-0">
							<Button
								size="sm"
								class="min-h-11 px-3"
								onclick={() => connect(server)}
								disabled={!server.enabled}
							>
								<TerminalSquare class="w-4 h-4" /> Connect
							</Button>
							<ActionMenu label="Actions for {server.name}" items={rowActions(server)} />
						</div>
						<div class="hidden md:flex items-center gap-1 flex-nowrap flex-shrink-0">
							<Button size="sm" onclick={() => connect(server)} disabled={!server.enabled}>
								<TerminalSquare class="w-3.5 h-3.5" /> Connect
							</Button>
							<Button
								variant="ghost"
								size="sm"
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
									<span class="hidden lg:inline">Test</span>
								{/if}
							</Button>
							<Button
								variant="ghost"
								size="sm"
								onclick={() => (diagnoseServer = server)}
								aria-label="Diagnose the connection"
								title="Why it can or cannot be reached, step by step"
							>
								<Stethoscope class="w-3.5 h-3.5" />
								<span class="hidden lg:inline">Diagnose</span>
							</Button>
							<Button
								variant={needsKey(server) ? 'secondary' : 'ghost'}
								size="sm"
								class={needsKey(server) ? 'text-warning' : ''}
								onclick={() => (authServer = server)}
								aria-label="Authorize an SSH key on this server"
								title="Install an SSH key on this server"
							>
								<ShieldCheck class="w-3.5 h-3.5" />
								<span class="hidden lg:inline">Authorize key</span>
							</Button>
							{#if canRotate(server)}
								<Button
									variant="ghost"
									size="sm"
									onclick={() => rotateKey(server)}
									disabled={rotating === server.id}
									aria-label="Rotate the SSH key"
									title="Replace the server's key with a new one"
								>
									<RotateCw class="w-3.5 h-3.5 {rotating === server.id ? 'animate-spin' : ''}" />
									<span class="hidden xl:inline">Rotate</span>
								</Button>
							{/if}
							<Button
								variant="ghost"
								size="sm"
								onclick={() => openHostKeys(server)}
								aria-label="Stored host keys"
								title="Stored SSH host keys"
							>
								<KeyRound class="w-3.5 h-3.5" />
								<span class="hidden lg:inline">Keys</span>
							</Button>
							<Button
								variant="ghost"
								size="sm"
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
		<Select label="Connect through (jump host)" bind:value={viaServerId}>
			<option value="">Directly</option>
			{#each jumpCandidates as jump (jump.id)}
				<option value={String(jump.id)}>{jump.name} — {jump.hostname}</option>
			{/each}
		</Select>
		{#if viaServerId}
			<p class="text-[11px] text-text-muted">
				The hostname above is resolved from the jump host, so a private address on its network
				works. Both host keys are checked and both servers need a working credential.
			</p>
		{/if}
		<Input
			label="Tags"
			bind:value={tagsText}
			placeholder="prod, home, db"
			hint="Comma separated; used to filter the server lists"
		/>
		<Input
			label="Startup command"
			bind:value={startupCommand}
			placeholder="tmux new -A -s main"
			class="font-mono"
			hint="Typed into every new shell on this server right after it starts. tmux or screen here gives you sessions that outlive the app's 15-minute grace period."
		/>
		<Toggle
			bind:checked={recordSessions}
			label="Record sessions"
			description="Every shell opened on this server from now on is recorded for the person who opens it: what the terminal shows, never what is typed. Each person sees only their own recordings, under Recordings."
		/>
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
		The first connection records the server's key and waits until you confirm its fingerprint
		(compare with <code class="font-mono">ssh-keygen -lf /etc/ssh/ssh_host_*_key.pub</code> on the host).
		Afterwards a connection is refused when the server presents a key that matches none of the trusted
		ones. If the server's key legitimately changed, forget them and connect again.
	</p>
	{#if keysLoading}
		<div class="text-text-muted text-sm">Loading…</div>
	{:else if hostKeys.length === 0}
		<div class="text-text-muted text-sm">
			No host key recorded yet — the first connection will record one to confirm.
		</div>
	{:else}
		<ul class="flex flex-col gap-2">
			{#each hostKeys as key (key.id)}
				<li class="border border-border/50 rounded-sm px-3 py-2">
					<div class="flex items-center gap-2 flex-wrap">
						<Badge variant="info">{key.key_type}</Badge>
						{#if key.trusted}
							<Badge variant="success">trusted</Badge>
						{:else}
							<Badge variant="warning">unconfirmed</Badge>
						{/if}
						<span class="font-mono text-xs text-text-primary break-all select-all"
							>{key.fingerprint}</span
						>
						{#if !key.trusted && keysServer}
							<Button
								size="sm"
								variant="secondary"
								onclick={() => keysServer && trustHostKey(keysServer.id, key.fingerprint)}
								>Trust</Button
							>
						{/if}
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
<DiagnoseModal
	server={diagnoseServer}
	onClose={() => (diagnoseServer = null)}
	onDone={() => serversState.loadServers()}
/>

<Modal open={showImport} title="Import from ssh_config" onClose={() => (showImport = false)}>
	{#if importResult}
		<div class="flex flex-col gap-3 text-sm">
			<p class="text-text-secondary">
				Created {importResult.created.length}, skipped {importResult.skipped.length}.
			</p>
			{#if importResult.created.length > 0}
				<ul class="flex flex-col gap-1">
					{#each importResult.created as c (c.id)}
						<li class="font-mono text-xs text-text-primary">{c.name} → {c.hostname}</li>
					{/each}
				</ul>
			{/if}
			{#if importResult.notes.length > 0}
				<ul class="flex flex-col gap-1 text-xs text-text-secondary">
					{#each importResult.notes as note (note)}
						<li>{note}</li>
					{/each}
				</ul>
			{/if}
			{#if importResult.skipped.length > 0}
				<details class="text-xs text-text-muted">
					<summary class="cursor-pointer">Skipped</summary>
					<ul class="mt-1 flex flex-col gap-0.5">
						{#each importResult.skipped as sk (sk.name + sk.reason)}
							<li><span class="font-mono">{sk.name}</span> — {sk.reason}</li>
						{/each}
					</ul>
				</details>
			{/if}
			<p class="text-xs text-text-muted">
				Nothing was connected to yet: each new server asks for its host key on first contact, and a
				server without a credential can get one with <em>Authorize key</em>.
			</p>
		</div>
	{:else}
		<form
			id="import-form"
			class="flex flex-col gap-3"
			onsubmit={(e) => {
				e.preventDefault();
				handleImport();
			}}
		>
			<Textarea
				label="~/.ssh/config"
				bind:value={importText}
				mono
				rows={10}
				placeholder={IMPORT_PLACEHOLDER}
				hint="Host, HostName, Port, User and ProxyJump are read; wildcard hosts and Match blocks are skipped. Names that already exist are left alone."
			/>
			<Select label="Credential for the imported servers" bind:value={importCredentialId}>
				<option value="">None (set later or use Authorize key)</option>
				{#each credentials as cred (cred.id)}
					<option value={String(cred.id)}
						>{cred.name} — {CREDENTIAL_TYPE_LABELS[cred.credential_type]}</option
					>
				{/each}
			</Select>
			<Input label="Tags for all of them" bind:value={importTags} placeholder="homelab" />
		</form>
	{/if}
	{#snippet footer()}
		<Button variant="secondary" onclick={() => (showImport = false)}
			>{importResult ? 'Close' : 'Cancel'}</Button
		>
		{#if !importResult}
			<Button type="submit" form="import-form" disabled={importing || !importText.trim()}>
				{importing ? 'Importing…' : 'Import'}
			</Button>
		{/if}
	{/snippet}
</Modal>
