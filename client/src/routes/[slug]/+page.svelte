<script lang="ts">
	import { page } from "$app/state";
	import ChatView from "$lib/components/chat/ChatView.svelte";

	const slug = $derived(page.params.slug!);
</script>

{#key slug}
	<ChatView {slug} />
{/key}
