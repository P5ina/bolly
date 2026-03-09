<script lang="ts">
	import { page } from "$app/state";
	import { fetchMessages } from "$lib/api/client.js";
	import ChatView from "$lib/components/chat/ChatView.svelte";
	import InstanceOnboarding from "$lib/components/onboarding/InstanceOnboarding.svelte";

	const slug = $derived(page.params.slug!);

	let isNew = $state(false);
	let checking = $state(true);

	$effect(() => {
		checking = true;
		isNew = false;

		fetchMessages(slug)
			.then((res) => {
				isNew = res.messages.length === 0;
			})
			.catch(() => {
				isNew = true;
			})
			.finally(() => {
				checking = false;
			});
	});

	function handleOnboardingComplete() {
		isNew = false;
	}
</script>

{#if checking}
	<div class="flex h-full items-center justify-center">
		<div class="h-5 w-5 animate-spin rounded-full border-2 border-warm/30 border-t-warm"></div>
	</div>
{:else if isNew}
	{#key slug}
		<InstanceOnboarding {slug} oncomplete={handleOnboardingComplete} />
	{/key}
{:else}
	{#key slug}
		<ChatView {slug} />
	{/key}
{/if}
