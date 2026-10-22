<script lang="ts">
	/** About → Backups: the last copy, the last error, the next run, and "Back up now". */
	import { onMount } from 'svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import { backUpNow, getBackupStatus, utcHourLabel, type BackupStatus } from '#lib/api/backups.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { formatSize } from '#lib/utils/files.js';
	import { timeAgo } from '#lib/utils/timeAgo.js';

	let status = $state<BackupStatus | null>(null);
	let loadError = $state<string | null>(null);
	let busy = $state(false);

	async function load() {
		loadError = null;
		try {
			status = await getBackupStatus();
		} catch (e) {
			loadError = extractErrorMessage(e, 'Could not load the backup status');
		}
	}

	onMount(load);

	async function runNow() {
		busy = true;
		try {
			const made = await backUpNow();
			toast.success(
				`Backup written (${formatSize(made.file.size_bytes)})` +
					(made.removed ? `; ${made.removed} older removed` : '')
			);
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Backup failed'));
		} finally {
			busy = false;
			await load();
		}
	}

	const failed = $derived(status?.last_run?.error ? status.last_run : null);
	const good = $derived(status?.last_success ?? null);
	const when = (iso: string) => new Date(iso).toLocaleString();
</script>

<div>
	<div class="flex items-center justify-between gap-3 mb-3">
		<h3 class="text-sm font-semibold text-text-primary">Backups</h3>
		{#if status}
			<Button size="sm" variant="secondary" onclick={runNow} disabled={busy || status.running}>
				{busy || status.running ? 'Backing up…' : 'Back up now'}
			</Button>
		{/if}
	</div>
	{#if loadError}
		<StateBlock kind="error" title={loadError} onRetry={load} compact />
	{:else if !status}
		<StateBlock kind="loading" title="Loading backups…" compact />
	{:else}
		<div class="flex flex-col gap-2">
			<div class="flex items-center justify-between gap-3 py-1">
				<span class="text-sm text-text-secondary">Last backup</span>
				{#if good?.finished_at}
					<span class="text-sm text-text-primary text-right" title={when(good.finished_at)}
						>{timeAgo(good.finished_at)} · {formatSize(good.size_bytes ?? 0)}</span
					>
				{:else}
					<span class="text-sm text-text-muted">None yet</span>
				{/if}
			</div>
			{#if failed}
				<div class="flex flex-col gap-1 py-1" role="alert">
					<div class="flex items-center justify-between gap-3">
						<span class="text-sm text-text-secondary">Last attempt</span>
						<span class="flex items-center gap-2" title={when(failed.started_at)}>
							<Badge variant="error">failed</Badge>
							<span class="text-sm text-text-primary">{timeAgo(failed.started_at)}</span>
						</span>
					</div>
					<p class="text-xs text-error break-words">{failed.error}</p>
				</div>
			{/if}
			<div class="flex items-center justify-between gap-3 py-1">
				<span class="text-sm text-text-secondary">Nightly</span>
				{#if status.next_run}
					<span class="text-sm text-text-primary text-right"
						>{utcHourLabel(status.hour)}, keeps {status.keep}</span
					>
				{:else}
					<span class="text-sm text-text-muted text-right">Off (Settings → General)</span>
				{/if}
			</div>
			<div class="flex items-center justify-between gap-3 py-1">
				<span class="text-sm text-text-secondary">Copies</span>
				<span class="text-sm text-text-primary text-right"
					>{status.files.length}{#if status.files.length}&nbsp;· {formatSize(
							status.files.reduce((n, f) => n + f.size_bytes, 0)
						)}{/if}</span
				>
			</div>
			<div class="flex items-center justify-between gap-3 py-1 min-w-0">
				<span class="text-sm text-text-secondary flex-shrink-0">Folder</span>
				<span class="text-sm text-text-primary font-mono truncate select-all" title={status.dir}
					>{status.dir}</span
				>
			</div>
			<p class="text-xs text-text-muted pt-1">
				Backups never hold the encryption key, without which their credentials cannot be read.
				{#if status.key_source === 'environment'}
					It is <span class="font-mono">ENCRYPTION_KEY</span>: keep a copy of the server's
					<span class="font-mono">.env</span>.
				{:else}
					It is <span class="font-mono">data/encryption.key</span> beside the database: keep a copy of
					it.
				{/if}
				Sign-ins are not in a backup; restoring one (<span class="font-mono">svrctl restore</span>)
				asks everyone to sign in again.
			</p>
		</div>
	{/if}
</div>
