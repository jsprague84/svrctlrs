<script lang="ts">
	import Modal from '$lib/components/ui/Modal.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Input from '$lib/components/ui/Input.svelte';
	import * as profilesState from '$lib/state/profiles.svelte.js';
	import * as terminalState from '$lib/state/terminal.svelte.js';
	import * as toast from '$lib/state/toast.svelte.js';
	import { extractErrorMessage } from '$lib/utils/error.js';

	interface Props {
		open: boolean;
		onClose: () => void;
		serverNames: Record<number, string>;
	}

	let { open, onClose, serverNames }: Props = $props();

	let name = $state('');
	let description = $state('');
	let isDefault = $state(false);

	function resetForm() {
		name = '';
		description = '';
		isDefault = false;
	}

	async function handleSave() {
		if (!name.trim()) return;

		try {
			const state = terminalState.captureProfileState();
			await profilesState.createProfile({
				name: name.trim(),
				description: description.trim() || undefined,
				layout: state.layout,
				pane_configs: state.panes.map((p) => ({
					server_id: p.server_id,
					mode: p.mode as 'pty' | 'cmd' | null
				})),
				is_default: isDefault
			});
			toast.success('Profile saved');
			resetForm();
			onClose();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to save profile'));
		}
	}
</script>

<Modal {open} title="Save Layout as Profile" onClose={onClose}>
	<div class="flex flex-col gap-3">
		<Input label="Name" bind:value={name} placeholder="e.g., Monitoring Setup" required />
		<Input label="Description" bind:value={description} placeholder="Optional description" />
		<label class="flex items-center gap-2 text-sm text-text-secondary">
			<input type="checkbox" bind:checked={isDefault} class="rounded" />
			Set as default (auto-loads on app start)
		</label>
		<p class="text-xs text-text-muted">
			Saves your current tab layout, server assignments, and terminal modes.
		</p>
	</div>

	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>Cancel</Button>
		<Button onclick={handleSave} disabled={!name.trim()}>Save Profile</Button>
	{/snippet}
</Modal>
