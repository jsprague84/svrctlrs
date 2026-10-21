<script lang="ts">
	import { onMount } from 'svelte';
	import { AppWindow, LogOut, Monitor, Smartphone } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import SettingsSection from '#lib/components/ui/SettingsSection.svelte';
	import * as devicesApi from '#lib/api/devices.js';
	import type { Device } from '#lib/api/devices.js';
	import { logout } from '#lib/api/client.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { timeAgo } from '#lib/utils/timeAgo.js';

	let devices = $state<Device[]>([]);
	let loading = $state(true);
	let loadError = $state<string | null>(null);
	let busy = $state(false);

	let others = $derived(devices.filter((d) => !d.current).length);

	/** How it signed in, when that is worth saying (the desktop app says it in its name) */
	const METHODS: Record<string, string> = {
		password: 'password',
		'password+totp': 'password and code',
		'password+recovery_code': 'recovery code',
		oidc: 'provider'
	};

	onMount(load);

	async function load() {
		loading = true;
		loadError = null;
		try {
			devices = await devicesApi.listDevices();
		} catch (e) {
			loadError = extractErrorMessage(e, 'Failed to load devices');
		} finally {
			loading = false;
		}
	}

	function icon(d: Device) {
		if (d.method === 'app') return AppWindow;
		return /Android|iOS|iPadOS/.test(d.label) ? Smartphone : Monitor;
	}

	function details(d: Device): string {
		const parts = [d.current ? 'in use now' : `last used ${timeAgo(d.last_seen_at)}`];
		if (d.last_ip) parts.push(d.last_ip);
		const how = METHODS[d.method];
		parts.push(
			`signed in ${new Date(d.created_at).toLocaleDateString()}${how ? ` with ${how}` : ''}`
		);
		return parts.join(' · ');
	}

	async function signOut(d: Device) {
		const ok = await confirmDialog({
			title: d.current ? 'Sign out of this device?' : `Sign out ${d.label}?`,
			message: d.current
				? 'You go back to the sign-in page.'
				: 'It has to sign in again. Its open terminals close; its shells stay for your other devices.',
			confirmLabel: 'Sign out',
			destructive: true
		});
		if (!ok) return;
		busy = true;
		try {
			await devicesApi.signOutDevice(d.id);
			if (d.current) {
				await logout();
				return;
			}
			toast.success(`${d.label} signed out`);
			await load();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to sign out'));
		} finally {
			busy = false;
		}
	}

	async function signOutOthers() {
		const ok = await confirmDialog({
			title: 'Sign out all other devices?',
			message: `${others} ${others === 1 ? 'device has' : 'devices have'} to sign in again. Their open terminals close; your shells stay.`,
			confirmLabel: 'Sign out others',
			destructive: true
		});
		if (!ok) return;
		busy = true;
		try {
			const n = await devicesApi.signOutOtherDevices();
			toast.success(`${n} ${n === 1 ? 'device' : 'devices'} signed out`);
			await load();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to sign out'));
		} finally {
			busy = false;
		}
	}
</script>

<SettingsSection
	title="Devices"
	description="Where you are signed in. Sign out a lost phone here; agent tokens are listed separately."
	card
>
	{#if loading && !devices.length}
		<StateBlock kind="loading" title="Loading devices…" compact />
	{:else if loadError}
		<StateBlock kind="error" title={loadError} onRetry={load} compact />
	{:else}
		<ul class="flex flex-col gap-2" data-testid="devices">
			{#each devices as device (device.id)}
				{@const Icon = icon(device)}
				<li class="flex items-center gap-3 border border-border/50 rounded-sm px-3 py-2">
					<Icon class="w-4 h-4 flex-shrink-0 text-text-muted" />
					<div class="flex-1 min-w-0">
						<div class="flex items-center gap-2 min-w-0">
							<span class="text-sm text-text-primary truncate">{device.label}</span>
							{#if device.current}
								<Badge variant="info">this device</Badge>
							{/if}
						</div>
						<div class="text-[11px] text-text-muted md:truncate" title={details(device)}>
							{details(device)}
						</div>
					</div>
					<Button
						variant="ghost"
						size="sm"
						class="min-h-11 min-w-11 md:min-h-0 md:min-w-0"
						onclick={() => signOut(device)}
						disabled={busy}
						aria-label={`Sign out ${device.current ? 'this device' : device.label}`}
						title="Sign out"
					>
						<LogOut class="w-3.5 h-3.5 text-error" />
					</Button>
				</li>
			{/each}
		</ul>
		{#if others > 0}
			<div class="mt-3">
				<Button size="sm" variant="secondary" onclick={signOutOthers} disabled={busy}>
					Sign out other devices
				</Button>
			</div>
		{/if}
	{/if}
</SettingsSection>
