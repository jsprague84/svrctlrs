<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { MonitorPlay, Play, Download, Trash2 } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import Select from '#lib/components/ui/Select.svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import ActionMenu, { type MenuItem } from '#lib/components/ui/ActionMenu.svelte';
	import RecordingPlayer from '#lib/components/recordings/RecordingPlayer.svelte';
	import * as recordingsApi from '#lib/api/recordings.js';
	import * as serversState from '#lib/state/servers.svelte.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { saveBlob } from '#lib/utils/download.js';
	import { timeAgo } from '#lib/utils/timeAgo.js';
	import { formatBytes, formatClock } from '#lib/recordings/cast.js';
	import type { Recording } from '#lib/types/index.js';

	const PAGE = 50;

	let items = $state<Recording[]>([]);
	let retentionDays = $state(30);
	let maxBytes = $state(0);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let hasMore = $state(false);
	let loadingMore = $state(false);
	/** '' shows every server */
	let serverFilter = $state('');
	let playing = $state<Recording | null>(null);

	let servers = $derived(serversState.getServers());
	let recordingServers = $derived(servers.filter((s) => s.record_sessions));

	async function load() {
		loading = true;
		error = null;
		try {
			const res = await recordingsApi.listRecordings({
				serverId: serverFilter ? Number(serverFilter) : undefined,
				limit: PAGE
			});
			items = res.recordings;
			retentionDays = res.retention_days;
			maxBytes = res.max_bytes;
			hasMore = res.recordings.length === PAGE;
		} catch (e) {
			error = extractErrorMessage(e, 'Could not load recordings');
		} finally {
			loading = false;
		}
	}

	async function loadMore() {
		const last = items[items.length - 1];
		if (!last) return;
		loadingMore = true;
		try {
			const res = await recordingsApi.listRecordings({
				serverId: serverFilter ? Number(serverFilter) : undefined,
				before: last.id,
				limit: PAGE
			});
			items = [...items, ...res.recordings];
			hasMore = res.recordings.length === PAGE;
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Could not load more recordings'));
		} finally {
			loadingMore = false;
		}
	}

	function changeFilter() {
		const query = serverFilter ? `?server=${serverFilter}` : '';
		goto(`${resolve('/recordings')}${query}`, { replace: true });
		load();
	}

	function fileName(r: Recording): string {
		const d = new Date(r.started_at);
		const pad = (n: number) => String(n).padStart(2, '0');
		const stamp = `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}`;
		return `${r.server_name}-${stamp}.cast`;
	}

	async function download(r: Recording) {
		try {
			saveBlob(await recordingsApi.downloadCast(r.id), fileName(r));
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Download failed'));
		}
	}

	async function remove(r: Recording) {
		const ok = await confirmDialog({
			title: 'Delete recording?',
			message: `The recording of ${r.server_name} from ${new Date(r.started_at).toLocaleString()} is deleted for good.`,
			confirmLabel: 'Delete',
			destructive: true
		});
		if (!ok) return;
		try {
			await recordingsApi.deleteRecording(r.id);
			items = items.filter((x) => x.id !== r.id);
			toast.success('Recording deleted');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Could not delete the recording'));
		}
	}

	function rowActions(r: Recording): MenuItem[] {
		return [
			{ label: 'Download', icon: Download, onSelect: () => download(r) },
			{
				label: 'Delete',
				icon: Trash2,
				onSelect: () => remove(r),
				destructive: true,
				disabled: r.status === 'recording'
			}
		];
	}

	const STATUS: Record<
		Recording['status'],
		{ label: string; variant: 'error' | 'warning' } | null
	> = {
		recording: { label: '● recording', variant: 'error' },
		complete: null,
		truncated: { label: 'stopped at size limit', variant: 'warning' },
		interrupted: { label: 'interrupted', variant: 'warning' },
		failed: { label: 'failed', variant: 'error' }
	};

	/** "1:05 · 12 KB", or for a shell still being recorded its running time so far */
	function describeLength(r: Recording): string {
		if (r.status === 'recording') {
			const secs = (Date.now() - new Date(r.started_at).getTime()) / 1000;
			const size = r.size_bytes > 0 ? ` · ${formatBytes(r.size_bytes)}` : '';
			return `${formatClock(secs)} so far${size}`;
		}
		const base = `${formatClock(r.duration_ms / 1000)} · ${formatBytes(r.size_bytes)}`;
		return r.status !== 'complete' && r.end_reason ? `${base} · ${r.end_reason}` : base;
	}

	function openPlayer(r: Recording) {
		playing = r;
	}

	function closePlayer() {
		playing = null;
		if (page.url.searchParams.has('play')) {
			const query = serverFilter ? `?server=${serverFilter}` : '';
			goto(`${resolve('/recordings')}${query}`, { replace: true });
		}
	}

	onMount(async () => {
		serverFilter = page.url.searchParams.get('server') ?? '';
		serversState.loadServers();
		await load();
		const play = Number(page.url.searchParams.get('play'));
		if (play) {
			const r = items.find((x) => x.id === play);
			if (r) playing = r;
		}
	});
