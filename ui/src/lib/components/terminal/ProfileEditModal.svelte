<script lang="ts">
	import Modal from '$lib/components/ui/Modal.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Input from '$lib/components/ui/Input.svelte';
	import * as profilesState from '$lib/state/profiles.svelte.js';
	import * as toast from '$lib/state/toast.svelte.js';
	import { extractErrorMessage } from '$lib/utils/error.js';
	import type { TerminalProfile } from '$lib/types/index.js';

	interface Props {
		open: boolean;
		onClose: () => void;
		profile: TerminalProfile | null;
	}

	let { open, onClose, profile }: Props = $props();

	let name = $state('');
	let description = $state('');
	let isDefault = $state(false);

	// Populate form when profile changes
	$effect(() => {
		if (profile) {
			name = profile.name;
			description = profile.description ?? '';
			isDefault = profile.is_default;
		}
	});

	async function handleSave() {
		if (!profile || !name.trim()) return;
		try {
			await profilesState.updateProfile(profile.id, {
				name: name.trim(),
				description: description.trim() || undefined,
				is_default: isDefault
			});
			toast.success('Profile updated');
			onClose();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to update profile'));
		}
	}
</script>

<Modal {open} title="Edit Profile" {onClose}>
	<div class="flex flex-col gap-3">
		<Input label="Name" bind:value={name} required />
		<Input label="Description" bind:value={description} placeholder="Optional description" />
		<label class="flex items-center gap-2 text-sm text-text-secondary">
			<input type="checkbox" bind:checked={isDefault} class="rounded" />
			Set as default (auto-loads on app start)
		</label>
	</div>

	{#snippet footer()}
		<Button variant="secondary" onclick={onClose}>Cancel</Button>
		<Button onclick={handleSave} disabled={!name.trim()}>Save</Button>
	{/snippet}
</Modal>
