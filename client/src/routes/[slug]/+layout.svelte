<script lang="ts">
	import { page } from "$app/state";
	import { fetchMessages } from "$lib/api/client.js";
	import InstanceOnboarding from "$lib/components/onboarding/InstanceOnboarding.svelte";
	let { children } = $props();

	const slug = $derived(page.params.slug!);

	let isNew = $state(false);
	let checking = $state(true);

	const tabs = ["chat", "drops", "thoughts", "skills"] as const;
	const activeTab = $derived(
		tabs.find((t) => page.url.pathname.includes(`/${slug}/${t}`)) ?? "chat"
	);

	$effect(() => {
		const s = slug;
		checking = true;
		isNew = false;

		fetchMessages(s)
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
		<nav class="instance-tabs">
			<a
				href="/"
				class="instance-tab instance-tab-home"
				title="all companions"
			>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="instance-tab-icon">
					<path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</a>
			{#each tabs as tab}
				<a
					href="/{slug}/{tab}"
					class="instance-tab"
					class:instance-tab-active={activeTab === tab}
				>
					{tab}
				</a>
			{/each}
		</nav>

		<div class="instance-content">
			{@render children()}
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
		max-width: 100%;
		overflow: hidden;
	}

	.instance-tabs {
		display: flex;
		gap: 0;
		padding: 0.5rem 1.5rem 0;
		border-bottom: 1px solid oklch(1 0 0 / 4%);
		flex-shrink: 0;
		z-index: 10;
		overflow-x: auto;
		scrollbar-width: none;
		-webkit-overflow-scrolling: touch;
	}
	.instance-tabs::-webkit-scrollbar {
		display: none;
	}

	@media (max-width: 720px) {
		.instance-tabs {
			padding: 0.4rem 0.75rem 0;
		}
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
		text-decoration: none;
		white-space: nowrap;
		flex-shrink: 0;
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

	.instance-tab-home {
		display: flex;
		align-items: center;
		padding: 0.5rem 0.75rem 0.625rem;
	}

	.instance-tab-icon {
		width: 0.8rem;
		height: 0.8rem;
	}

	.instance-content {
		flex: 1;
		min-height: 0;
		min-width: 0;
		overflow: hidden;
	}
</style>