</script>

<div class="flex flex-col h-full">
	<div
		class="flex flex-wrap items-center justify-between gap-2 px-4 md:px-6 py-4 border-b border-border"
	>
		<div class="flex items-center gap-3">
			<MonitorPlay class="w-5 h-5 text-accent" />
			<h1 class="text-lg font-semibold text-text-primary">Recordings</h1>
			{#if !loading}<Badge>{items.length}{hasMore ? '+' : ''}</Badge>{/if}
		</div>
		{#if servers.length > 0}
			<div class="w-48">
				<Select bind:value={serverFilter} onchange={changeFilter} aria-label="Server">
					<option value="">All servers</option>
					{#each servers as s (s.id)}
						<option value={String(s.id)}>{s.name}</option>
					{/each}
				</Select>
			</div>
		{/if}
	</div>
	<p class="px-4 md:px-6 py-2 border-b border-border/60 text-xs text-text-muted">
		What the terminal showed, never what you typed. Only you can see your recordings.
		{retentionDays > 0
			? `Kept for ${retentionDays} day${retentionDays === 1 ? '' : 's'}`
			: 'Kept until you delete them'}{maxBytes ? `, up to ${formatBytes(maxBytes)} each` : ''}.
		{#if recordingServers.length > 0}
			Recording is on for {recordingServers.map((s) => s.name).join(', ')}.
		{/if}
	</p>

	<div class="flex-1 overflow-y-auto p-4 md:p-6">
		{#if loading && items.length === 0}
			<StateBlock kind="loading" title="Loading recordings…" />
		{:else if error}
			<StateBlock kind="error" title={error} onRetry={load} />
		{:else if items.length === 0}
			<StateBlock
				kind="empty"
				icon={MonitorPlay}
				title={serverFilter ? 'No recordings of this server' : 'No recordings yet'}
				hint="Switch on “Record sessions” for a server (Servers → Edit). Shells you open there afterwards are recorded and appear here."
			>
				<Button variant="secondary" onclick={() => goto(resolve('/servers'))}>Go to Servers</Button>
			</StateBlock>
		{:else}
			<ul class="flex flex-col gap-2">
				{#each items as r (r.id)}
					{@const status = STATUS[r.status]}
					<li
						class="bg-surface border border-border rounded-lg pl-3 pr-1.5 md:px-4 py-2.5 md:py-3 hover:border-border-strong/60 flex items-center gap-2 md:gap-4"
					>
						<button
							type="button"
							class="min-w-0 flex-1 text-left"
							onclick={() => openPlayer(r)}
							aria-label="Play the recording of {r.server_name} from {new Date(
								r.started_at
							).toLocaleString()}"
						>
							<span class="flex items-center gap-2 min-w-0">
								<span class="text-text-primary font-medium truncate">{r.server_name}</span>
								{#if status}<Badge variant={status.variant}>{status.label}</Badge>{/if}
							</span>
							<span class="block mt-0.5 text-xs text-text-secondary truncate">
								<span title={new Date(r.started_at).toLocaleString()}
									>{new Date(r.started_at).toLocaleString([], {
										dateStyle: 'medium',
										timeStyle: 'short'
									})}</span
								>
								<span class="text-text-muted"> · {timeAgo(r.started_at)}</span>
							</span>
							<span class="block mt-0.5 text-xs text-text-muted truncate">
								{describeLength(r)}
							</span>
						</button>

						<div class="flex md:hidden items-center gap-0.5 flex-shrink-0">
							<Button size="sm" class="min-h-11 px-3" onclick={() => openPlayer(r)}>
								<Play class="w-4 h-4" /> Play
							</Button>
							<ActionMenu label="Actions for this recording" items={rowActions(r)} />
						</div>
						<div class="hidden md:flex items-center gap-1 flex-shrink-0">
							<Button size="sm" onclick={() => openPlayer(r)}>
								<Play class="w-3.5 h-3.5" /> Play
							</Button>
							<Button
								variant="ghost"
								size="sm"
								onclick={() => download(r)}
								aria-label="Download"
								title="Download as .cast"
							>
								<Download class="w-3.5 h-3.5" />
								<span class="hidden lg:inline">Download</span>
							</Button>
							<Button
								variant="ghost"
								size="sm"
								class="text-error"
								onclick={() => remove(r)}
								disabled={r.status === 'recording'}
								aria-label="Delete"
								title={r.status === 'recording'
									? 'Still recording: end the session first'
									: 'Delete this recording'}
							>
								<Trash2 class="w-3.5 h-3.5" />
								<span class="hidden lg:inline">Delete</span>
							</Button>
						</div>
					</li>
				{/each}
			</ul>
			{#if hasMore}
				<div class="flex justify-center mt-4">
					<Button variant="secondary" onclick={loadMore} disabled={loadingMore}>
						{loadingMore ? 'Loading…' : 'Show older recordings'}
					</Button>
				</div>
			{/if}
		{/if}
	</div>
</div>

{#if playing}
	<RecordingPlayer recording={playing} onClose={closePlayer} onDownload={download} />
{/if}
