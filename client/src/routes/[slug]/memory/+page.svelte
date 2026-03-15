<script lang="ts">
	import { page } from "$app/state";
	import MemoryMapView from "$lib/components/memory/MemoryMapView.svelte";

	const slug = $derived(page.params.slug!);
</script>

{#key slug}
	<MemoryMapView {slug} />
{/key}
