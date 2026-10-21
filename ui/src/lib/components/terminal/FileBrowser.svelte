<script lang="ts">
	/**
	 * Remote files of one server over SFTP: breadcrumb, listing, upload, new folder, rename,
	 * delete. A tap on a file opens it in the viewer (FileViewer); downloading is the row's
	 * own button. Shared by the desktop side panel and the phone sheet; the state module
	 * holds the directory, so switching between the two keeps the place.
	 */
	import {
		ArrowUp,
		RotateCw,
		FolderPlus,
		Upload,
		Folder,
		File as FileIcon,
		Link2,
		Download,
		Pencil,
		Trash2,
		Check,
		X
	} from '@lucide/svelte';
	import * as files from '#lib/state/files.svelte.js';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import * as toast from '#lib/state/toast.svelte.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { breadcrumbs, formatSize, joinPath, validName } from '#lib/utils/files.js';
	import { timeAgo } from '#lib/utils/timeAgo.js';
	import type { RemoteEntry } from '#lib/api/files.js';

	let listing = $derived(files.getListing());
	let loading = $derived(files.isLoading());
	let error = $derived(files.getError());
	let busy = $derived(files.getBusy());
	let serverId = $derived(files.getServerId());
	let serverName = $derived(files.getServerName());

	/** Inline editors: a new-folder row at the top, or one row being renamed */
	let newFolderOpen = $state(false);
	let newFolderName = $state('');
	let renaming = $state<string | null>(null);
	let renameValue = $state('');
	let fileInput: HTMLInputElement | undefined = $state();

	function go(path: string) {
		if (serverId === null) return;
		newFolderOpen = false;
		renaming = null;
		files.browse(serverId, serverName, path);
	}

	async function openEntry(entry: RemoteEntry) {
		if (!listing || serverId === null) return;
		if (entry.kind === 'dir' || entry.kind === 'symlink') {
			// A symlink is followed as a folder; one that leads to a file opens in the viewer
			const target = joinPath(listing.path, entry.name);
			newFolderOpen = false;
			renaming = null;
			const listed = await files.browse(serverId, serverName, target);
			if (listed || entry.kind === 'dir') return;
			files.clearError();
		}
		await run(() => files.view(entry), `Opening ${entry.name}`);
	}

	async function run(work: () => Promise<void>, what: string) {
		try {
			await work();
		} catch (e) {
			toast.error(extractErrorMessage(e, `${what} failed`));
		}
	}

	async function submitNewFolder() {
		const problem = validName(newFolderName);
		if (problem) {
			toast.warning(problem);
			return;
		}
		const name = newFolderName.trim();
		newFolderOpen = false;
		newFolderName = '';
		await run(() => files.makeDirectory(name), `Creating ${name}`);
	}

	function startRename(entry: RemoteEntry) {
		renaming = entry.name;
		renameValue = entry.name;
	}

	async function submitRename(entry: RemoteEntry) {
		const problem = validName(renameValue);
		if (problem) {
			toast.warning(problem);
			return;
		}
		const name = renameValue.trim();
		renaming = null;
		if (name === entry.name) return;
		await run(() => files.rename(entry, name), `Renaming ${entry.name}`);
	}

	async function confirmDelete(entry: RemoteEntry) {
		const isDir = entry.kind === 'dir';
		const ok = await confirmDialog({
			title: `Delete ${entry.name}?`,
			message: isDir
				? 'Only an empty folder can be deleted here. This cannot be undone.'
				: 'The file is removed from the server. This cannot be undone.',
			confirmLabel: 'Delete',
			destructive: true
		});
		if (!ok) return;
		await run(() => files.remove(entry), `Deleting ${entry.name}`);
	}

	async function onFilesPicked(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const picked = Array.from(input.files ?? []);
		input.value = '';
		await uploadAll(picked);
	}

	/** Files dragged in from the desktop upload into the folder shown */
	let dropping = $state(false);
	const carriesFiles = (e: DragEvent) => e.dataTransfer?.types.includes('Files') ?? false;

	function onDragOver(e: DragEvent) {
		if (!listing || busy || !carriesFiles(e)) return;
		e.preventDefault();
		if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
		dropping = true;
	}

	function onDragLeave(e: DragEvent) {
		// Leaving for a child is not leaving
		const to = e.relatedTarget as Node | null;
		if (!to || !(e.currentTarget as HTMLElement).contains(to)) dropping = false;
	}

	async function onDrop(e: DragEvent) {
		if (!carriesFiles(e)) return;
		e.preventDefault();
		dropping = false;
		if (!listing || busy) return;
		// Folders arrive too and cannot be read as files; skip them
		const dropped: File[] = [];
		for (const item of Array.from(e.dataTransfer?.items ?? [])) {
			if (item.kind !== 'file' || item.webkitGetAsEntry?.()?.isDirectory) continue;
			const f = item.getAsFile();
			if (f) dropped.push(f);
		}
		if (dropped.length === 0) {
			toast.warning('Only files can be dropped here; upload a folder from the terminal');
			return;
		}
		await uploadAll(dropped);
	}

	async function uploadAll(picked: File[]) {
		for (const f of picked) {
			await run(() => files.upload(f), `Uploading ${f.name}`);
		}
		if (picked.length)
			toast.success(
				picked.length === 1 ? `Uploaded ${picked[0].name}` : `Uploaded ${picked.length} files`
			);
	}

	function onEditorKey(e: KeyboardEvent, submit: () => void, cancel: () => void) {
		if (e.key === 'Enter') {
			e.preventDefault();
			submit();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			cancel();
		}
	}

	const iconBtn =
		'w-11 h-11 md:w-7 md:h-7 flex items-center justify-center rounded-md text-text-muted hover:text-text-primary hover:bg-surface-raised active:bg-surface-raised disabled:opacity-40';
