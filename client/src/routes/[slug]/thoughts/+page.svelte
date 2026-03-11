<script lang="ts">
	import { page } from "$app/state";
	import ThoughtsView from "$lib/components/thoughts/ThoughtsView.svelte";

	const slug = $derived(page.params.slug!);
</script>

{#key slug}
	<ThoughtsView {slug} />
{/key}
