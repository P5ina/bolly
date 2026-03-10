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
		<div class="companion-waking-dot"></div>
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

<style>
	.companion-waking-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 30%);
		animation: waking 2s ease-in-out infinite;
	}
	@keyframes waking {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.3; transform: scale(0.7); }
	}
</style>
