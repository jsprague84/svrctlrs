<script lang="ts">
	/**
	 * A remote file, opened from the file browser: images and text shown, small text files
	 * edited in a plain textarea, anything else offered for download or for the server's own
	 * editor in a terminal tab. Full screen on the phone, a large dialog on the desktop.
	 */
	import {
		X,
		Download,
		Pencil,
		SquareTerminal,
		File as FileIcon,
		RotateCw,
		TriangleAlert
	} from '@lucide/svelte';
	import * as files from '#lib/state/files.svelte.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { ApiError } from '#lib/api/client.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { formatSize } from '#lib/utils/files.js';
	import { whyNotEditable } from '#lib/utils/fileView.js';
	import { overlay } from '#lib/utils/overlay.js';
	import { watchClose } from '#lib/utils/closeWatcher.js';
	import { timeAgo } from '#lib/utils/timeAgo.js';

	interface Props {
		/** Open a terminal tab on that server with the file in its own editor */
		onEditInTerminal: (serverId: number, serverName: string | null, path: string) => void;
	}

	let { onEditInTerminal }: Props = $props();

	let v = $derived(files.getViewing());
	let decoded = $derived(v?.decoded ?? null);
	let notEditable = $derived(decoded ? whyNotEditable(decoded, !!v?.etag) : null);

	/** The editor: its text, and why the last save was refused */
	let editing = $state(false);
	let draft = $state('');
	let saving = $state(false);
	let refusal = $state<{ kind: 'changed' | 'denied' | 'other'; message: string } | null>(null);
	let textarea: HTMLTextAreaElement | undefined = $state();
	let dirty = $derived(editing && decoded !== null && draft !== decoded.text);

	// A different file (or none): leave the editor
	let shownPath = $state<string | null>(null);
	$effect(() => {
		const path = v?.path ?? null;
		if (path !== shownPath) {
			shownPath = path;
			editing = false;
			refusal = null;
		}
	});

	// The phone's back gesture closes the viewer; a watcher fires once, so one that was
	// answered with "keep editing" is set up again
	let watchGeneration = $state(0);
	$effect(() => {
		void watchGeneration;
		if (v) return watchClose(() => void close());
	});

	async function close() {
		if (dirty) {
			const ok = await confirmDialog({
				title: 'Discard your changes?',
				message: `Your edits to ${v?.name} have not been saved.`,
				confirmLabel: 'Discard',
				destructive: true
			});
			if (!ok) {
				watchGeneration++;
				return;
			}
		}
		editing = false;
		files.closeViewer();
	}

	function startEditing() {
		if (!decoded || notEditable) return;
		draft = decoded.text;
		refusal = null;
		editing = true;
		requestAnimationFrame(() => textarea?.focus());
	}

	async function cancelEditing() {
		if (dirty) {
			const ok = await confirmDialog({
				title: 'Discard your changes?',
				message: 'The file stays as it was.',
				confirmLabel: 'Discard',
				destructive: true
			});
			if (!ok) return;
		}
		editing = false;
		refusal = null;
	}

	async function save() {
		if (!editing || saving) return;
		saving = true;
		refusal = null;
		try {
			await files.saveText(draft);
			editing = false;
			toast.success(`Saved ${v?.name}`);
		} catch (e) {
			const message = extractErrorMessage(e, 'Saving failed');
			if (e instanceof ApiError && e.status === 412) {
				refusal = {
					kind: 'changed',
					message:
						'The file changed on the server since you opened it, so nothing was saved. Copy your text if you need it, then reload.'
				};
			} else if (e instanceof ApiError && e.status === 403) {
				refusal = {
					kind: 'denied',
					message: 'You may not write this file. Edit it in the terminal (with sudoedit, say).'
				};
			} else {
				refusal = { kind: 'other', message };
			}
		} finally {
			saving = false;
		}
	}

	async function reload() {
		if (dirty) {
			const ok = await confirmDialog({
				title: 'Reload and lose your edits?',
				message: 'The editor shows the file as it is on the server now.',
				confirmLabel: 'Reload',
				destructive: true
			});
			if (!ok) return;
		}
		editing = false;
		refusal = null;
		await files.load();
	}

	async function download() {
		try {
			await files.downloadViewed();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Download failed'));
		}
	}

	function editInTerminal() {
		if (!v) return;
		const { serverId, serverName, path } = v;
		editing = false;
		files.closeViewer();
		onEditInTerminal(serverId, serverName, path);
	}

	/** After a refused save: the terminal opens the file as it is on the server */
	async function leaveForTerminal() {
		const ok = await confirmDialog({
			title: 'Leave your edits?',
			message: 'The terminal opens the file as it is on the server; your edits here are discarded.',
			confirmLabel: 'Open in terminal'
		});
		if (!ok) return;
		editing = false;
		editInTerminal();
	}

	function onEditorKey(e: KeyboardEvent) {
		if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
			e.preventDefault();
			void save();
		} else if (e.key === 'Tab' && !e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
			// A tab character, as any editor; Escape then Tab leaves the textarea
			e.preventDefault();
			const ta = e.currentTarget as HTMLTextAreaElement;
			ta.setRangeText('\t', ta.selectionStart, ta.selectionEnd, 'end');
			draft = ta.value;
		}
	}

	const lineEndingLabel = {
		lf: 'LF',
		crlf: 'CRLF',
		none: 'no line breaks',
		mixed: 'mixed line endings'
	};

	const headerBtn =
		'w-11 h-11 md:w-auto md:h-8 md:px-2.5 flex items-center justify-center gap-1.5 rounded-md text-xs text-text-secondary hover:text-text-primary hover:bg-surface-raised active:bg-surface-raised disabled:opacity-40 disabled:hover:bg-transparent';