</script>

{#snippet kindIcon(entry: RemoteEntry)}
	{#if entry.kind === 'dir'}
		<Folder class="w-4 h-4 text-accent flex-shrink-0" />
	{:else if entry.kind === 'symlink'}
		<Link2 class="w-4 h-4 text-text-muted flex-shrink-0" />
	{:else}
		<FileIcon class="w-4 h-4 text-text-muted flex-shrink-0" />
	{/if}
{/snippet}

<div
	class="relative flex flex-col h-full min-h-0 text-sm"
	role="region"
	aria-label="Files"
	ondragover={onDragOver}
	ondragleave={onDragLeave}
	ondrop={onDrop}
>
	{#if dropping && listing}
		<div
			class="absolute inset-1 z-10 flex items-center justify-center rounded-md border-2 border-dashed border-accent bg-surface/85 text-sm text-text-primary pointer-events-none"
		>
			Drop to upload to <span class="ml-1 font-mono">{listing.path}</span>
		</div>
	{/if}
	<!-- Toolbar -->
	<div class="flex items-center gap-1 px-2 py-1.5 border-b border-border">
		<button
			type="button"
			class={iconBtn}
			disabled={!listing?.parent || loading}
			onclick={() => listing?.parent && go(listing.parent)}
			title="Up one level"
			aria-label="Up one level"
		>
			<ArrowUp class="w-4 h-4" />
		</button>
		<button
			type="button"
			class={iconBtn}
			disabled={loading || serverId === null}
			onclick={() => files.refresh()}
			title="Refresh"
			aria-label="Refresh"
		>
			<RotateCw class="w-4 h-4 {loading ? 'animate-spin' : ''}" />
		</button>
		<span class="flex-1"></span>
		<button
			type="button"
			class={iconBtn}
			disabled={!listing || !!busy}
			onclick={() => {
				newFolderOpen = !newFolderOpen;
				newFolderName = '';
			}}
			title="New folder"
			aria-label="New folder"
		>
			<FolderPlus class="w-4 h-4" />
		</button>
		<button
			type="button"
			class={iconBtn}
			disabled={!listing || !!busy}
			onclick={() => fileInput?.click()}
			title="Upload files here"
			aria-label="Upload files here"
		>
			<Upload class="w-4 h-4" />
		</button>
		<input type="file" multiple class="hidden" bind:this={fileInput} onchange={onFilesPicked} />
	</div>

	<!-- Breadcrumb -->
	{#if listing}
		<nav
			class="flex flex-wrap items-center gap-x-0.5 px-2 py-1 text-xs border-b border-border"
			aria-label="Path"
		>
			{#each breadcrumbs(listing.path) as crumb, i (crumb.path)}
				{#if i > 1}<span class="text-text-muted">/</span>{/if}
				<button
					type="button"
					class="px-2 md:px-1 min-h-11 md:min-h-0 py-0.5 rounded-sm font-mono {crumb.path ===
					listing.path
						? 'text-text-primary'
						: 'text-text-secondary hover:text-text-primary hover:bg-surface-raised'}"
					onclick={() => go(crumb.path)}
					aria-current={crumb.path === listing.path ? 'location' : undefined}
				>
					{crumb.label}
				</button>
			{/each}
		</nav>
	{/if}

	{#if busy}
		<div class="px-3 py-1 text-[11px] text-accent" role="status" aria-live="polite">{busy}…</div>
	{/if}
	{#if error}
		<div
			class="m-2 rounded-md border border-error/40 bg-error/10 px-3 py-2 text-xs text-error"
			role="alert"
		>
			{error}
		</div>
	{/if}

	<!-- Listing -->
	<div class="flex-1 min-h-0 overflow-y-auto">
		{#if newFolderOpen}
			<div class="flex items-center gap-2 px-2 py-1.5 border-b border-border bg-surface-raised/40">
				<Folder class="w-4 h-4 text-accent flex-shrink-0" />
				<!-- svelte-ignore a11y_autofocus -->
				<input
					type="text"
					class="flex-1 min-w-0 px-2 py-1 text-sm bg-input border border-border rounded-sm text-text-primary"
					placeholder="Folder name"
					bind:value={newFolderName}
					autofocus
					onkeydown={(e) => onEditorKey(e, submitNewFolder, () => (newFolderOpen = false))}
				/>
				<button type="button" class={iconBtn} onclick={submitNewFolder} aria-label="Create folder"
					><Check class="w-4 h-4" /></button
				>
				<button
					type="button"
					class={iconBtn}
					onclick={() => (newFolderOpen = false)}
					aria-label="Cancel"><X class="w-4 h-4" /></button
				>
			</div>
		{/if}

		{#if listing && listing.entries.length === 0 && !loading}
			<StateBlock kind="empty" title="This folder is empty" compact />
		{:else if !listing && loading}
			<p class="px-3 py-6 text-center text-text-muted">Connecting…</p>
		{:else if listing}
			<ul class="flex flex-col">
				{#each listing.entries as entry (entry.name)}
					<li
						class="group flex items-center gap-2 px-2 h-11 md:h-9 border-b border-border/50 hover:bg-surface-raised/50"
					>
						{#if renaming === entry.name}
							{@render kindIcon(entry)}
							<!-- svelte-ignore a11y_autofocus -->
							<input
								type="text"
								class="flex-1 min-w-0 px-2 py-1 text-sm bg-input border border-border rounded-sm text-text-primary"
								bind:value={renameValue}
								autofocus
								onkeydown={(e) =>
									onEditorKey(
										e,
										() => submitRename(entry),
										() => (renaming = null)
									)}
							/>
							<button
								type="button"
								class={iconBtn}
								onclick={() => submitRename(entry)}
								aria-label="Save name"><Check class="w-4 h-4" /></button
							>
							<button
								type="button"
								class={iconBtn}
								onclick={() => (renaming = null)}
								aria-label="Cancel"><X class="w-4 h-4" /></button
							>
						{:else}
							<!-- The icon is part of the target: the whole row up to its actions opens it -->
							<button
								type="button"
								class="flex-1 min-w-0 flex items-center gap-3 text-left h-full -ml-2 pl-2"
								onclick={() => openEntry(entry)}
								title="Open {entry.name}"
							>
								{@render kindIcon(entry)}
								<span
									class="flex-1 min-w-0 -ml-1 truncate {entry.kind === 'dir'
										? 'text-text-primary'
										: 'text-text-secondary'}"
								>
									{entry.name}
								</span>
								<span
									class="hidden md:inline w-16 text-right text-[11px] text-text-muted font-mono tabular-nums"
								>
									{entry.kind === 'file' ? formatSize(entry.size) : ''}
								</span>
								<span
									class="hidden md:inline w-20 text-right text-[11px] text-text-muted truncate"
									title={entry.modified ?? ''}
								>
									{entry.modified ? timeAgo(entry.modified) : ''}
								</span>
							</button>
							<span
								class="flex items-center opacity-70 md:opacity-0 md:group-hover:opacity-100 md:group-focus-within:opacity-100"
							>
								{#if entry.kind === 'file'}
									<button
										type="button"
										class={iconBtn}
										onclick={() => run(() => files.download(entry), `Downloading ${entry.name}`)}
										title="Download"
										aria-label="Download {entry.name}"
									>
										<Download class="w-3.5 h-3.5" />
									</button>
								{/if}
								<button
									type="button"
									class={iconBtn}
									onclick={() => startRename(entry)}
									title="Rename"
									aria-label="Rename {entry.name}"
								>
									<Pencil class="w-3.5 h-3.5" />
								</button>
								<button
									type="button"
									class="{iconBtn} hover:text-error"
									onclick={() => confirmDelete(entry)}
									title="Delete"
									aria-label="Delete {entry.name}"
								>
									<Trash2 class="w-3.5 h-3.5" />
								</button>
							</span>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</div>
