<script lang="ts">
	import { onMount } from 'svelte';
	import { Settings, Save } from 'lucide-svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import * as settingsState from '$lib/state/settings.svelte.js';
	import * as toast from '$lib/state/toast.svelte.js';
	import type { Setting } from '$lib/types/index.js';

	let editingKey = $state<string | null>(null);
	let editValue = $state('');

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
</script>

<div class="flex flex-col h-full">
	<div class="flex items-center gap-3 px-6 py-4 border-b border-border">
		<Settings class="w-5 h-5 text-accent" />
		<h1 class="text-lg font-semibold text-text-primary">Settings</h1>
	</div>

	<div class="flex-1 overflow-y-auto p-6">
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
							<div class="flex items-start gap-4 px-3 py-2 rounded-sm hover:bg-surface-raised/30">
								<div class="flex-1 min-w-0">
									<div class="text-sm text-text-primary font-mono">{setting.key}</div>
									{#if setting.description}
										<div class="text-xs text-text-muted mt-0.5">{setting.description}</div>
									{/if}
								</div>
								<div class="flex items-center gap-2">
									{#if editingKey === setting.key}
										{#if setting.value_type === 'boolean'}
											<select bind:value={editValue} class="px-2 py-1 text-sm bg-input border border-border rounded-sm text-text-primary">
												<option value="true">true</option>
												<option value="false">false</option>
											</select>
										{:else}
											<input
												type={setting.value_type === 'number' ? 'number' : 'text'}
												bind:value={editValue}
												class="px-2 py-1 text-sm bg-input border border-border rounded-sm text-text-primary w-48"
											/>
										{/if}
										<Button size="sm" onclick={() => saveEdit(setting.key)}>
											<Save class="w-3 h-3" /> Save
										</Button>
										<Button variant="ghost" size="sm" onclick={cancelEdit}>Cancel</Button>
									{:else}
										<span class="text-sm text-text-secondary font-mono bg-surface-raised px-2 py-0.5 rounded-sm">
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
