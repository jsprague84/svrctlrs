<script lang="ts">
	import { onMount } from 'svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Select from '#lib/components/ui/Select.svelte';
	import SettingsSection from '#lib/components/ui/SettingsSection.svelte';
	import { isTauri, getServerUrl } from '#lib/platform/index.js';
	import { forgetAppToken } from '#lib/platform/appToken.js';
	import * as themeState from '#lib/state/theme.svelte.js';
	import * as profilesState from '#lib/state/profiles.svelte.js';
	import * as settingsState from '#lib/state/settings.svelte.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { localTimeOf, utcHour, utcHourLabel } from '#lib/api/backups.js';

	let profiles = $derived(profilesState.getProfiles());
	let defaultProfileId = $state('');
	let retentionDays = $state(30);
	let savedRetention = 30;
	let backupKeep = $state(14);
	let savedKeep = 14;
	let backupHour = $state('3');
	const hours = Array.from({ length: 24 }, (_, h) => h);
	const backupLocal = $derived(localTimeOf(Number(backupHour)));
	let loaded = $state(false);

	onMount(async () => {
		await Promise.all([profilesState.loadProfiles(), settingsState.loadSettings()]);
		defaultProfileId = settingsState.getSetting('app.default_profile_id')?.value ?? '';
		const days = Number(settingsState.getSetting('recording.retention_days')?.value ?? '30');
		retentionDays = savedRetention = Number.isInteger(days) && days >= 0 ? days : 30;
		const keep = Number(settingsState.getSetting('backup.keep')?.value ?? '14');
		backupKeep = savedKeep = Number.isInteger(keep) && keep >= 0 ? keep : 14;
		const hour = Number(settingsState.getSetting('backup.hour')?.value ?? '3');
		backupHour = String(Number.isInteger(hour) && hour >= 0 && hour < 24 ? hour : 3);
		loaded = true;
	});

	async function saveRetention() {
		const days = Number(retentionDays);
		if (!Number.isInteger(days) || days < 0 || days > 3650) {
			toast.error('Keep recordings for 0 to 3650 days');
			retentionDays = savedRetention;
			return;
		}
		if (days === savedRetention) return;
		// A shorter period deletes recordings at the next sweep
		if (days > 0 && (savedRetention === 0 || days < savedRetention)) {
			const ok = await confirmDialog({
				title: `Keep recordings for ${days} day${days === 1 ? '' : 's'}?`,
				message: `Recordings older than that, yours and everyone else's, are deleted within the hour.`,
				confirmLabel: 'Shorten',
				destructive: true
			});
			if (!ok) {
				retentionDays = savedRetention;
				return;
			}
		}
		try {
			await settingsState.updateSetting('recording.retention_days', String(days));
			savedRetention = days;
			toast.success(
				days === 0
					? 'Recordings are kept until deleted'
					: `Recordings are kept for ${days} day${days === 1 ? '' : 's'}`
			);
		} catch (e) {
			retentionDays = savedRetention;
			toast.error(extractErrorMessage(e, 'Failed to save'));
		}
	}

	async function saveBackupKeep() {
		const keep = Number(backupKeep);
		if (!Number.isInteger(keep) || keep < 0 || keep > 365) {
			toast.error('Keep 0 to 365 backups');
			backupKeep = savedKeep;
			return;
		}
		if (keep === savedKeep) return;
		// Fewer copies: the oldest go after the next backup
		if (keep > 0 && keep < savedKeep) {
			const ok = await confirmDialog({
				title: `Keep ${keep} backup${keep === 1 ? '' : 's'}?`,
				message: `After the next backup, copies beyond the newest ${keep} are deleted.`,
				confirmLabel: 'Keep fewer',
				destructive: true
			});
			if (!ok) {
				backupKeep = savedKeep;
				return;
			}
		}
		try {
			await settingsState.updateSetting('backup.keep', String(keep));
			savedKeep = keep;
			toast.success(
				keep === 0
					? 'Nightly backups are off'
					: `Keeping ${keep} nightly backup${keep === 1 ? '' : 's'}`
			);
		} catch (e) {
			backupKeep = savedKeep;
			toast.error(extractErrorMessage(e, 'Failed to save'));
		}
	}

	async function saveBackupHour() {
		try {
			await settingsState.updateSetting('backup.hour', backupHour);
			toast.success(`Nightly backups run at ${utcHourLabel(Number(backupHour))}`);
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to save'));
		}
	}

	async function saveDefaultProfile() {
		try {
			await settingsState.updateSetting('app.default_profile_id', defaultProfileId);
			toast.success(defaultProfileId ? 'Default profile set' : 'Default profile cleared');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to save'));
		}
	}

	async function changeServerUrl() {
		const ok = await confirmDialog({
			title: 'Change server?',
			message: 'You will be signed out of this server and asked for a new address.',
			confirmLabel: 'Change server'
		});
		if (!ok) return;
		await forgetAppToken();
		localStorage.removeItem('svrctlrs-server-url');
		window.location.reload();
	}
