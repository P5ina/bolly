<script lang="ts">
	import { page } from "$app/stores";
	import AgentsView from "$lib/components/agents/AgentsView.svelte";

	const slug = $derived($page.params.slug!);
</script>

{#key slug}
	<AgentsView {slug} />
{/key}
