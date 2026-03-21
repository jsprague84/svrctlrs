<script lang="ts">
	import { onMount } from 'svelte';
	import { Plus, Pencil, Trash2 } from 'lucide-svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Input from '$lib/components/ui/Input.svelte';
	import Modal from '$lib/components/ui/Modal.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';
	import * as quickCommandsState from '$lib/state/quickCommands.svelte.js';
	import * as serversState from '$lib/state/servers.svelte.js';
	import * as toast from '$lib/state/toast.svelte.js';
	import { extractErrorMessage } from '$lib/utils/error.js';

	let commands = $derived(quickCommandsState.getCommands());
	let servers = $derived(serversState.getServers());

	let showModal = $state(false);
	let editingId = $state<number | null>(null);

	// Form fields
	let name = $state('');
	let command = $state('');
	let category = $state('general');
	let serverId = $state<string>('');

	onMount(() => {
		quickCommandsState.loadCommands();
	});

	function resetForm() {
		name = '';
		command = '';
		category = 'general';
		serverId = '';
		editingId = null;
	}

	function openCreate() {
		resetForm();
		showModal = true;
	}

	function openEdit(cmd: typeof commands[0]) {
		editingId = cmd.id;
		name = cmd.name;
		command = cmd.command;
		category = cmd.category;
		serverId = cmd.server_id ? String(cmd.server_id) : '';
		showModal = true;
	}

	async function handleSubmit() {
		if (!name.trim() || !command.trim()) return;
		try {
			const input = {
				name: name.trim(),
				command: command.trim(),
				category: category.trim() || 'general',
				server_id: serverId ? Number(serverId) : undefined
			};
			if (editingId) {
				await quickCommandsState.updateCommand(editingId, input);
				toast.success('Command updated');
			} else {
				await quickCommandsState.createCommand(input);
				toast.success('Command created');
			}
			showModal = false;
			resetForm();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Operation failed'));
		}
	}

	async function handleDelete(id: number) {
		if (!confirm('Delete this command?')) return;
		try {
			await quickCommandsState.deleteCommand(id);
			toast.success('Command deleted');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Delete failed'));
		}
	}

	function getServerName(id: number | null): string {
		if (!id) return 'global';
		return servers.find(s => s.id === id)?.name ?? `Server ${id}`;
	}
</script>

<div>
	<div class="flex items-center justify-between mb-4">
		<h3 class="text-[11px] uppercase tracking-wider text-text-muted font-semibold">Saved Commands</h3>
		<Button size="sm" onclick={openCreate}><Plus class="w-3.5 h-3.5" /> Add Command</Button>
	</div>

	{#if commands.length === 0}
		<div class="text-center py-8">
			<p class="text-text-muted text-sm">No quick commands yet.</p>
			<p class="text-text-muted text-xs mt-1">Add one to use it from the command palette (Ctrl+K).</p>
			<Button variant="secondary" class="mt-3" onclick={openCreate}>Add your first command</Button>
		</div>
	{:else}
		<div class="flex flex-col gap-2">
			{#each commands as cmd (cmd.id)}
				<div class="flex flex-col md:flex-row md:items-center gap-2 md:gap-3 p-3 bg-surface border border-border/50 rounded-lg hover:bg-surface-raised/30">
					<div class="flex-1 min-w-0">
						<p class="text-sm font-medium text-text-primary">{cmd.name}</p>
						<p class="text-xs font-mono text-accent truncate">{cmd.command}</p>
					</div>
					<div class="flex items-center gap-2 flex-shrink-0">
						<Badge>{cmd.category}</Badge>
						<Badge variant={cmd.server_id ? 'info' : 'default'}>{getServerName(cmd.server_id)}</Badge>
						<button class="p-1 text-text-muted hover:text-text-primary" onclick={() => openEdit(cmd)} aria-label="Edit">
							<Pencil class="w-3.5 h-3.5" />
						</button>
						<button class="p-1 text-text-muted hover:text-error" onclick={() => handleDelete(cmd.id)} aria-label="Delete">
							<Trash2 class="w-3.5 h-3.5" />
						</button>
					</div>
				</div>
			{/each}
		</div>
		<p class="text-[10px] text-text-muted mt-4">Access commands via Ctrl+K (command palette) or double-tap on mobile.</p>
	{/if}
</div>

<!-- Create/Edit Modal -->
<Modal open={showModal} title={editingId ? 'Edit Command' : 'Add Command'} onClose={() => { showModal = false; resetForm(); }}>
	<div class="flex flex-col gap-3">
		<Input label="Name" bind:value={name} placeholder="e.g., Check Docker" required />
		<div>
			<label class="block text-xs font-medium text-text-secondary mb-1">Command</label>
			<textarea
				bind:value={command}
				placeholder="e.g., docker ps --format table"
				required
				rows="3"
				class="w-full px-3 py-1.5 text-sm bg-input border border-border rounded-sm text-text-primary font-mono resize-y focus:ring-1 focus:ring-ring focus:outline-none"
			></textarea>
		</div>
		<Input label="Category" bind:value={category} placeholder="general" />
		<div>
			<label class="block text-xs font-medium text-text-secondary mb-1">Server Scope</label>
			<select bind:value={serverId} class="w-full px-3 py-1.5 text-sm bg-input border border-border rounded-sm text-text-primary">
				<option value="">All Servers (global)</option>
				{#each servers as server}
					<option value={String(server.id)}>{server.name}</option>
				{/each}
			</select>
		</div>
	</div>

	{#snippet footer()}
		<Button variant="secondary" onclick={() => { showModal = false; resetForm(); }}>Cancel</Button>
		<Button onclick={handleSubmit} disabled={!name.trim() || !command.trim()}>
			{editingId ? 'Save' : 'Create'}
		</Button>
	{/snippet}
</Modal>
