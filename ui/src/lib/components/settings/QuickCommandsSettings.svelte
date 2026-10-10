<script lang="ts">
	import { onMount } from 'svelte';
	import { Plus, Pencil, Trash2 } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Input from '#lib/components/ui/Input.svelte';
	import Select from '#lib/components/ui/Select.svelte';
	import Textarea from '#lib/components/ui/Textarea.svelte';
	import Modal from '#lib/components/ui/Modal.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import SettingsSection from '#lib/components/ui/SettingsSection.svelte';
	import * as quickCommandsState from '#lib/state/quickCommands.svelte.js';
	import * as serversState from '#lib/state/servers.svelte.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import type { QuickCommand } from '#lib/types/index.js';

	let commands = $derived(quickCommandsState.getCommands());
	let servers = $derived(serversState.getServers());
	let categories = $derived([...new Set(commands.map((c) => c.category))].sort());

	let showModal = $state(false);
	let editingId = $state<number | null>(null);
	let saving = $state(false);

	// Form fields
	let name = $state('');
	let command = $state('');
	let category = $state('general');
	let serverId = $state<string>('');

	onMount(() => {
		quickCommandsState.loadCommands();
		if (servers.length === 0) serversState.loadServers();
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

	function openEdit(cmd: QuickCommand) {
		editingId = cmd.id;
		name = cmd.name;
		command = cmd.command;
		category = cmd.category;
		serverId = cmd.server_id ? String(cmd.server_id) : '';
		showModal = true;
	}

	function close() {
		showModal = false;
		resetForm();
	}

	async function handleSubmit() {
		if (saving || !name.trim() || !command.trim()) return;
		saving = true;
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
			close();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Operation failed'));
		} finally {
			saving = false;
		}
	}

	async function handleDelete(cmd: QuickCommand) {
		const ok = await confirmDialog({
			title: `Delete “${cmd.name}”?`,
			message: `The saved command is removed from the palette.\n${cmd.command}`,
			confirmLabel: 'Delete',
			destructive: true
		});
		if (!ok) return;
		try {
			await quickCommandsState.deleteCommand(cmd.id);
			toast.success('Command deleted');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Delete failed'));
		}
	}

	function getServerName(id: number | null): string {
		if (!id) return 'all servers';
		return servers.find((s) => s.id === id)?.name ?? `Server ${id}`;
	}
</script>

<SettingsSection
	title="Quick commands"
	description="Run from the command palette (Ctrl+K, Leader then :) or by double-tapping on a phone."
>
	{#snippet actions()}
		<Button size="sm" onclick={openCreate}><Plus class="w-3.5 h-3.5" /> Add command</Button>
	{/snippet}

	{#if commands.length === 0}
		<div class="text-center py-8">
			<p class="text-text-muted text-sm">No quick commands yet.</p>
			<Button variant="secondary" class="mt-3" onclick={openCreate}>Add your first command</Button>
		</div>
	{:else}
		<ul class="flex flex-col gap-2">
			{#each commands as cmd (cmd.id)}
				<li
					class="flex flex-col md:flex-row md:items-center gap-2 md:gap-3 p-3 border border-border/50 rounded-lg hover:bg-surface-raised/30"
				>
					<div class="flex-1 min-w-0">
						<p class="text-sm font-medium text-text-primary">{cmd.name}</p>
						<p class="text-xs font-mono text-accent truncate">{cmd.command}</p>
					</div>
					<div class="flex items-center gap-1.5 flex-shrink-0">
						<Badge>{cmd.category}</Badge>
						<Badge variant={cmd.server_id ? 'info' : 'default'}
							>{getServerName(cmd.server_id)}</Badge
						>
						<Button
							variant="ghost"
							size="sm"
							class="min-h-9 min-w-9 md:min-h-0 md:min-w-0"
							onclick={() => openEdit(cmd)}
							aria-label="Edit command"
							title="Edit"
						>
							<Pencil class="w-3.5 h-3.5" />
						</Button>
						<Button
							variant="ghost"
							size="sm"
							class="min-h-9 min-w-9 md:min-h-0 md:min-w-0"
							onclick={() => handleDelete(cmd)}
							aria-label="Delete command"
							title="Delete"
						>
							<Trash2 class="w-3.5 h-3.5 text-error" />
						</Button>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</SettingsSection>

<Modal open={showModal} title={editingId ? 'Edit command' : 'Add command'} onClose={close}>
	<form
		id="quick-command-form"
		class="flex flex-col gap-3"
		onsubmit={(e) => {
			e.preventDefault();
			handleSubmit();
		}}
	>
		<Input label="Name" bind:value={name} placeholder="Check Docker" required />
		<Textarea
			label="Command"
			bind:value={command}
			placeholder="docker ps --format table"
			required
			rows={3}
			mono
			hint="Sent to the shell followed by Enter"
		/>
		<Input label="Category" bind:value={category} placeholder="general" list="qc-categories" />
		<datalist id="qc-categories">
			{#each categories as c (c)}
				<option value={c}></option>
			{/each}
		</datalist>
		<Select label="Available on" bind:value={serverId}>
			<option value="">All servers</option>
			{#each servers as server (server.id)}
				<option value={String(server.id)}>{server.name}</option>
			{/each}
		</Select>
	</form>

	{#snippet footer()}
		<Button variant="secondary" onclick={close}>Cancel</Button>
		<Button
			type="submit"
			form="quick-command-form"
			disabled={saving || !name.trim() || !command.trim()}
		>
			{saving ? 'Saving…' : editingId ? 'Save' : 'Create'}
		</Button>
	{/snippet}
</Modal>