</script>

<div class="flex flex-col gap-6 max-w-2xl">
	{#if isTauri()}
		<SettingsSection title="Server" card>
			<div class="flex items-center gap-3">
				<span class="text-sm text-text-secondary flex-1 font-mono truncate"
					>{getServerUrl() || 'Not configured'}</span
				>
				<Button size="sm" variant="secondary" onclick={changeServerUrl}>Change…</Button>
			</div>
		</SettingsSection>
	{/if}

	<SettingsSection title="Appearance">
		<div class="flex items-center justify-between py-2">
			<div>
				<p class="text-sm text-text-primary">Theme</p>
				<p class="text-xs text-text-muted">The terminal colours follow</p>
			</div>
			<div class="flex gap-1" role="group" aria-label="Theme">
				<button
					type="button"
					aria-pressed={themeState.isDark()}
					class="px-4 md:px-3 py-1 min-h-11 md:min-h-0 text-sm md:text-xs rounded-sm transition-colors {themeState.isDark()
						? 'bg-accent text-accent-foreground font-medium'
						: 'bg-surface-raised text-text-secondary'}"
					onclick={() => {
						if (!themeState.isDark()) themeState.toggleTheme();
					}}>Dark</button
				>
				<button
					type="button"
					aria-pressed={!themeState.isDark()}
					class="px-4 md:px-3 py-1 min-h-11 md:min-h-0 text-sm md:text-xs rounded-sm transition-colors {!themeState.isDark()
						? 'bg-accent text-accent-foreground font-medium'
						: 'bg-surface-raised text-text-secondary'}"
					onclick={() => {
						if (themeState.isDark()) themeState.toggleTheme();
					}}>Light</button
				>
			</div>
		</div>
	</SettingsSection>

	<SettingsSection
		title="Terminal"
		description="Keyboard, font and scrollback preferences are under the gear icon on the terminal page."
	>
		<div class="flex items-center justify-between gap-4 py-2">
			<div>
				<p class="text-sm text-text-primary">Default profile</p>
				<p class="text-xs text-text-muted">Opened automatically when the app starts</p>
			</div>
			<div class="w-48">
				<Select bind:value={defaultProfileId} onchange={saveDefaultProfile} disabled={!loaded}>
					<option value="">None</option>
					{#each profiles as profile (profile.id)}
						<option value={String(profile.id)}>{profile.name}</option>
					{/each}
				</Select>
			</div>
		</div>
	</SettingsSection>

	<SettingsSection
		title="Recordings"
		description="Switched on per server (Servers → Edit → Record sessions). Everyone sees only their own recordings."
	>
		<div class="flex items-center justify-between gap-4 py-2">
			<div>
				<label for="retention-days" class="text-sm text-text-primary">Keep recordings for</label>
				<p class="text-xs text-text-muted">
					Older ones are deleted within the hour. 0 keeps them until deleted.
				</p>
			</div>
			<div class="flex items-center gap-2 flex-shrink-0">
				<input
					id="retention-days"
					type="number"
					inputmode="numeric"
					min="0"
					max="3650"
					step="1"
					bind:value={retentionDays}
					onchange={saveRetention}
					disabled={!loaded}
					class="w-20 px-3 py-1.5 min-h-11 md:min-h-0 text-base md:text-sm bg-input border border-border rounded-sm text-text-primary"
				/>
				<span class="text-sm text-text-secondary">days</span>
			</div>
		</div>
	</SettingsSection>

	<SettingsSection
		title="Backups"
		description="A copy of the database every night, on the server's data volume. Settings → About shows the last one."
	>
		<div class="flex items-center justify-between gap-4 py-2">
			<div>
				<label for="backup-keep" class="text-sm text-text-primary">Copies to keep</label>
				<p class="text-xs text-text-muted">0 turns nightly backups off.</p>
			</div>
			<input
				id="backup-keep"
				type="number"
				inputmode="numeric"
				min="0"
				max="365"
				step="1"
				bind:value={backupKeep}
				onchange={saveBackupKeep}
				disabled={!loaded}
				class="w-20 px-3 py-1.5 min-h-11 md:min-h-0 text-base md:text-sm bg-input border border-border rounded-sm text-text-primary flex-shrink-0"
			/>
		</div>
		<div class="flex items-center justify-between gap-4 py-2">
			<div>
				<label for="backup-hour" class="text-sm text-text-primary">Time</label>
				<p class="text-xs text-text-muted">
					{#if backupLocal}{backupLocal} your time.
					{/if}Missed while the server was down: made up when it starts.
				</p>
			</div>
			<div class="w-36 flex-shrink-0">
				<Select
					id="backup-hour"
					bind:value={backupHour}
					onchange={saveBackupHour}
					disabled={!loaded || backupKeep === 0}
				>
					{#each hours as h (h)}
						<option value={String(h)}>{utcHour(h)}</option>
					{/each}
				</Select>
			</div>
		</div>
	</SettingsSection>
</div>
