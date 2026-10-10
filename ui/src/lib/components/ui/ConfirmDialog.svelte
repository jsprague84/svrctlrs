<script lang="ts">
	import Modal from './Modal.svelte';
	import Button from './Button.svelte';
	import * as confirmState from '#lib/state/confirm.svelte.js';

	let pending = $derived(confirmState.getPending());
</script>

{#if pending}
	<Modal open={true} title={pending.title} onClose={() => confirmState.answer(false)}>
		<p class="text-sm text-text-secondary whitespace-pre-line">{pending.message}</p>
		{#snippet footer()}
			<Button variant="secondary" onclick={() => confirmState.answer(false)}>
				{pending?.cancelLabel ?? 'Cancel'}
			</Button>
			<Button
				variant={pending?.destructive ? 'destructive' : 'primary'}
				onclick={() => confirmState.answer(true)}
			>
				{pending?.confirmLabel ?? 'Confirm'}
			</Button>
		{/snippet}
	</Modal>
{/if}
