<script lang="ts">
	import Modal from '#lib/components/ui/Modal.svelte';
	import Input from '#lib/components/ui/Input.svelte';
	import { fillSnippet, type SnippetRequest } from '#lib/utils/snippet.js';

	interface Props {
		request: SnippetRequest | null;
		/** The filled-in command, to be typed and run */
		onRun: (command: string) => void;
		onClose: () => void;
	}

	let { request, onRun, onClose }: Props = $props();

	/** What the user typed; a variable not touched yet shows the snippet's default */
	let answers = $state<Record<string, string>>({});

	function answer(name: string, fallback: string): string {
		return answers[name] ?? fallback;
	}

	let preview = $derived.by(() => {
		if (!request) return '';
		const values: Partial<Record<string, string>> = { ...request.known };
		for (const v of request.variables) values[v.name] = answer(v.name, v.default);
		return fillSnippet(request.command, values);
	});

	function close() {
		answers = {};
		onClose();
	}

	function submit(e: SubmitEvent) {
		e.preventDefault();
		if (!request) return;
		const command = preview;
		answers = {};
		onRun(command);
	}
</script>

<Modal open={request !== null} title="Fill in the command" onClose={close}>
	{#if request}
		<form id="snippet-vars" class="flex flex-col gap-3" onsubmit={submit}>
			{#each request.variables as v, i (v.name)}
				<Input
					label={v.name}
					value={answer(v.name, v.default)}
					oninput={(e) => (answers[v.name] = e.currentTarget.value)}
					autofocus={i === 0}
					autocomplete="off"
					autocapitalize="off"
					spellcheck={false}
					class="font-mono"
				/>
			{/each}
			<div>
				<div class="text-xs text-text-muted mb-1">Runs</div>
				<pre
					class="font-mono text-xs text-text-primary bg-background border border-border/60 rounded-sm px-3 py-2 whitespace-pre-wrap break-all"
					data-testid="snippet-preview">{preview}</pre>
			</div>
		</form>
	{/if}
	{#snippet footer()}
		<button
			type="button"
			class="px-3 py-1.5 min-h-11 md:min-h-0 text-sm font-medium bg-secondary text-secondary-foreground border border-border rounded-md hover:bg-secondary-hover"
			onclick={close}
		>
			Cancel
		</button>
		<button
			type="submit"
			form="snippet-vars"
			class="px-3 py-1.5 min-h-11 md:min-h-0 text-sm font-medium bg-primary text-primary-foreground border border-transparent rounded-md hover:bg-primary/90"
		>
			Run
		</button>
	{/snippet}
</Modal>
