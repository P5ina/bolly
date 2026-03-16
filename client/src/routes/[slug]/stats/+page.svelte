<script lang="ts">
	import { page } from "$app/state";
	import StatsView from "$lib/components/stats/StatsView.svelte";

	const slug = $derived(page.params.slug!);
</script>

{#key slug}
	<StatsView {slug} />
{/key}
