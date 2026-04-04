<script lang="ts">
	import { page } from "$app/stores";
	import ObservationsView from "$lib/components/observations/ObservationsView.svelte";

	const slug = $derived($page.params.slug!);
</script>

{#key slug}
	<ObservationsView {slug} />
{/key}
