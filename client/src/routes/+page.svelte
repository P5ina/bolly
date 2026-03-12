<script lang="ts">
	import { goto } from "$app/navigation";
	import { onMount } from "svelte";
	import { getInstances } from "$lib/stores/instances.svelte.js";
	import { fetchMeta } from "$lib/api/client.js";

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

	function getGreeting(): string {
		const hour = new Date().getHours();
		if (hour < 6) return "still up?";
		if (hour < 12) return "good morning";
		if (hour < 17) return "good afternoon";
		if (hour < 22) return "good evening";
		return "late night?";
	}

	const qualities = [
		"focus", "study", "think", "create",
		"reflect", "plan", "rest", "breathe",
		"grow", "learn", "feel", "write",
	];
</script>

	<div class="home-container">
		<!-- layered atmosphere -->
		<div class="home-atmosphere"></div>
		<div class="home-atmosphere-secondary"></div>
		<div class="home-atmosphere-tertiary"></div>

		<!-- ambient particles -->
		<div class="home-particles">
			{#each Array(10) as _, i}
				<div class="home-particle" style="--i:{i}; --x:{12 + (i * 13) % 76}; --y:{8 + (i * 19) % 74}"></div>
			{/each}
		</div>

		<!-- floating quality words -->
		<div class="home-qualities">
			{#each qualities as word, i}
				<span
					class="home-quality-word"
					style="--qi:{i}; --qx:{8 + (i * 7.3) % 84}; --qy:{12 + (i * 11.7) % 76}"
				>{word}</span>
			{/each}
		</div>

		<div class="relative z-10 flex h-full flex-col items-center justify-center px-6">
			<!-- hero greeting -->
			<div class="home-hero">
				<p class="home-time-greeting">{getGreeting()}</p>
				<h1 class="home-title">
					your friend that<br/>
					<span class="home-title-accent">actually gets you</span>
				</h1>
				<p class="home-subtitle">
					work, study, mood, mind — always here, always yours
				</p>
			</div>

			<!-- instance orbs -->
			<div class="home-orbs">
				{#each instances.list as instance, i (instance.slug)}
					<a
						href="/{instance.slug}"
						class="home-orb"
						style="animation-delay: {200 + i * 120}ms"
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
						<div class="home-orb-pulse"></div>
					</a>
				{/each}

				<!-- new companion orb -->
				{#if !showCreate}
					<button
						class="home-orb home-orb-new"
						style="animation-delay: {200 + instances.list.length * 120}ms"
						onclick={() => showCreate = true}
						aria-label="New companion"
					>
						<div class="home-orb-core home-orb-core-new">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-5 h-5">
								<path d="M12 5v14M5 12h14" stroke-linecap="round"/>
							</svg>
						</div>
						<div class="home-orb-name home-orb-name-visible">
							new friend
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
							placeholder="give them a name..."
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

			<!-- value hints -->
			<div class="home-hints">
				<div class="home-hint" style="animation-delay: 600ms">
					<span class="home-hint-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-3.5 h-3.5">
							<path d="M12 6.042A8.967 8.967 0 0 0 6 3.75c-1.052 0-2.062.18-3 .512v14.25A8.987 8.987 0 0 1 6 18c2.305 0 4.408.867 6 2.292m0-14.25a8.966 8.966 0 0 1 6-2.292c1.052 0 2.062.18 3 .512v14.25A8.987 8.987 0 0 0 18 18a8.967 8.967 0 0 0-6 2.292m0-14.25v14.25" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
					</span>
					<span>helps you study & learn</span>
				</div>
				<span class="home-hint-dot"></span>
				<div class="home-hint" style="animation-delay: 750ms">
					<span class="home-hint-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-3.5 h-3.5">
							<path d="M9.813 15.904 9 18.75l-.813-2.846a4.5 4.5 0 0 0-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 0 0 3.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 0 0 3.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 0 0-3.09 3.09ZM18.259 8.715 18 9.75l-.259-1.035a3.375 3.375 0 0 0-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 0 0 2.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 0 0 2.455 2.456L21.75 6l-1.036.259a3.375 3.375 0 0 0-2.455 2.456ZM16.894 20.567 16.5 21.75l-.394-1.183a2.25 2.25 0 0 0-1.423-1.423L13.5 18.75l1.183-.394a2.25 2.25 0 0 0 1.423-1.423l.394-1.183.394 1.183a2.25 2.25 0 0 0 1.423 1.423l1.183.394-1.183.394a2.25 2.25 0 0 0-1.423 1.423Z" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
					</span>
					<span>thinks with you</span>
				</div>
				<span class="home-hint-dot"></span>
				<div class="home-hint" style="animation-delay: 900ms">
					<span class="home-hint-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-3.5 h-3.5">
							<path d="M21 8.25c0-2.485-2.099-4.5-4.688-4.5-1.935 0-3.597 1.126-4.312 2.733-.715-1.607-2.377-2.733-4.313-2.733C5.1 3.75 3 5.765 3 8.25c0 7.22 9 12 9 12s9-4.78 9-12Z" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
					</span>
					<span>feels your mood</span>
				</div>
			</div>
		</div>

		{#if version}
			<div class="home-version">
				v{version}{commit && commit !== "dev" ? ` · ${commit.slice(0, 7)}` : ""}
			</div>
		{/if}
	</div>

<style>
	.home-container {
		position: relative;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}

	/* --- atmosphere --- */
	.home-atmosphere {
		position: absolute;
		top: 25%;
		left: 50%;
		width: 700px;
		height: 700px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(
			circle,
			oklch(0.78 0.12 75 / 5%) 0%,
			oklch(0.78 0.12 75 / 2%) 25%,
			transparent 60%
		);
		animation: breathe 8s ease-in-out infinite;
		pointer-events: none;
	}

	.home-atmosphere-secondary {
		position: absolute;
		top: 40%;
		left: 45%;
		width: 500px;
		height: 500px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(
			circle,
			oklch(0.70 0.08 300 / 3%) 0%,
			transparent 55%
		);
		animation: breathe 12s ease-in-out infinite;
		animation-delay: -4s;
		pointer-events: none;
	}

	.home-atmosphere-tertiary {
		position: absolute;
		top: 60%;
		left: 55%;
		width: 350px;
		height: 350px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(
			circle,
			oklch(0.65 0.10 160 / 2%) 0%,
			transparent 60%
		);
		animation: breathe 15s ease-in-out infinite;
		animation-delay: -8s;
		pointer-events: none;
	}

	@keyframes breathe {
		0%, 100% { transform: translate(-50%, -50%) scale(1); opacity: 0.7; }
		50% { transform: translate(-50%, -50%) scale(1.12); opacity: 1; }
	}

	/* --- particles --- */
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
		background: oklch(0.78 0.12 75 / 18%);
		left: calc(var(--x) * 1%);
		top: calc(var(--y) * 1%);
		animation: drift 18s ease-in-out infinite;
		animation-delay: calc(var(--i) * -1.8s);
	}

	/* --- floating quality words --- */
	.home-qualities {
		position: absolute;
		inset: 0;
		pointer-events: none;
		overflow: hidden;
	}
	.home-quality-word {
		position: absolute;
		left: calc(var(--qx) * 1%);
		top: calc(var(--qy) * 1%);
		font-family: var(--font-display);
		font-size: 0.7rem;
		font-weight: 300;
		font-style: italic;
		letter-spacing: 0.08em;
		color: oklch(0.78 0.12 75 / 0%);
		animation: quality-drift 20s ease-in-out infinite;
		animation-delay: calc(var(--qi) * -1.67s);
		user-select: none;
	}
	@keyframes quality-drift {
		0%, 100% {
			transform: translate(0, 0);
			color: oklch(0.78 0.12 75 / 0%);
		}
		15% {
			color: oklch(0.78 0.12 75 / 7%);
		}
		35% {
			transform: translate(8px, -14px);
			color: oklch(0.78 0.12 75 / 5%);
		}
		50% {
			color: oklch(0.78 0.12 75 / 0%);
		}
		65% {
			transform: translate(-6px, -22px);
			color: oklch(0.78 0.12 75 / 6%);
		}
		85% {
			color: oklch(0.78 0.12 75 / 4%);
		}
	}

	/* --- hero --- */
	.home-hero {
		text-align: center;
		margin-bottom: 2.5rem;
		animation: hero-enter 0.8s cubic-bezier(0.16, 1, 0.3, 1) both;
	}
	@keyframes hero-enter {
		from { opacity: 0; transform: translateY(16px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.home-time-greeting {
		font-family: var(--font-display);
		font-size: 0.75rem;
		font-weight: 300;
		font-style: italic;
		letter-spacing: 0.15em;
		text-transform: lowercase;
		color: oklch(0.78 0.12 75 / 40%);
		margin-bottom: 1rem;
		animation: hero-enter 0.6s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 100ms;
	}

	.home-title {
		font-family: var(--font-display);
		font-size: clamp(1.75rem, 5vw, 2.5rem);
		font-weight: 300;
		line-height: 1.2;
		letter-spacing: -0.01em;
		color: oklch(0.88 0.02 75 / 85%);
		animation: hero-enter 0.8s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 150ms;
	}

	.home-title-accent {
		font-style: italic;
		font-weight: 400;
		color: oklch(0.78 0.12 75 / 80%);
	}

	.home-subtitle {
		margin-top: 0.875rem;
		font-family: var(--font-body);
		font-size: 0.825rem;
		letter-spacing: 0.03em;
		color: oklch(0.88 0.02 75 / 30%);
		animation: hero-enter 0.8s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 300ms;
	}

	/* --- orb grid --- */
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
		width: 110px;
		height: 110px;
		background: radial-gradient(circle, oklch(0.78 0.12 75 / 16%) 0%, transparent 70%);
	}

	.home-orb-core {
		width: 56px;
		height: 56px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		background: oklch(0.78 0.12 75 / 6%);
		border: 1px solid oklch(0.78 0.12 75 / 12%);
		transition: all 0.4s cubic-bezier(0.16, 1, 0.3, 1);
	}
	.home-orb:hover .home-orb-core {
		background: oklch(0.78 0.12 75 / 14%);
		border-color: oklch(0.78 0.12 75 / 30%);
		box-shadow: 0 0 40px oklch(0.78 0.12 75 / 12%);
		transform: scale(1.1);
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
		font-size: 1.2rem;
		font-weight: 500;
		color: oklch(0.78 0.12 75 / 60%);
		font-style: italic;
		transition: color 0.3s ease;
	}
	.home-orb:hover .home-orb-letter {
		color: oklch(0.78 0.12 75 / 95%);
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
		color: oklch(0.78 0.12 75 / 45%);
	}

	.home-orb-soul-ring {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 64px;
		height: 64px;
		transform: translate(-50%, calc(-50% - 6px));
		border-radius: 50%;
		border: 1px solid oklch(0.78 0.12 75 / 8%);
		animation: soul-ring 6s ease-in-out infinite;
		pointer-events: none;
	}
	@keyframes soul-ring {
		0%, 100% { transform: translate(-50%, calc(-50% - 6px)) scale(1); opacity: 0.6; }
		50% { transform: translate(-50%, calc(-50% - 6px)) scale(1.15); opacity: 0.15; }
	}

	/* alive pulse on orbs */
	.home-orb-pulse {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 56px;
		height: 56px;
		transform: translate(-50%, calc(-50% - 6px));
		border-radius: 50%;
		border: 1px solid oklch(0.78 0.12 75 / 5%);
		animation: orb-alive 4s ease-in-out infinite;
		pointer-events: none;
	}
	@keyframes orb-alive {
		0%, 100% { transform: translate(-50%, calc(-50% - 6px)) scale(1); opacity: 1; }
		50% { transform: translate(-50%, calc(-50% - 6px)) scale(1.35); opacity: 0; }
	}

	/* --- create --- */
	.home-create {
		margin-top: 2rem;
		animation: orb-enter 0.4s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	.home-create-inner {
		position: relative;
	}

	.home-create-input {
		width: 280px;
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
		box-shadow: 0 0 50px oklch(0.78 0.12 75 / 8%);
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

	/* --- value hints --- */
	.home-hints {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		margin-top: 3rem;
		animation: hero-enter 0.8s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 500ms;
	}

	.home-hint {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		font-family: var(--font-body);
		font-size: 0.65rem;
		letter-spacing: 0.04em;
		color: oklch(0.88 0.02 75 / 22%);
		animation: hero-enter 0.6s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	.home-hint-icon {
		display: flex;
		color: oklch(0.78 0.12 75 / 25%);
	}

	.home-hint-dot {
		width: 2px;
		height: 2px;
		border-radius: 50%;
		background: oklch(0.78 0.12 75 / 15%);
		flex-shrink: 0;
	}

	@media (max-width: 540px) {
		.home-hints {
			flex-direction: column;
			gap: 0.5rem;
		}
		.home-hint-dot {
			display: none;
		}
	}

	/* --- loading --- */
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
		0%, 100% { transform: translate(0, 0) scale(1); opacity: 0.15; }
		25% { transform: translate(14px, -20px) scale(1.3); opacity: 0.5; }
		50% { transform: translate(-10px, -35px) scale(0.9); opacity: 0.25; }
		75% { transform: translate(18px, -14px) scale(1.15); opacity: 0.45; }
	}

	@keyframes pulse-alive {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.3; }
	}

	.home-version {
		position: absolute;
		bottom: calc(1.5rem + env(safe-area-inset-bottom, 0px));
		left: 50%;
		transform: translateX(-50%);
		font-family: var(--font-body);
		font-size: 0.6rem;
		letter-spacing: 0.05em;
		color: oklch(0.78 0.12 75 / 15%);
		pointer-events: none;
		animation: hero-enter 0.6s cubic-bezier(0.16, 1, 0.3, 1) both;
		animation-delay: 1.2s;
	}
</style>
