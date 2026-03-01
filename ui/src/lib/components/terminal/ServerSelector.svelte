<script lang="ts">
	import type { Server } from '$lib/types/index.js';

	interface Props {
		servers: Server[];
		selectedId: number | null;
		onChange: (id: number | null) => void;
	}

	let { servers, selectedId, onChange }: Props = $props();
</script>

<select
	class="px-2 py-1 text-sm bg-input border border-border rounded-sm text-text-primary"
	value={selectedId}
	onchange={(e) => {
		const v = (e.target as HTMLSelectElement).value;
		onChange(v ? Number(v) : null);
	}}
>
	<option value="">Select server...</option>
	{#each servers as server}
		<option value={server.id}>{server.name}{server.hostname ? ` (${server.hostname})` : ''}</option>
	{/each}
</select>
