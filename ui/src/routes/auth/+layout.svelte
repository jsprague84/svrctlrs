<script lang="ts">
	import '../../app.css';
	import Toast from '$lib/components/ui/Toast.svelte';

	let { children } = $props();
</script>

<div class="min-h-screen bg-background text-foreground">
	{@render children()}
</div>
<Toast />
