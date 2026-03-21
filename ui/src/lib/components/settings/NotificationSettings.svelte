<script lang="ts">
	import { onMount } from 'svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Input from '$lib/components/ui/Input.svelte';
	import * as settingsState from '$lib/state/settings.svelte.js';
	import * as toast from '$lib/state/toast.svelte.js';
	import { extractErrorMessage } from '$lib/utils/error.js';
	import { post, put } from '$lib/api/client.js';

	let enabled = $state(false);
	let provider = $state('gotify');
	let url = $state('');
	let token = $state('');
	let topic = $state('svrctlrs');
	let healthCheckEnabled = $state(false);
	let healthCheckInterval = $state('300');
	let saving = $state(false);
	let testing = $state(false);

	onMount(() => {
		loadSettings();
	});

	function loadSettings() {
		const settings = settingsState.getSettings();
		const notif = settings['notification'] ?? [];
		for (const s of notif) {
			switch (s.key) {
				case 'notification.enabled': enabled = s.value === 'true'; break;
				case 'notification.provider': provider = s.value || 'gotify'; break;
				case 'notification.url': url = s.value; break;
				case 'notification.token': token = s.value; break;
				case 'notification.topic': topic = s.value || 'svrctlrs'; break;
				case 'notification.health_check_enabled': healthCheckEnabled = s.value === 'true'; break;
				case 'notification.health_check_interval_seconds': healthCheckInterval = s.value || '300'; break;
			}
		}
	}

	async function save() {
		saving = true;
		try {
			const updates: Record<string, string> = {
				'notification.enabled': String(enabled),
				'notification.provider': provider,
				'notification.url': url,
				'notification.token': token,
				'notification.topic': topic,
				'notification.health_check_enabled': String(healthCheckEnabled),
				'notification.health_check_interval_seconds': healthCheckInterval
			};
			for (const [key, value] of Object.entries(updates)) {
				await put(`/settings/${key}`, { value });
			}
			toast.success('Notification settings saved');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to save'));
		} finally {
			saving = false;
		}
	}

	async function testNotification() {
		testing = true;
		try {
			await post('/notifications/test');
			toast.success('Test notification sent!');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to send test'));
		} finally {
			testing = false;
		}
	}
</script>

<div class="flex flex-col gap-6">
	<!-- Enable toggle -->
	<div class="flex items-center justify-between py-2 border-b border-border/50">
		<div>
			<p class="text-sm text-text-primary">Enable Notifications</p>
			<p class="text-[10px] text-text-muted">Send alerts via rstify/Gotify/ntfy</p>
		</div>
		<label class="relative inline-flex items-center cursor-pointer">
			<input type="checkbox" bind:checked={enabled} class="sr-only peer" />
			<div class="w-9 h-5 bg-surface-raised rounded-full peer peer-checked:bg-success transition-colors after:content-[''] after:absolute after:top-0.5 after:left-0.5 after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:after:translate-x-4"></div>
		</label>
	</div>

	<!-- Provider -->
	<div>
		<label class="block text-[11px] text-text-muted mb-1">Provider</label>
		<select bind:value={provider} class="w-full px-3 py-1.5 text-sm bg-input border border-border rounded-sm text-text-primary">
			<option value="gotify">Gotify (rstify compatible)</option>
			<option value="ntfy">ntfy</option>
		</select>
	</div>

	<!-- URL -->
	<Input label="Server URL" bind:value={url} placeholder="https://rstify.example.com" class="font-mono" />

	<!-- Token -->
	<div>
		<label class="block text-[11px] text-text-muted mb-1">Application Token</label>
		<input type="password" bind:value={token} class="w-full px-3 py-1.5 text-sm bg-input border border-border rounded-sm text-text-primary" placeholder="Paste your app token" />
	</div>

	<!-- Topic (ntfy only) -->
	{#if provider === 'ntfy'}
		<Input label="Topic" bind:value={topic} placeholder="svrctlrs" />
	{/if}

	<!-- Health checks -->
	<div class="border-t border-border/50 pt-4">
		<div class="flex items-center justify-between py-2">
			<div>
				<p class="text-sm text-text-primary">Server Health Checks</p>
				<p class="text-[10px] text-text-muted">Notify when servers go down/up</p>
			</div>
			<label class="relative inline-flex items-center cursor-pointer">
				<input type="checkbox" bind:checked={healthCheckEnabled} class="sr-only peer" />
				<div class="w-9 h-5 bg-surface-raised rounded-full peer peer-checked:bg-success transition-colors after:content-[''] after:absolute after:top-0.5 after:left-0.5 after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:after:translate-x-4"></div>
			</label>
		</div>

		{#if healthCheckEnabled}
			<div class="mt-2">
				<label class="block text-[11px] text-text-muted mb-1">Check Interval</label>
				<select bind:value={healthCheckInterval} class="px-3 py-1.5 text-sm bg-input border border-border rounded-sm text-text-primary">
					<option value="60">Every 1 minute</option>
					<option value="300">Every 5 minutes</option>
					<option value="900">Every 15 minutes</option>
					<option value="1800">Every 30 minutes</option>
				</select>
			</div>
		{/if}
	</div>

	<!-- Actions -->
	<div class="flex gap-2 pt-2">
		<Button variant="secondary" onclick={testNotification} disabled={testing || !url || !token}>
			{testing ? 'Sending...' : '🔔 Test Notification'}
		</Button>
		<Button onclick={save} disabled={saving}>
			{saving ? 'Saving...' : 'Save'}
		</Button>
	</div>
</div>
