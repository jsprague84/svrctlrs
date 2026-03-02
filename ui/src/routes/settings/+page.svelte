<script lang="ts">
	import { onMount } from 'svelte';
	import { Settings, Save, Plus } from 'lucide-svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Modal from '$lib/components/ui/Modal.svelte';
	import Input from '$lib/components/ui/Input.svelte';
	import Select from '$lib/components/ui/Select.svelte';
	import * as settingsState from '$lib/state/settings.svelte.js';
	import * as toast from '$lib/state/toast.svelte.js';
	import type { Setting, SettingValueType } from '$lib/types/index.js';

	let editingKey = $state<string | null>(null);
	let editValue = $state('');

	// Create modal state
	let showCreate = $state(false);
	let newKey = $state('');
	let newValue = $state('');
	let newValueType = $state<SettingValueType>('string');
	let newDescription = $state('');

	let settings = $derived(settingsState.getSettings());
	let loading = $derived(settingsState.isLoading());

	onMount(() => {
		settingsState.loadSettings();
	});

	function startEdit(setting: Setting) {
		editingKey = setting.key;
		editValue = setting.value;
	}

	function cancelEdit() {
		editingKey = null;
		editValue = '';
	}

	async function saveEdit(key: string) {
		try {
			await settingsState.updateSetting(key, editValue);
			toast.success('Setting updated');
			editingKey = null;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'Failed to update setting');
		}
	}

	function openCreate() {
		newKey = ''; newValue = ''; newValueType = 'string'; newDescription = '';
		showCreate = true;
	}

	async function handleCreate() {
		try {
			await settingsState.createSetting({
				key: newKey,
				value: newValue,
				value_type: newValueType,
				description: newDescription || undefined
			});
			toast.success('Setting created');
			showCreate = false;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'Failed to create setting');
		}
	}
</script>

<div class="flex flex-col h-full">
	<div class="flex flex-wrap items-center justify-between gap-2 px-4 md:px-6 py-4 border-b border-border">
		<div class="flex items-center gap-3">
			<Settings class="w-5 h-5 text-accent" />
			<h1 class="text-lg font-semibold text-text-primary">Settings</h1>
		</div>
		<Button onclick={openCreate}><Plus class="w-4 h-4" /> Add Setting</Button>
	</div>

	<div class="flex-1 overflow-y-auto p-4 md:p-6">
		{#if loading}
			<div class="text-text-muted">Loading...</div>
		{:else}
			{#each Object.entries(settings) as [group, items]}
				<div class="mb-6">
					<h2 class="text-sm font-semibold text-text-primary capitalize mb-3 pb-1 border-b border-border/50">
						{group.replace(/_/g, ' ')}
					</h2>
					<div class="flex flex-col gap-2">
						{#each items as setting}
							<div class="flex flex-col md:flex-row md:items-start gap-2 md:gap-4 px-3 py-2 rounded-sm hover:bg-surface-raised/30">
								<div class="flex-1 min-w-0">
									<div class="text-sm text-text-primary font-mono break-all">{setting.key}</div>
									{#if setting.description}
										<div class="text-xs text-text-muted mt-0.5">{setting.description}</div>
									{/if}
								</div>
								<div class="flex flex-wrap items-center gap-2">
									{#if editingKey === setting.key}
										{#if setting.value_type === 'boolean'}
											<select bind:value={editValue} class="w-full md:w-auto px-2 py-1 text-sm bg-input border border-border rounded-sm text-text-primary">
												<option value="true">true</option>
												<option value="false">false</option>
											</select>
										{:else}
											<input
												type={setting.value_type === 'number' ? 'number' : 'text'}
												bind:value={editValue}
												class="w-full md:w-48 px-2 py-1 text-sm bg-input border border-border rounded-sm text-text-primary"
											/>
										{/if}
										<Button size="sm" onclick={() => saveEdit(setting.key)}>
											<Save class="w-3 h-3" /> Save
										</Button>
										<Button variant="ghost" size="sm" onclick={cancelEdit}>Cancel</Button>
									{:else}
										<span class="text-sm text-text-secondary font-mono bg-surface-raised px-2 py-0.5 rounded-sm break-all">
											{setting.value}
										</span>
										<span class="text-[10px] text-text-muted uppercase">{setting.value_type}</span>
										<Button variant="ghost" size="sm" onclick={() => startEdit(setting)}>Edit</Button>
									{/if}
								</div>
							</div>
						{/each}
					</div>
				</div>
			{/each}

			{#if Object.keys(settings).length === 0}
				<div class="text-center text-text-muted py-8">
					<p>No settings available.</p>
				</div>
			{/if}
		{/if}
	</div>
</div>

<Modal open={showCreate} title="Add Setting" onClose={() => showCreate = false}>
	<form class="flex flex-col gap-3" onsubmit={(e) => { e.preventDefault(); handleCreate(); }}>
		<Input label="Key" bind:value={newKey} required placeholder="category.setting_name" />
		<Select label="Type" bind:value={newValueType}>
			<option value="string">String</option>
			<option value="number">Number</option>
			<option value="boolean">Boolean</option>
			<option value="json">JSON</option>
		</Select>
		{#if newValueType === 'boolean'}
			<Select label="Value" bind:value={newValue}>
				<option value="true">true</option>
				<option value="false">false</option>
			</Select>
		{:else}
			<Input label="Value" bind:value={newValue} required placeholder={newValueType === 'number' ? '0' : newValueType === 'json' ? '{}' : 'value'} />
		{/if}
		<Input label="Description" bind:value={newDescription} placeholder="Optional description" />
	</form>

	{#snippet footer()}
		<Button variant="secondary" onclick={() => showCreate = false}>Cancel</Button>
		<Button onclick={handleCreate}>Create</Button>
	{/snippet}
</Modal>