</script>

{#if v}
	<div
		class="fixed inset-0 z-50 flex md:items-center md:justify-center md:p-6"
		role="dialog"
		aria-modal="true"
		aria-label={v.name}
		tabindex="-1"
		{@attach overlay({ onEscape: () => void close() })}
	>
		<button
			class="absolute inset-0 bg-black/60 hidden md:block"
			onclick={() => void close()}
			tabindex="-1"
			aria-label="Close"
		></button>

		<div
			class="relative flex flex-col w-full h-full md:max-w-5xl md:h-[min(88dvh,60rem)] bg-surface md:border md:border-border md:rounded-lg md:shadow-lg overflow-hidden pt-[env(safe-area-inset-top,0px)]"
		>
			<!-- Header: the file, and what can be done with it -->
			<div class="flex items-center gap-1 pl-3 pr-1 md:pr-2 py-1 border-b border-border">
				<FileIcon class="w-4 h-4 text-text-muted flex-shrink-0" />
				<div class="flex-1 min-w-0 px-1">
					<div class="text-sm font-semibold text-text-primary truncate">{v.name}</div>
					<div
						class="text-[11px] text-text-muted truncate font-mono"
						title={v.serverName ? `${v.serverName}:${v.path}` : v.path}
					>
						<span class="hidden md:inline">{v.serverName ? `${v.serverName}:` : ''}</span>{v.path}
					</div>
				</div>
				{#if !editing}
					<button
						type="button"
						class={headerBtn}
						disabled={v.loading || !decoded || !!notEditable}
						onclick={startEditing}
						title={notEditable ?? 'Edit here'}
						aria-label="Edit here"
					>
						<Pencil class="w-4 h-4" /><span class="hidden md:inline">Edit</span>
					</button>
				{/if}
				<button
					type="button"
					class={headerBtn}
					disabled={dirty}
					onclick={editInTerminal}
					title={dirty
						? 'Save or discard your edits first'
						: 'Open in the server’s editor ($VISUAL, $EDITOR, vi)'}
					aria-label="Edit in terminal"
				>
					<SquareTerminal class="w-4 h-4" /><span class="hidden md:inline">Edit in terminal</span>
				</button>
				<button
					type="button"
					class={headerBtn}
					onclick={download}
					title="Download"
					aria-label="Download {v.name}"
				>
					<Download class="w-4 h-4" /><span class="hidden md:inline">Download</span>
				</button>
				<button
					type="button"
					class={headerBtn}
					onclick={() => void close()}
					title="Close"
					aria-label="Close {v.name}"
				>
					<X class="w-4 h-4" />
				</button>
			</div>

			{#if refusal}
				<div
					class="flex flex-wrap items-center gap-2 px-3 py-2 text-xs border-b border-warning/40 bg-warning/10 text-text-primary"
					role="alert"
				>
					<TriangleAlert class="w-4 h-4 text-warning flex-shrink-0" />
					<span class="flex-1 min-w-[12rem]">{refusal.message}</span>
					{#if refusal.kind === 'changed'}
						<button
							type="button"
							class="{headerBtn} md:border md:border-border"
							onclick={reload}
							aria-label="Reload the file"
						>
							<RotateCw class="w-4 h-4" /><span class="hidden md:inline">Reload</span>
						</button>
					{:else if refusal.kind === 'denied'}
						<button
							type="button"
							class="{headerBtn} md:border md:border-border"
							onclick={leaveForTerminal}
							aria-label="Open in terminal instead"
						>
							<SquareTerminal class="w-4 h-4" /><span class="hidden md:inline"
								>Edit in terminal</span
							>
						</button>
					{/if}
				</div>
			{/if}

			<!-- Body -->
			<div class="flex-1 min-h-0 flex flex-col">
				{#if v.loading}
					<p class="m-auto text-sm text-text-muted" role="status">Reading {v.name}…</p>
				{:else if v.error}
					<div class="m-auto flex flex-col items-center gap-3 px-6 text-center">
						<p class="text-sm text-error">{v.error}</p>
						<button
							type="button"
							class="{headerBtn} md:border md:border-border"
							onclick={() => files.load()}
						>
							<RotateCw class="w-4 h-4" /> <span>Try again</span>
						</button>
					</div>
				{:else if v.kind === 'special'}
					<p class="m-auto px-6 text-center text-sm text-text-muted">
						Not a regular file (a device, a pipe or a socket). Open it in the terminal.
					</p>
				{:else if v.kind === 'too-large'}
					<p class="m-auto px-6 text-center text-sm text-text-muted">
						Too large to show here ({formatSize(v.entry.size)}). Download it, or open it in the
						terminal.
					</p>
				{:else if v.imageUrl}
					<div class="flex-1 min-h-0 flex items-center justify-center p-3 bg-surface-raised/40">
						<img src={v.imageUrl} alt={v.name} class="max-w-full max-h-full object-contain" />
					</div>
				{:else if decoded?.binary}
					<p class="m-auto px-6 text-center text-sm text-text-muted">
						A binary file ({formatSize(v.entry.size)}). Download it, or open it in the terminal.
					</p>
				{:else if decoded && editing}
					<textarea
						bind:this={textarea}
						bind:value={draft}
						onkeydown={onEditorKey}
						class="flex-1 min-h-0 w-full resize-none p-3 bg-input text-text-primary font-mono text-base md:text-[13px] leading-snug outline-none"
						wrap="off"
						spellcheck="false"
						autocapitalize="off"
						autocomplete="off"
						aria-label="Contents of {v.name}"></textarea>
				{:else if decoded}
					<!-- Focusable so the keyboard can scroll it -->
					<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
					<pre
						aria-label="Contents of {v.name}"
						class="flex-1 min-h-0 overflow-auto p-3 m-0 font-mono text-[13px] leading-snug text-text-primary whitespace-pre select-text"
						tabindex="0">{decoded.text}</pre>
				{/if}
			</div>

			<!-- Footer: what the file is; Save and Cancel while editing -->
			{#if decoded && !decoded.binary && !v.loading && !v.error}
				<div
					class="flex items-center gap-2 px-3 py-1.5 pb-[max(0.375rem,env(safe-area-inset-bottom))] border-t border-border text-[11px] text-text-muted"
				>
					<span class="flex-1 min-w-0 truncate">
						{decoded.utf8 ? 'UTF-8' : 'not UTF-8'}{decoded.bom ? ' with BOM' : ''} · {lineEndingLabel[
							decoded.lineEnding
						]} · {formatSize(v.blob?.size ?? v.entry.size)}{v.entry.modified
							? ` · changed ${timeAgo(v.entry.modified)}`
							: ''}{!editing && notEditable ? ` · read-only: ${notEditable}` : ''}
					</span>
					{#if editing}
						<button
							type="button"
							class="min-h-11 md:min-h-0 px-3 py-1.5 rounded-sm text-sm text-text-secondary hover:bg-surface-raised"
							onclick={cancelEditing}
							disabled={saving}
						>
							Cancel
						</button>
						<button
							type="button"
							class="min-h-11 md:min-h-0 px-3 py-1.5 rounded-sm text-sm font-medium bg-primary text-primary-foreground hover:opacity-90 disabled:opacity-50"
							onclick={save}
							disabled={saving || !dirty}
							title="Save (Ctrl+S)"
						>
							{saving ? 'Saving…' : 'Save'}
						</button>
					{/if}
				</div>
			{/if}
		</div>
	</div>
{/if}
