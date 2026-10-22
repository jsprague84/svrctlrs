<script lang="ts">
	/**
	 * How long the user's shells are kept with nothing attached (ROADMAP 23): a choice of the
	 * keep times the server offers, saved for the account. Shells already waiting are kept that
	 * long from now. Falls back to a plain sentence while the choices are unknown.
	 */
	import * as sessionsState from '#lib/state/sessions.svelte.js';
	import { formatTtl } from '#lib/utils/sessions.js';
	import { extractErrorMessage } from '#lib/utils/error.js';

	let saving = $state(false);
	let error = $state<string | null>(null);
	let choices = $derived(sessionsState.getTtlChoices());
	let current = $derived(sessionsState.getDetachTtlSecs());

	async function choose(event: Event) {
		const secs = Number((event.currentTarget as HTMLSelectElement).value);
		if (!Number.isFinite(secs) || secs === current) return;
		saving = true;
		error = null;
		try {
			await sessionsState.setKeepTime(secs);
		} catch (e) {
			error = extractErrorMessage(e, 'Could not save the keep time');
		} finally {
			saving = false;
		}
	}
</script>

{#if choices.length > 0}
	<label class="inline-flex flex-wrap items-center gap-1">
		<span>Keep a shell nothing is attached to for</span>
		<select
			class="min-h-11 md:min-h-0 rounded-md border border-border-strong bg-input px-1.5 py-0.5 text-[11px] text-text-primary disabled:opacity-50"
			value={String(current)}
			onchange={choose}
			disabled={saving}
			aria-label="How long a shell nothing is attached to is kept"
		>
			{#if !choices.includes(current)}
				<option value={String(current)}>{formatTtl(current)}</option>
			{/if}
			{#each choices as secs (secs)}
				<option value={String(secs)}>{formatTtl(secs)}</option>
			{/each}
		</select>
		{#if error}<span class="text-error" role="alert">{error}</span>{/if}
	</label>
{:else if current > 0}
	A shell with nothing attached closes after {formatTtl(current)}.
{/if}
