<script lang="ts">
	import { onMount } from 'svelte';
	import { Trash2, UserPlus } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import StateBlock from '#lib/components/ui/StateBlock.svelte';
	import Input from '#lib/components/ui/Input.svelte';
	import Badge from '#lib/components/ui/Badge.svelte';
	import SettingsSection from '#lib/components/ui/SettingsSection.svelte';
	import * as usersApi from '#lib/api/users.js';
	import type { UserAccount } from '#lib/api/users.js';
	import * as toast from '#lib/state/toast.svelte.js';
	import { confirmDialog } from '#lib/state/confirm.svelte.js';
	import { extractErrorMessage } from '#lib/utils/error.js';
	import { timeAgo } from '#lib/utils/timeAgo.js';

	let users = $state<UserAccount[]>([]);
	let loading = $state(true);
	let loadError = $state<string | null>(null);

	// New account
	let newUsername = $state('');
	let newPassword = $state('');
	let creating = $state(false);

	// Own password
	let currentPassword = $state('');
	let nextPassword = $state('');
	let nextPasswordRepeat = $state('');
	let changing = $state(false);

	const MIN_PASSWORD_LEN = 12;

	let newPasswordError = $derived(
		newPassword && newPassword.length < MIN_PASSWORD_LEN
			? `At least ${MIN_PASSWORD_LEN} characters`
			: ''
	);
	let nextPasswordError = $derived(
		nextPassword && nextPassword.length < MIN_PASSWORD_LEN
			? `At least ${MIN_PASSWORD_LEN} characters`
			: ''
	);
	let repeatError = $derived(
		nextPasswordRepeat && nextPasswordRepeat !== nextPassword ? 'Passwords do not match' : ''
	);
	let canChange = $derived(
		!changing &&
			!!currentPassword &&
			nextPassword.length >= MIN_PASSWORD_LEN &&
			nextPasswordRepeat === nextPassword
	);
	let canCreate = $derived(
		!creating && !!newUsername.trim() && newPassword.length >= MIN_PASSWORD_LEN
	);

	onMount(load);

	async function load() {
		loading = true;
		loadError = null;
		try {
			users = await usersApi.listUsers();
		} catch (e) {
			loadError = extractErrorMessage(e, 'Failed to load users');
		} finally {
			loading = false;
		}
	}

	async function create() {
		if (!canCreate) return;
		creating = true;
		try {
			await usersApi.createUser(newUsername.trim(), newPassword);
			toast.success(`User ${newUsername.trim()} created`);
			newUsername = '';
			newPassword = '';
			await load();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to create user'));
		} finally {
			creating = false;
		}
	}

	async function remove(user: UserAccount) {
		const ok = await confirmDialog({
			title: `Delete user ${user.username}?`,
			message:
				'Their sessions end now and their terminal profiles and quick commands are deleted. Audit entries are kept.',
			confirmLabel: 'Delete user',
			destructive: true
		});
		if (!ok) return;
		try {
			await usersApi.deleteUser(user.id);
			toast.success(`User ${user.username} deleted`);
			await load();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to delete user'));
		}
	}

	async function changePassword() {
		if (!canChange) return;
		changing = true;
		try {
			await usersApi.changePassword(currentPassword, nextPassword);
			toast.success('Password changed');
			currentPassword = '';
			nextPassword = '';
			nextPasswordRepeat = '';
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to change password'));
		} finally {
			changing = false;
		}
	}
</script>

<div class="flex flex-col gap-6">
	<SettingsSection title="Your password" card>
		<form
			class="flex flex-col gap-3 max-w-sm"
			onsubmit={(e) => {
				e.preventDefault();
				changePassword();
			}}
		>
			<Input
				label="Current password"
				type="password"
				bind:value={currentPassword}
				required
				autocomplete="current-password"
			/>
			<Input
				label="New password"
				type="password"
				bind:value={nextPassword}
				required
				autocomplete="new-password"
				error={nextPasswordError}
				hint={`At least ${MIN_PASSWORD_LEN} characters`}
			/>
			<Input
				label="Repeat new password"
				type="password"
				bind:value={nextPasswordRepeat}
				required
				autocomplete="new-password"
				error={repeatError}
			/>
			<div>
				<Button type="submit" disabled={!canChange}>
					{changing ? 'Changing…' : 'Change password'}
				</Button>
			</div>
		</form>
	</SettingsSection>

	<SettingsSection
		title="Accounts"
		description="Every account can open a shell on every configured server and manage these accounts; there are no roles yet. Create accounts only for people you would hand root to."
		card
	>
		{#if loading}
			<StateBlock kind="loading" title="Loading accounts…" compact />
		{:else if loadError}
			<StateBlock kind="error" title={loadError} onRetry={load} compact />
		{:else}
			<ul class="flex flex-col divide-y divide-border border border-border rounded-md mb-4">
				{#each users as user (user.id)}
					<li class="flex items-center gap-3 px-3 py-2">
						<div class="flex-1 min-w-0">
							<span class="text-sm text-text-primary font-medium truncate">{user.username}</span>
							{#if user.is_me}<Badge variant="info">you</Badge>{/if}
							<div class="text-[11px] text-text-muted">
								{#if user.last_login_at}
									last login {timeAgo(user.last_login_at)}
								{:else}
									no login recorded
								{/if}
								· since {new Date(user.created_at).toLocaleDateString()}
							</div>
						</div>
						<Button
							variant="ghost"
							size="sm"
							class="min-h-11 min-w-11 md:min-h-0 md:min-w-0"
							onclick={() => remove(user)}
							disabled={user.is_me || users.length <= 1}
							aria-label={`Delete user ${user.username}`}
							title={user.is_me
								? 'You cannot delete your own account'
								: users.length <= 1
									? 'The last account cannot be deleted'
									: 'Delete user'}
						>
							<Trash2 class="w-3.5 h-3.5 text-error" />
						</Button>
					</li>
				{/each}
			</ul>
		{/if}
		<form
			class="flex flex-col md:flex-row md:items-start gap-3"
			onsubmit={(e) => {
				e.preventDefault();
				create();
			}}
		>
			<div class="flex-1">
				<Input
					label="Username"
					bind:value={newUsername}
					required
					placeholder="alice"
					autocomplete="off"
				/>
			</div>
			<div class="flex-1">
				<Input
					label="Password"
					type="password"
					bind:value={newPassword}
					required
					autocomplete="new-password"
					error={newPasswordError}
					hint={`At least ${MIN_PASSWORD_LEN} characters`}
				/>
			</div>
			<div class="md:pt-6">
				<Button type="submit" disabled={!canCreate}>
					<UserPlus class="w-4 h-4" />
					{creating ? 'Adding…' : 'Add account'}
				</Button>
			</div>
		</form>
	</SettingsSection>
</div>
