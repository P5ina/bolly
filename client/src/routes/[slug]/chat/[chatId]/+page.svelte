<script lang="ts">
	import { page } from "$app/state";
	import ChatView from "$lib/components/chat/ChatView.svelte";

	const slug = $derived(page.params.slug!);
	const chatId = $derived(page.params.chatId!);
</script>

{#key `${slug}/${chatId}`}
	<ChatView {slug} {chatId} />
{/key}
