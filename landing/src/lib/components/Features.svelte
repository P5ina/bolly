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
		},
		{
			title: 'studies with you',
			desc: "Breaks down concepts, quizzes you, tracks what trips you up. A study partner that never gets tired.",
			image: '/assets/feature-study.webp',
		},
		{
			title: 'thinks with you',
			desc: "Talk through ideas and decisions. It pushes back, asks questions, and helps you think deeper.",
			image: '/assets/feature-connection.webp',
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

	onMount(() => {
		if (!sectionEl) return;

		function onScroll() {
			if (!sectionEl) return;
			const rect = sectionEl.getBoundingClientRect();
			const sectionHeight = sectionEl.offsetHeight - window.innerHeight;
			const scrolled = -rect.top;
			const p = Math.max(0, Math.min(1, scrolled / sectionHeight));
			progress = p;
			activeIndex = Math.min(features.length - 1, Math.floor(p * features.length));
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
					<img
						src={f.image}
						alt={f.title}
						class="feature-image"
						class:feature-image-active={i === activeIndex}
						loading="lazy"
					/>
				{/each}
			</div>
		</div>
	</div>
</section>

<style>
	.features {
		height: 400vh;
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
		display: flex;
		align-items: center;
		gap: 4rem;
		max-width: 1100px;
		width: 100%;
		padding: 0 2rem;
	}

	/* ── Text side ── */
	.features-text {
		flex: 0.8;
		position: relative;
		min-height: 200px;
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

	/* ── Image side ── */
	.features-visual {
		flex: 1.2;
		position: relative;
		aspect-ratio: 16 / 9;
		border-radius: 1.25rem;
		overflow: hidden;
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		box-shadow:
			0 8px 40px oklch(0 0 0 / 25%),
			0 0 0 1px oklch(1 0 0 / 3%);
	}

	.features-visual::before {
		content: '';
		position: absolute;
		top: 0;
		left: 8%;
		right: 8%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 18%), transparent);
		pointer-events: none;
		z-index: 2;
	}

	.feature-image {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
		opacity: 0;
		scale: 1.05;
		transition: opacity 0.6s ease, scale 0.6s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.feature-image-active {
		opacity: 1;
		scale: 1;
	}

	/* ── Mobile ── */
	@media (max-width: 768px) {
		.features {
			height: 300vh;
		}

		.features-content {
			flex-direction: column;
			gap: 2rem;
			padding: 0 1.25rem;
		}

		.features-visual {
			width: 100%;
		}

		.features-text {
			text-align: center;
		}

		.feature-desc {
			margin: 0 auto;
		}

		.feature-dots {
			justify-content: center;
		}
	}
</style>
