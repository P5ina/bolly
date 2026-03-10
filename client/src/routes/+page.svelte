<script lang="ts">
	import { goto } from "$app/navigation";
	import { onMount } from "svelte";
	import { getInstances } from "$lib/stores/instances.svelte.js";
	import { fetchMeta } from "$lib/api/client.js";
	import Onboarding from "$lib/components/onboarding/Onboarding.svelte";

	const instances = getInstances();

	let version = $state("");
	let commit = $state("");

	onMount(async () => {
		try {
			const meta = await fetchMeta();
			version = meta.version;
			commit = meta.commit;
		} catch {}
	});

	const showOnboarding = $derived(!instances.loading && instances.list.length === 0);

	let newSlug = $state("");
	let showCreate = $state(false);
	let hovered = $state<string | null>(null);

	function create() {
		const slug = newSlug
			.trim()
			.toLowerCase()
			.replace(/[^a-z0-9_-]/g, "-")
			.replace(/-+/g, "-")
			.replace(/^-|-$/g, "");
		if (!slug) return;
		goto(`/${slug}`);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Enter") {
			e.preventDefault();
			create();
		}
		if (e.key === "Escape") {
			showCreate = false;
			newSlug = "";
		}
	}
</script>

{#if showOnboarding}
	<Onboarding />
{:else}
	<div class="home-container">
		<!-- atmospheric background -->
		<div class="home-atmosphere"></div>
		<div class="home-atmosphere-secondary"></div>

		<!-- ambient particles -->
		<div class="home-particles">
			{#each Array(8) as _, i}
				<div class="home-particle" style="--i:{i}; --x:{15 + (i * 11) % 70}; --y:{10 + (i * 17) % 70}"></div>
			{/each}
		</div>

		<div class="relative z-10 flex h-full flex-col items-center justify-center px-6">
			<!-- companion greeting -->
			<div class="home-greeting">
				<h1 class="font-display text-3xl font-light tracking-tight text-foreground/90 italic">
					your companions
				</h1>
				<p class="mt-2 text-sm text-muted-foreground/60">
					{instances.list.length} living {instances.list.length === 1 ? "presence" : "presences"}
				</p>
			</div>

			<!-- instance orbs -->
			<div class="home-orbs">
				{#each instances.list as instance, i (instance.slug)}
					<a
						href="/{instance.slug}"
						class="home-orb"
						style="animation-delay: {i * 120}ms"
						onmouseenter={() => hovered = instance.slug}
						onmouseleave={() => hovered = null}
					>
						<div class="home-orb-glow"></div>
						<div class="home-orb-core">
							<span class="home-orb-letter">{instance.slug[0]?.toUpperCase()}</span>
						</div>
						<div class="home-orb-name" class:home-orb-name-visible={hovered === instance.slug}>
							{instance.companion_name || instance.slug}
						</div>
						{#if instance.soul_exists}
							<div class="home-orb-soul-ring"></div>
						{/if}
					</a>
				{/each}

				<!-- new companion orb -->
				{#if !showCreate}
					<button
						class="home-orb home-orb-new"
						style="animation-delay: {instances.list.length * 120}ms"
						onclick={() => showCreate = true}
						aria-label="New companion"
					>
						<div class="home-orb-core home-orb-core-new">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-5 h-5">
								<path d="M12 5v14M5 12h14" stroke-linecap="round"/>
							</svg>
						</div>
					</button>
				{/if}
			</div>

			<!-- create input -->
			{#if showCreate}
				<div class="home-create">
					<div class="home-create-inner">
						<input
							bind:value={newSlug}
							onkeydown={handleKeydown}
							placeholder="your name..."
							autofocus
							class="home-create-input"
						/>
						{#if newSlug.trim()}
							<button onclick={create} class="home-create-go" aria-label="Create">
								<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-4 h-4">
									<path d="M5 12h14" stroke-linecap="round"/><path d="m12 5 7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
								</svg>
							</button>
						{/if}
					</div>
				</div>
			{/if}

			{#if instances.loading}
				<div class="home-loading">
					<div class="home-loading-dot"></div>
				</div>
			{/if}
		</div>

		{#if version}
			<div class="home-version">
				v{version}{commit && commit !== "dev" ? ` · ${commit.slice(0, 7)}` : ""}
			</div>
		{/if}
	</div>
{/if}

<style>
	.home-container {
		position: relative;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}

	.home-atmosphere {
		position: absolute;
		top: 30%;
		left: 50%;
		width: 600px;
		height: 600px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(
			circle,
			oklch(0.78 0.12 75 / 4%) 0%,
			oklch(0.78 0.12 75 / 1.5%) 30%,
			transparent 65%
		);
		animation: breathe 8s ease-in-out infinite;
		pointer-events: none;
	}

	.home-atmosphere-secondary {
		position: absolute;
		top: 35%;
		left: 48%;
		width: 400px;
		height: 400px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(
			circle,
			oklch(0.70 0.08 300 / 2%) 0%,
			transparent 60%
		);
		animation: breathe 12s ease-in-out infinite;
		animation-delay: -4s;
		pointer-events: none;
	}

	@keyframes breathe {
		0%, 100% { transform: translate(-50%, -50%) scale(1); opacity: 0.7; }
		50% { transform: translate(-50%, -50%) scale(1.08); opacity: 1; }
	}

	/* particles */
	.home-particles {
		position: absolute;
		inset: 0;
		pointer-events: none;
		overflow: hidden;
	}
	.home-particle {
		position: absolute;
		width: 2px;
		height: 2px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 20%);
		left: calc(var(--x) * 1%);
		top: calc(var(--y) * 1%);
		animation: drift 16s ease-in-out infinite;
		animation-delay: calc(var(--i) * -2s);
	}

	/* greeting */
	.home-greeting {
		text-align: center;
		margin-bottom: 3rem;
		animation: page-fade-in 0.6s cubic-bezier(0.16, 1, 0.3, 1) both;
	}
	@keyframes page-fade-in {
		from { opacity: 0; transform: translateY(8px); }
		to { opacity: 1; transform: translateY(0); }
	}

	/* orb grid */
	.home-orbs {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: 2rem;
		max-width: 500px;
	}

	.home-orb {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.75rem;
		text-decoration: none;
		animation: orb-enter 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
		cursor: pointer;
	}

	@keyframes orb-enter {
		from { opacity: 0; transform: scale(0.8) translateY(12px); }
		to { opacity: 1; transform: scale(1) translateY(0); }
	}

	.home-orb-glow {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 80px;
		height: 80px;
		transform: translate(-50%, calc(-50% - 6px));
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.78 0.12 75 / 8%) 0%, transparent 70%);
		transition: all 0.5s ease;
		pointer-events: none;
	}
	.home-orb:hover .home-orb-glow {
		width: 100px;
		height: 100px;
		background: radial-gradient(circle, oklch(0.78 0.12 75 / 14%) 0%, transparent 70%);
	}

	.home-orb-core {
		width: 52px;
		height: 52px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		background: oklch(0.78 0.12 75 / 6%);
		border: 1px solid oklch(0.78 0.12 75 / 12%);
		transition: all 0.4s cubic-bezier(0.16, 1, 0.3, 1);
	}
	.home-orb:hover .home-orb-core {
		background: oklch(0.78 0.12 75 / 12%);
		border-color: oklch(0.78 0.12 75 / 25%);
		box-shadow: 0 0 30px oklch(0.78 0.12 75 / 10%);
		transform: scale(1.08);
	}

	.home-orb-core-new {
		background: oklch(1 0 0 / 3%);
		border: 1px dashed oklch(1 0 0 / 10%);
		color: oklch(0.78 0.12 75 / 30%);
	}
	.home-orb-new:hover .home-orb-core-new {
		background: oklch(0.78 0.12 75 / 6%);
		border-style: solid;
		border-color: oklch(0.78 0.12 75 / 20%);
		color: oklch(0.78 0.12 75 / 60%);
	}

	.home-orb-letter {
		font-family: var(--font-display);
		font-size: 1.125rem;
		font-weight: 500;
		color: oklch(0.78 0.12 75 / 60%);
		font-style: italic;
		transition: color 0.3s ease;
	}
	.home-orb:hover .home-orb-letter {
		color: oklch(0.78 0.12 75 / 90%);
	}

	.home-orb-name {
		font-family: var(--font-body);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 0%);
		letter-spacing: 0.02em;
		transition: all 0.3s ease;
		white-space: nowrap;
	}
	.home-orb-name-visible {
		color: oklch(0.78 0.12 75 / 50%);
	}

	.home-orb-soul-ring {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 60px;
		height: 60px;
		transform: translate(-50%, calc(-50% - 6px));
		border-radius: 50%;
		border: 1px solid oklch(0.78 0.12 75 / 8%);
		animation: soul-ring 6s ease-in-out infinite;
		pointer-events: none;
	}
	@keyframes soul-ring {
		0%, 100% { transform: translate(-50%, calc(-50% - 6px)) scale(1); opacity: 0.6; }
		50% { transform: translate(-50%, calc(-50% - 6px)) scale(1.1); opacity: 0.2; }
	}

	/* create */
	.home-create {
		margin-top: 2rem;
		animation: orb-enter 0.4s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	.home-create-inner {
		position: relative;
	}

	.home-create-input {
		width: 260px;
		padding: 0.75rem 1.25rem;
		border-radius: 2rem;
		border: 1px solid oklch(0.78 0.12 75 / 12%);
		background: oklch(0.78 0.12 75 / 4%);
		color: var(--foreground);
		font-family: var(--font-display);
		font-size: 0.875rem;
		font-style: italic;
		outline: none;
		transition: all 0.4s ease;
	}
	.home-create-input::placeholder {
		color: oklch(0.78 0.12 75 / 20%);
		font-style: italic;
	}
	.home-create-input:focus {
		border-color: oklch(0.78 0.12 75 / 30%);
		box-shadow: 0 0 40px oklch(0.78 0.12 75 / 8%);
	}

	.home-create-go {
		position: absolute;
		right: 0.375rem;
		top: 50%;
		transform: translateY(-50%);
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 60%);
		color: oklch(0.065 0.015 280);
		transition: all 0.2s ease;
	}
	.home-create-go:hover {
		background: oklch(0.78 0.12 75 / 80%);
	}

	/* loading */
	.home-loading {
		margin-top: 3rem;
	}
	.home-loading-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 30%);
		animation: pulse-alive 2s ease-in-out infinite;
	}

	@keyframes drift {
		0%, 100% { transform: translate(0, 0) scale(1); opacity: 0.2; }
		25% { transform: translate(12px, -18px) scale(1.3); opacity: 0.6; }
		50% { transform: translate(-8px, -30px) scale(0.9); opacity: 0.3; }
		75% { transform: translate(16px, -12px) scale(1.15); opacity: 0.5; }
	}

	@keyframes pulse-alive {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.3; }
	}

	.home-version {
		position: absolute;
		bottom: 1.5rem;
		left: 50%;
		transform: translateX(-50%);
		font-family: var(--font-body);
		font-size: 0.65rem;
		letter-spacing: 0.05em;
		color: oklch(0.78 0.12 75 / 20%);
		pointer-events: none;
		animation: page-fade-in 0.6s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 1s;
	}
</style>
