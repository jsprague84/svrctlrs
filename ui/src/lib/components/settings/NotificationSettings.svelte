<script lang="ts">
	import { onMount } from 'svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Input from '#lib/components/ui/Input.svelte';
	import Select from '#lib/components/ui/Select.svelte';
	import Toggle from '#lib/components/ui/Toggle.svelte';
	import SettingsSection from '#lib/components/ui/SettingsSection.svelte';
	import * as settingsState from '#lib/state/settings.svelte.js';
	import * as settingsApi from '#lib/api/settings.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { post } from '#lib/api/client.js';

	interface Form {
		enabled: boolean;
		provider: string;
		url: string;
		/** Empty means "keep the stored token" — the API never returns it */
		token: string;
		topic: string;
		healthNotify: boolean;
		healthInterval: string;
		/** Where approval requests go: push, webhook, matrix or none */
		approvalChannel: string;
		/** Secret: empty keeps the stored one */
		webhookUrl: string;
		matrixHomeserver: string;
		matrixRoom: string;
		/** Secret: empty keeps the stored one */
		matrixToken: string;
		linkBase: string;
	}

	const EMPTY: Form = {
		enabled: false,
		provider: 'gotify',
		url: '',
		token: '',
		topic: 'svrctlrs',
		healthNotify: false,
		healthInterval: '300',
		approvalChannel: 'push',
		webhookUrl: '',
		matrixHomeserver: '',
		matrixRoom: '',
		matrixToken: '',
		linkBase: ''
	};

	const HTTP = /^https?:\/\/\S+$/;

	let form = $state<Form>({ ...EMPTY });
	let saved = $state<Form>({ ...EMPTY });
	let tokenSet = $state(false);
	let webhookSet = $state(false);
	let matrixTokenSet = $state(false);
	let testingApproval = $state(false);
	let loading = $state(true);
	let loadError = $state<string | null>(null);
	let saving = $state(false);
	let testing = $state(false);

	let dirty = $derived(JSON.stringify(form) !== JSON.stringify(saved));
	let urlError = $derived(
		form.url && !/^https?:\/\/\S+$/.test(form.url.trim())
			? 'Must start with http:// or https://'
			: ''
	);
	let canTest = $derived(!testing && !urlError && !!form.url.trim() && (tokenSet || !!form.token));
	let approvalUrlError = $derived(
		[form.webhookUrl, form.matrixHomeserver, form.linkBase].some(
			(v) => v.trim() && !HTTP.test(v.trim())
		)
			? 'Addresses must start with http:// or https://'
			: ''
	);

	onMount(load);

	async function load() {
		loading = true;
		loadError = null;
		await settingsState.loadSettings();
		loadError = settingsState.getError();
		const v = (key: string, fallback = '') => settingsState.getSetting(key)?.value || fallback;
		const next: Form = {
			enabled: v('notification.enabled') === 'true',
			provider: v('notification.provider', 'gotify'),
			url: v('notification.url'),
			token: '',
			topic: v('notification.topic', 'svrctlrs'),
			healthNotify: v('notification.health_check_enabled') === 'true',
			healthInterval: v('notification.health_check_interval_seconds', '300'),
			approvalChannel: v('approval.channel', 'push'),
			webhookUrl: '',
			matrixHomeserver: v('approval.matrix_homeserver'),
			matrixRoom: v('approval.matrix_room'),
			matrixToken: '',
			linkBase: v('approval.link_base')
		};
		tokenSet = settingsState.getSetting('notification.token')?.is_set ?? false;
		webhookSet = settingsState.getSetting('approval.webhook_url')?.is_set ?? false;
		matrixTokenSet = settingsState.getSetting('approval.matrix_token')?.is_set ?? false;
		form = { ...next };
		saved = { ...next };
		loading = false;
	}

	async function save() {
		if (saving || urlError || approvalUrlError) return;
		saving = true;
		try {
			// One request; the server skips an empty token so the stored one survives
			await settingsApi.updateSettings({
				'notification.enabled': String(form.enabled),
				'notification.provider': form.provider,
				'notification.url': form.url.trim(),
				'notification.token': form.token,
				'notification.topic': form.topic.trim() || 'svrctlrs',
				'notification.health_check_enabled': String(form.healthNotify),
				'notification.health_check_interval_seconds': form.healthInterval,
				'approval.channel': form.approvalChannel,
				'approval.webhook_url': form.webhookUrl.trim(),
				'approval.matrix_homeserver': form.matrixHomeserver.trim(),
				'approval.matrix_room': form.matrixRoom.trim(),
				'approval.matrix_token': form.matrixToken.trim(),
				'approval.link_base': form.linkBase.trim()
			});
			if (form.token) tokenSet = true;
			if (form.webhookUrl) webhookSet = true;
			if (form.matrixToken) matrixTokenSet = true;
			form.token = '';
			form.webhookUrl = '';
			form.matrixToken = '';
			saved = { ...form };
			toast.success('Notification settings saved');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to save'));
		} finally {
			saving = false;
		}
	}

	/** Sends a sample approval request through the channel on screen, saved or not */
	async function testApproval() {
		testingApproval = true;
		try {
			const res = await post<{ message: string }>('/notifications/test-approval', {
				channel: form.approvalChannel,
				webhook_url: form.webhookUrl.trim(),
				matrix_homeserver: form.matrixHomeserver.trim(),
				matrix_room: form.matrixRoom.trim(),
				matrix_token: form.matrixToken.trim()
			});
			toast.success(res.message);
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to send a test request'));
		} finally {
			testingApproval = false;
		}
	}

	/** Tests what is on screen, saved or not */
	async function testNotification() {
		testing = true;
		try {
			await post('/notifications/test', {
				provider: form.provider,
				url: form.url.trim(),
				token: form.token,
				topic: form.topic.trim() || 'svrctlrs'
			});
			toast.success('Test notification sent');
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to send test'));
		} finally {
			testing = false;
		}
	}
