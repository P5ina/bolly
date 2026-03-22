<script lang="ts">
	import { onMount } from 'svelte';

	const features = [
		{
			title: 'feels your mood',
			desc: "Notices when you're stressed, tired, or excited — not because you said so, but from how you write.",
			image: '/assets/feature-mood.webp',
		},
		{
			title: 'remembers everything',
			desc: "Every conversation, every detail. It builds a living memory of who you are and what matters to you.",
			image: '/assets/feature-memory.webp',
			video: '/assets/feature-memory.mp4',
		},
		{
			title: 'studies with you',
			desc: "Breaks down concepts, quizzes you, tracks what trips you up. A study partner that never gets tired.",
			image: '/assets/feature-study.webp',
		},
		{
			title: 'transparent tool calls',
			desc: "You see everything the agent does — every tool call, every decision. Nothing hidden.",
			image: '/assets/feature-connection.webp',
			video: '/assets/feature-transparent.mp4',
		},
		{
			title: 'checks in on you',
			desc: "Every 45 minutes, it wakes up on its own. Reflects, journals, and sometimes reaches out.",
			image: '/assets/feature-checkin.webp',
		},
		{
			title: 'completely private',
			desc: "Your conversations, memories, and feelings are fully encrypted and private. No one else can access them.",
			image: '/assets/feature-privacy.webp',
		},
	];

	let sectionEl: HTMLElement | undefined = $state();
	let activeIndex = $state(0);
	let progress = $state(0);
	let videoRefs: Record<number, HTMLVideoElement> = {};

	onMount(() => {
		if (!sectionEl) return;

		function onScroll() {
			if (!sectionEl) return;
			const rect = sectionEl.getBoundingClientRect();
			const sectionHeight = sectionEl.offsetHeight - window.innerHeight;
			const scrolled = -rect.top;
			const p = Math.max(0, Math.min(1, scrolled / sectionHeight));
			progress = p;
			const newIndex = Math.min(features.length - 1, Math.floor(p * features.length));
			activeIndex = newIndex;

			// Play/pause videos based on active feature
			for (const [idx, vid] of Object.entries(videoRefs)) {
				if (!vid) continue;
				if (Number(idx) === newIndex) {
					if (vid.paused) {
						vid.currentTime = 0;
						vid.play().catch(() => {});
					}
				} else {
					vid.pause();
				}
			}
		}

		window.addEventListener('scroll', onScroll, { passive: true });
		onScroll();
		return () => window.removeEventListener('scroll', onScroll);
	});
</script>

<section class="features" id="features" bind:this={sectionEl}>
	<div class="features-sticky">
		<div class="features-content">
			<div class="features-text">
				{#each features as f, i}
					<div class="feature-item" class:feature-active={i === activeIndex}>
						<h3 class="feature-title">{f.title}</h3>
						<p class="feature-desc">{f.desc}</p>
					</div>
				{/each}

				<!-- Progress dots -->
				<div class="feature-dots">
					{#each features as _, i}
						<div class="feature-dot" class:feature-dot-active={i === activeIndex}></div>
					{/each}
				</div>
			</div>

			<div class="features-visual">
				{#each features as f, i}
					{#if f.video}
						<video
							bind:this={videoRefs[i]}
							muted
							loop
							playsinline
							preload="auto"
							class="feature-image"
							class:feature-image-active={i === activeIndex}
							src={f.video}
						></video>
					{:else}
						<img
							src={f.image}
							alt={f.title}
							class="feature-image"
							class:feature-image-active={i === activeIndex}
							loading="lazy"
						/>
					{/if}
				{/each}
			</div>
		</div>
	</div>
</section>

<style>
	.features {
		height: 600vh;
		position: relative;
	}

	.features-sticky {
		position: sticky;
		top: 0;
		height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		overflow: hidden;
	}

	.features-content {
		position: relative;
		width: 100%;
		height: 100%;
	}

	/* ── Text side ── */
	.features-text {
		position: absolute;
		bottom: 4rem;
		left: 4rem;
		z-index: 3;
		max-width: 480px;
	}

	.feature-item {
		position: absolute;
		top: 0;
		left: 0;
		opacity: 0;
		transform: translateY(20px);
		transition: opacity 0.5s cubic-bezier(0.16, 1, 0.3, 1),
					transform 0.5s cubic-bezier(0.16, 1, 0.3, 1);
		pointer-events: none;
	}

	.feature-active {
		opacity: 1;
		transform: translateY(0);
		pointer-events: auto;
		position: relative;
	}

	.feature-title {
		font-family: var(--font-display);
		font-weight: 400;
		font-style: italic;
		font-size: clamp(1.75rem, 3.5vw, 2.75rem);
		line-height: 1.1;
		letter-spacing: -0.02em;
		color: var(--color-text);
		margin-bottom: 1rem;
	}

	.feature-desc {
		font-size: 1.0625rem;
		line-height: 1.7;
		color: var(--color-text-dim);
		max-width: 400px;
	}

	/* ── Dots ── */
	.feature-dots {
		display: flex;
		gap: 0.5rem;
		margin-top: 2.5rem;
	}

	.feature-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: oklch(1 0 0 / 12%);
		transition: all 0.3s ease;
	}

	.feature-dot-active {
		background: var(--color-warm);
		box-shadow: 0 0 8px oklch(0.78 0.12 75 / 40%);
		transform: scale(1.3);
	}

	/* ── Visual — fullscreen ── */
	.features-visual {
		position: absolute;
		inset: 0;
		z-index: 1;
	}

	.feature-image {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
		opacity: 0;
		scale: 1.05;
		transition: opacity 0.8s ease, scale 0.8s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.feature-image-active {
		opacity: 1;
		scale: 1;
	}

	/* Dark gradient overlay for text readability */
	.features-visual::after {
		content: '';
		position: absolute;
		inset: 0;
		background: linear-gradient(
			to top,
			oklch(0.02 0.01 260 / 90%) 0%,
			oklch(0.02 0.01 260 / 40%) 40%,
			transparent 70%
		);
		z-index: 2;
		pointer-events: none;
	}

	/* ── Mobile ── */
	@media (max-width: 768px) {
		.features {
			height: 800vh;
		}

		.features-text {
			left: 1.5rem;
			right: 1.5rem;
			bottom: 2.5rem;
		}

		.feature-title {
			font-size: 1.5rem;
		}
	}
</style>
