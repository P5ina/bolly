<script lang="ts">
	import { page } from "$app/state";
	import DropsView from "$lib/components/drops/DropsView.svelte";

	const slug = $derived(page.params.slug!);
</script>

{#key slug}
	<DropsView {slug} />
{/key}