</script>

<div class="flex flex-col gap-6 max-w-2xl">
	{#if loadError}
		<div
			class="flex items-center justify-between gap-3 rounded-sm border border-error/40 bg-error/10 px-3 py-2"
		>
			<span class="text-sm text-error">{loadError}</span>
			<Button size="sm" variant="secondary" onclick={load}>Retry</Button>
		</div>
	{/if}

	<SettingsSection
		title="Push notifications"
		description="Server down/up alerts via Gotify (rstify) or ntfy."
	>
		<div class="flex flex-col gap-3" class:opacity-60={loading}>
			<Toggle bind:checked={form.enabled} label="Enable notifications" disabled={loading} />
			<Select label="Provider" bind:value={form.provider} disabled={loading}>
				<option value="gotify">Gotify (rstify compatible)</option>
				<option value="ntfy">ntfy</option>
			</Select>
			<Input
				label="Server URL"
				bind:value={form.url}
				placeholder="https://rstify.example.com"
				class="font-mono"
				error={urlError}
				disabled={loading}
				inputmode="url"
			/>
			<Input
				label={form.provider === 'ntfy'
					? 'Access token (if the topic is protected)'
					: 'Application token'}
				type="password"
				bind:value={form.token}
				autocomplete="off"
				placeholder={tokenSet
					? '•••••••• (stored — enter a new one to replace)'
					: 'Paste the token'}
				hint={tokenSet ? 'A token is stored; it is never shown again.' : undefined}
				disabled={loading}
			/>
			{#if form.provider === 'ntfy'}
				<Input label="Topic" bind:value={form.topic} placeholder="svrctlrs" disabled={loading} />
			{/if}
		</div>
	</SettingsSection>

	<SettingsSection
		title="Monitoring"
		description="Reachability is always probed — that is what the status dots and the Servers page show. This only decides whether a change gets a push notification."
	>
		<Toggle
			bind:checked={form.healthNotify}
			label="Notify when a server goes down or comes back"
			disabled={loading}
		/>
		<div class="w-56">
			<Select label="Probe interval" bind:value={form.healthInterval} disabled={loading}>
				<option value="60">Every minute</option>
				<option value="300">Every 5 minutes</option>
				<option value="600">Every 10 minutes</option>
			</Select>
		</div>
	</SettingsSection>

	<SettingsSection
		title="Approval requests"
		description="When a token set to ask first wants to delete a server or a credential, or forget host keys, the link to approve or deny it goes here. The agent also gets the link in its answer."
	>
		<div class="flex flex-col gap-3" class:opacity-60={loading}>
			<Select label="Send them" bind:value={form.approvalChannel} disabled={loading}>
				<option value="push">As a push notification (above)</option>
				<option value="matrix">To a Matrix room</option>
				<option value="webhook">To a webhook (n8n, …)</option>
				<option value="none">Nowhere (the agent passes the link on)</option>
			</Select>
			{#if form.approvalChannel === 'webhook'}
				<Input
					label="Webhook URL"
					type="password"
					bind:value={form.webhookUrl}
					autocomplete="off"
					class="font-mono"
					placeholder={webhookSet
						? '•••••••• (stored — enter a new one to replace)'
						: 'https://n8n.example.com/webhook/…'}
					hint="Receives JSON: title, message, url and the request."
					disabled={loading}
				/>
			{:else if form.approvalChannel === 'matrix'}
				<Input
					label="Homeserver"
					bind:value={form.matrixHomeserver}
					placeholder="https://matrix.example.org"
					class="font-mono"
					inputmode="url"
					disabled={loading}
				/>
				<Input
					label="Room id"
					bind:value={form.matrixRoom}
					placeholder="!abc123:example.org"
					class="font-mono"
					disabled={loading}
				/>
				<Input
					label="Access token of the posting account"
					type="password"
					bind:value={form.matrixToken}
					autocomplete="off"
					placeholder={matrixTokenSet ? '•••••••• (stored — enter a new one to replace)' : 'syt_…'}
					hint="Use an account made for alerts, invited to the room."
					disabled={loading}
				/>
			{/if}
			<Input
				label="Links start with"
				bind:value={form.linkBase}
				placeholder={typeof location !== 'undefined' ? location.origin : 'https://svrctlrs.example'}
				class="font-mono"
				inputmode="url"
				hint="Empty: the address the agent called. Set it when agents call an internal address."
				error={approvalUrlError}
				disabled={loading}
			/>
			{#if form.approvalChannel !== 'none'}
				<div>
					<Button variant="secondary" size="sm" onclick={testApproval} disabled={testingApproval}>
						{testingApproval ? 'Sending…' : 'Send a test request'}
					</Button>
				</div>
			{/if}
		</div>
	</SettingsSection>

	<div
		class="sticky bottom-0 -mx-4 md:-mx-6 px-4 md:px-6 py-3 bg-background/95 backdrop-blur border-t border-border flex items-center justify-between gap-2"
	>
		<span class="text-xs text-text-muted">
			{#if dirty}Unsaved changes{:else if loading}Loading…{:else}All changes saved{/if}
		</span>
		<div class="flex gap-2">
			<Button variant="secondary" onclick={testNotification} disabled={!canTest}>
				{testing ? 'Sending…' : 'Send test'}
			</Button>
			<Button onclick={save} disabled={saving || !dirty || !!urlError || !!approvalUrlError}>
				{saving ? 'Saving…' : 'Save changes'}
			</Button>
		</div>
	</div>
</div>
