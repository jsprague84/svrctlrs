<script lang="ts">
	import { onMount } from 'svelte';
	import { Trash2, UserPlus } from 'lucide-svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Input from '$lib/components/ui/Input.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';
	import * as usersApi from '$lib/api/users.js';
	import type { UserAccount } from '$lib/api/users.js';
	import * as toast from '$lib/state/toast.svelte.js';
	import { extractErrorMessage } from '$lib/utils/error.js';

	let users = $state<UserAccount[]>([]);
	let loading = $state(true);

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

	onMount(load);

	async function load() {
		loading = true;
		try {
			users = await usersApi.listUsers();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to load users'));
		} finally {
			loading = false;
		}
	}

	async function create() {
		if (!newUsername.trim() || newPassword.length < MIN_PASSWORD_LEN) return;
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
		if (!confirm(`Delete user ${user.username}? Their profiles and quick commands go with them.`)) return;
		try {
			await usersApi.deleteUser(user.id);
			toast.success(`User ${user.username} deleted`);
			await load();
		} catch (e) {
			toast.error(extractErrorMessage(e, 'Failed to delete user'));
		}
	}

	async function changePassword() {
		if (nextPassword !== nextPasswordRepeat) {
			toast.error('New passwords do not match');
			return;
		}
		if (nextPassword.length < MIN_PASSWORD_LEN) return;
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
	<div class="p-4 bg-surface border border-border rounded-lg">
		<h3 class="text-[11px] uppercase tracking-wider text-text-muted font-semibold mb-3">Your password</h3>
		<form class="flex flex-col gap-3 max-w-sm" onsubmit={(e) => { e.preventDefault(); changePassword(); }}>
			<Input label="Current password" type="password" bind:value={currentPassword} required autocomplete="current-password" />
			<Input label={`New password (at least ${MIN_PASSWORD_LEN} characters)`} type="password" bind:value={nextPassword} required autocomplete="new-password" />
			<Input label="Repeat new password" type="password" bind:value={nextPasswordRepeat} required autocomplete="new-password" />
			<div>
				<Button type="submit" disabled={changing || !currentPassword || nextPassword.length < MIN_PASSWORD_LEN}>
					{changing ? 'Changing...' : 'Change password'}
				</Button>
			</div>
		</form>
	</div>

	<div class="p-4 bg-surface border border-border rounded-lg">
		<h3 class="text-[11px] uppercase tracking-wider text-text-muted font-semibold mb-1">Accounts</h3>
		<p class="text-xs text-text-muted mb-3">
			Every account can open a shell on every configured server and manage these accounts; there are
			no roles yet. Create accounts only for people you would hand root to.
		</p>
		{#if loading}
			<div class="text-text-muted text-sm">Loading...</div>
		{:else}
			<ul class="flex flex-col gap-2 mb-4">
				{#each users as user (user.id)}
					<li class="flex items-center gap-3 border border-border/50 rounded-sm px-3 py-2">
						<span class="text-sm text-text-primary font-medium flex-1 truncate">{user.username}</span>
						{#if user.is_me}<Badge variant="info">you</Badge>{/if}
						<span class="text-[11px] text-text-muted hidden md:inline">since {new Date(user.created_at).toLocaleDateString()}</span>
						<Button variant="ghost" size="sm" onclick={() => remove(user)} disabled={user.is_me || users.length <= 1} title={user.is_me ? 'You cannot delete your own account' : 'Delete user'}>
							<Trash2 class="w-3.5 h-3.5 text-error" />
						</Button>
					</li>
				{/each}
			</ul>
		{/if}
		<form class="flex flex-col md:flex-row md:items-end gap-3" onsubmit={(e) => { e.preventDefault(); create(); }}>
			<Input label="Username" bind:value={newUsername} required placeholder="alice" autocomplete="off" />
			<Input label={`Password (${MIN_PASSWORD_LEN}+ characters)`} type="password" bind:value={newPassword} required autocomplete="new-password" />
			<Button type="submit" disabled={creating || !newUsername.trim() || newPassword.length < MIN_PASSWORD_LEN}>
				<UserPlus class="w-4 h-4" /> Add
			</Button>
		</form>
	</div>
</div>
