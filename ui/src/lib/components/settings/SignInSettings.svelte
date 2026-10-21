<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { Copy, Download, KeyRound, Link2, ShieldCheck, Trash2 } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import Input from '#lib/components/ui/Input.svelte';
	import Modal from '#lib/components/ui/Modal.svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import SettingsSection from '#lib/components/ui/SettingsSection.svelte';
	import * as signInApi from '#lib/api/signIn.js';
	import type { LinkedIdentity, SignInMethods, TotpEnrolment } from '#lib/api/signIn.js';
	import { getServerUrl } from '#lib/platform/index.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { timeAgo } from '#lib/utils/timeAgo.js';

	let methods = $state<SignInMethods | null>(null);
	let loading = $state(true);
	let loadError = $state<string | null>(null);

	// The two-factor dialog walks through: password → scan and confirm → recovery codes.
	// The same dialog asks for the password to turn it off or renew the codes.
	type Step = 'password' | 'scan' | 'codes';
	type Purpose = 'enable' | 'disable' | 'renew';
	let dialogOpen = $state(false);
	let purpose = $state<Purpose>('enable');
	let step = $state<Step>('password');
	let password = $state('');
	let code = $state('');
	let enrolment = $state<TotpEnrolment | null>(null);
	let recoveryCodes = $state<string[]>([]);
	let busy = $state(false);

	// Linking navigates to the provider; the browser is the only client that can follow it
	const canLink = !getServerUrl();

	let qrSrc = $derived(
		enrolment ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(enrolment.qr_svg)}` : ''
	);
	let codeLooksRight = $derived(/^\d{6}$/.test(code.replace(/\s/g, '')));
	let dialogTitle = $derived(
		step === 'codes'
			? 'Recovery codes'
			: purpose === 'enable'
				? 'Turn on two-factor sign-in'
				: purpose === 'disable'
					? 'Turn off two-factor sign-in'
					: 'New recovery codes'
	);

	onMount(async () => {
		await load();
		if (page.url?.searchParams.get('linked') === '1' && methods?.provider) {
			toast.success(`${methods.provider.name} linked`);
		}
	});

	async function load() {
		loading = true;
		loadError = null;
		try {
			methods = await signInApi.getSignInMethods();
		} catch (e) {
			loadError = extractErrorMessage(e, 'Failed to load sign-in methods');
		} finally {
			loading = false;
		}
	}

	function open(p: Purpose) {
		purpose = p;
		step = 'password';
		password = '';
		code = '';
		enrolment = null;
		recoveryCodes = [];
		dialogOpen = true;
	}

	function close() {
		dialogOpen = false;
		password = '';
		code = '';
		enrolment = null;
		recoveryCodes = [];
		load();
	}

	async function submitPassword() {
		if (!password || busy) return;
		busy = true;
		try {
			if (purpose === 'enable') {
				enrolment = await signInApi.beginTotp(password);
				step = 'scan';
			} else if (purpose === 'disable') {
				await signInApi.disableTotp(password);
				toast.success('Two-factor sign-in is off');
				close();
			} else {
				recoveryCodes = await signInApi.renewRecoveryCodes(password);
				step = 'codes';
			}
			password = '';
		} catch (e) {
			toast.error(extractErrorMessage(e, 'That did not work'));
		} finally {
			busy = false;
		}
	}

	async function submitCode() {
		if (!codeLooksRight || busy) return;
		busy = true;
		try {
			recoveryCodes = await signInApi.confirmTotp(code);
			toast.success('Two-factor sign-in is on');
			step = 'codes';
		} catch (e) {
			toast.error(extractErrorMessage(e, 'That code did not work'));
		} finally {
			busy = false;
		}
	}

	async function copy(text: string, what: string) {
		try {
			await navigator.clipboard.writeText(text);
			toast.success(`${what} copied`);
		} catch {
			toast.error('Copying is blocked here; select the text instead');
		}
	}

	function downloadCodes() {
		const host = location.host;
		const text = `SvrCtlRS recovery codes for ${host}\nEach works once instead of an authenticator code.\n\n${recoveryCodes.join('\n')}\n`;
		const url = URL.createObjectURL(new Blob([text], { type: 'text/plain' }));
		const a = document.createElement('a');
		a.href = url;
		a.download = `svrctlrs-recovery-codes-${host}.txt`;
		a.click();
		URL.revokeObjectURL(url);
	}

	function identityLabel(i: LinkedIdentity): string {
		return i.email ?? i.display_name ?? i.issuer;
	}

	async function unlink(i: LinkedIdentity) {
		const ok = await confirmDialog({
			title: `Unlink ${identityLabel(i)}?`,
			message: `Signing in with ${methods?.provider?.name ?? 'the provider'} as this identity stops working until you link it again. Sessions already open stay open.`,
			confirmLabel: 'Unlink',
			destructive: true
		});
		if (!ok) return;
		try {
			await signInApi.unlinkIdentity(i.id);
			toast.success('Identity unlinked');
			await load();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to unlink'));
		}
	}

	function link() {
		window.location.href = signInApi.LINK_URL;
	}
</script>

<SettingsSection
	title="Sign-in"
	description="How you sign in to SvrCtlRS. Agent tokens are separate and keep working whatever is set here."
	card
>
	{#if loading && !methods}
		<StateBlock kind="loading" title="Loading sign-in methods…" compact />
	{:else if loadError}
		<StateBlock kind="error" title={loadError} onRetry={load} compact />
	{:else if methods}
		<div class="flex flex-col gap-4">
			{#if methods.provider}
				<div data-testid="sign-in-provider">
					<div class="flex flex-wrap items-center gap-2 mb-2">
						<Link2 class="w-4 h-4 text-text-muted" />
						<span class="text-sm text-text-primary font-medium">{methods.provider.name}</span>
						{#if methods.identities.length}
							<Badge variant="success">linked</Badge>
						{:else}
							<Badge>not linked</Badge>
						{/if}
					</div>
					{#if methods.identities.length}
						<ul class="flex flex-col gap-2 mb-2">
							{#each methods.identities as identity (identity.id)}
								<li class="flex items-center gap-3 border border-border/50 rounded-sm px-3 py-2">
									<div class="flex-1 min-w-0">
										<div class="text-sm text-text-primary truncate">{identityLabel(identity)}</div>
										<div class="text-[11px] text-text-muted truncate">
											{identity.last_used_at
												? `last sign-in ${timeAgo(identity.last_used_at)}`
												: 'not used yet'}
											· linked {new Date(identity.created_at).toLocaleDateString()}
										</div>
									</div>
									<Button
										variant="ghost"
										size="sm"
										class="min-h-11 min-w-11 md:min-h-0 md:min-w-0"
										onclick={() => unlink(identity)}
										disabled={!methods.password_login && methods.identities.length <= 1}
										aria-label={`Unlink ${identityLabel(identity)}`}
										title={!methods.password_login && methods.identities.length <= 1
											? 'Password sign-in is off, so this is your only way in'
											: 'Unlink'}
									>
										<Trash2 class="w-3.5 h-3.5 text-error" />
									</Button>
								</li>
							{/each}
						</ul>
					{:else}
						<p class="text-xs text-text-muted mb-2">
							Link your {methods.provider.name} account to sign in with it instead of a password.
						</p>
					{/if}
					{#if canLink}
						<Button size="sm" variant="secondary" onclick={link}>
							<Link2 class="w-4 h-4" />
							{methods.identities.length ? 'Link another' : `Link ${methods.provider.name}`}
						</Button>
					{:else}
						<p class="text-xs text-text-muted">Link it from SvrCtlRS in a browser.</p>
					{/if}
				</div>
			{/if}

			<div class="flex flex-wrap items-center gap-2">
				<KeyRound class="w-4 h-4 text-text-muted" />
				<span class="text-sm text-text-primary font-medium">Password</span>
				{#if methods.password_login}
					<Badge variant="info">accepted</Badge>
				{:else}
					<Badge>off on this server</Badge>
				{/if}
			</div>

			<div data-testid="sign-in-totp">
				<div class="flex flex-wrap items-center gap-2 mb-2">
					<ShieldCheck class="w-4 h-4 text-text-muted" />
					<span class="text-sm text-text-primary font-medium">Two-factor (authenticator app)</span>
					{#if methods.totp.enabled}
						<Badge variant="success">on</Badge>
					{:else}
						<Badge>off</Badge>
					{/if}
				</div>
				<p class="text-xs text-text-muted mb-2">
					{#if methods.totp.enabled}
						A password sign-in also asks for a code. {methods.totp.recovery_codes_left} of 10 recovery
						codes left.
					{:else}
						A password sign-in will also ask for a six-digit code from an app such as Aegis, 2FAS or
						Google Authenticator.
					{/if}
				</p>
				<div class="flex flex-wrap gap-2">
					{#if methods.totp.enabled}
						<Button size="sm" variant="secondary" onclick={() => open('renew')}>
							New recovery codes
						</Button>
						<Button size="sm" variant="secondary" onclick={() => open('disable')}>Turn off</Button>
					{:else}
						<Button size="sm" onclick={() => open('enable')}>Turn on</Button>
					{/if}
				</div>
			</div>
		</div>
	{/if}
</SettingsSection>

<Modal open={dialogOpen} title={dialogTitle} onClose={close}>
	{#if step === 'password'}
		<form
			id="sign-in-password-form"
			class="flex flex-col gap-3"
			onsubmit={(e) => {
				e.preventDefault();
				submitPassword();
			}}
		>
			<p class="text-xs text-text-secondary">
				{purpose === 'disable'
					? 'A password alone will sign you in again.'
					: purpose === 'renew'
						? 'Your old recovery codes stop working.'
						: 'Your password first, so a session left open somewhere cannot change this.'}
			</p>
			<Input
				label="Current password"
				type="password"
				bind:value={password}
				required
				autocomplete="current-password"
			/>
		</form>
	{:else if step === 'scan' && enrolment}
		<form
			id="sign-in-code-form"
			class="flex flex-col gap-3"
			onsubmit={(e) => {
				e.preventDefault();
				submitCode();
			}}
		>
			<p class="text-xs text-text-secondary">
				Scan this with your authenticator app, then enter the code it shows.
			</p>
			<img
				src={qrSrc}
				alt="QR code for the authenticator app"
				class="w-48 h-48 self-center rounded-sm bg-white p-1"
			/>
			<a
				href={enrolment.otpauth_uri}
				class="md:hidden text-xs text-info underline self-center min-h-11 flex items-center"
				>Open in the authenticator app on this phone</a
			>
			<div>
				<p class="text-[11px] text-text-muted mb-1">Or type the key in by hand</p>
				<div class="flex items-center gap-2">
					<code
						class="flex-1 min-w-0 text-[11px] font-mono bg-surface-raised border border-border rounded-sm px-2 py-1.5 break-all text-text-secondary"
						>{enrolment.secret}</code
					>
					<Button
						variant="ghost"
						size="sm"
						class="min-h-11 min-w-11 md:min-h-0 md:min-w-0"
						onclick={() => copy(enrolment?.secret ?? '', 'Key')}
						aria-label="Copy the key"
					>
						<Copy class="w-3.5 h-3.5" />
					</Button>
				</div>
			</div>
			<Input
				label="Code from the app"
				bind:value={code}
				required
				inputmode="numeric"
				autocomplete="one-time-code"
				maxlength={7}
				placeholder="123456"
			/>
		</form>
	{:else if step === 'codes'}
		<p class="text-xs text-text-secondary mb-3">
			Each code signs you in once instead of an authenticator code, if you lose the phone. They are
			shown only now; keep them somewhere safe (a password manager).
		</p>
		<ul
			class="grid grid-cols-2 gap-x-4 gap-y-1 font-mono text-sm text-text-primary bg-surface-raised border border-border rounded-sm px-3 py-2"
			data-testid="recovery-codes"
		>
			{#each recoveryCodes as c (c)}
				<li>{c}</li>
			{/each}
		</ul>
	{/if}
	{#snippet footer()}
		{#if step === 'password'}
			<Button variant="secondary" onclick={close}>Cancel</Button>
			<Button
				type="submit"
				form="sign-in-password-form"
				variant={purpose === 'disable' ? 'destructive' : 'primary'}
				disabled={!password || busy}
			>
				{busy ? 'Checking…' : purpose === 'disable' ? 'Turn off' : 'Continue'}
			</Button>
		{:else if step === 'scan'}
			<Button variant="secondary" onclick={close}>Cancel</Button>
			<Button type="submit" form="sign-in-code-form" disabled={!codeLooksRight || busy}>
				{busy ? 'Checking…' : 'Turn on'}
			</Button>
		{:else}
			<Button variant="secondary" onclick={downloadCodes}
				><Download class="w-4 h-4" /> Download</Button
			>
			<Button variant="secondary" onclick={() => copy(recoveryCodes.join('\n'), 'Recovery codes')}>
				<Copy class="w-4 h-4" /> Copy
			</Button>
			<Button onclick={close}>Done</Button>
		{/if}
	{/snippet}
</Modal>
