<script lang="ts">
	import { page } from "$app/state";
	import { fetchMessages } from "$lib/api/client.js";
	import ChatView from "$lib/components/chat/ChatView.svelte";
	import DropsView from "$lib/components/drops/DropsView.svelte";
	import ThoughtsView from "$lib/components/thoughts/ThoughtsView.svelte";
	import InstanceOnboarding from "$lib/components/onboarding/InstanceOnboarding.svelte";

	const slug = $derived(page.params.slug!);

	let isNew = $state(false);
	let checking = $state(true);
	let activeTab = $state<"chat" | "drops" | "thoughts">("chat");

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
	<div class="instance-view">
		<!-- tab switcher -->
		<nav class="instance-tabs">
			<button
				class="instance-tab"
				class:instance-tab-active={activeTab === "chat"}
				onclick={() => (activeTab = "chat")}
			>
				chat
			</button>
			<button
				class="instance-tab"
				class:instance-tab-active={activeTab === "drops"}
				onclick={() => (activeTab = "drops")}
			>
				drops
			</button>
			<button
				class="instance-tab"
				class:instance-tab-active={activeTab === "thoughts"}
				onclick={() => (activeTab = "thoughts")}
			>
				thoughts
			</button>
		</nav>

		<div class="instance-content">
			{#if activeTab === "chat"}
				{#key slug}
					<ChatView {slug} />
				{/key}
			{:else if activeTab === "drops"}
				{#key slug}
					<DropsView {slug} />
				{/key}
			{:else}
				{#key slug}
					<ThoughtsView {slug} />
				{/key}
			{/if}
		</div>
	</div>
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

	.instance-view {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.instance-tabs {
		display: flex;
		gap: 0;
		padding: 0.5rem 1.5rem 0;
		border-bottom: 1px solid oklch(1 0 0 / 4%);
		flex-shrink: 0;
		z-index: 10;
	}

	.instance-tab {
		font-family: var(--font-mono);
		font-size: 0.7rem;
		letter-spacing: 0.05em;
		color: oklch(0.78 0.12 75 / 30%);
		background: none;
		border: none;
		padding: 0.5rem 1rem 0.625rem;
		cursor: pointer;
		position: relative;
		transition: color 0.3s ease;
	}

	.instance-tab:hover {
		color: oklch(0.78 0.12 75 / 55%);
	}

	.instance-tab-active {
		color: oklch(0.78 0.12 75 / 75%);
	}

	.instance-tab-active::after {
		content: "";
		position: absolute;
		bottom: -1px;
		left: 1rem;
		right: 1rem;
		height: 1px;
		background: oklch(0.78 0.12 75 / 30%);
	}

	.instance-content {
		flex: 1;
		min-height: 0;
		overflow: hidden;
	}
</style>
