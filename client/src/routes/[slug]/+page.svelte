<script lang="ts">
	import { page } from "$app/state";
	import { goto } from "$app/navigation";

	const slug = $derived(page.params.slug!);

	$effect(() => {
		goto(`/${slug}/chat`, { replaceState: true });
	});
</script>
