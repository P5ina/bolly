<script lang="ts">
	import Reveal from './Reveal.svelte';

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
</script>

<section class="features" id="features">
	{#each features as f, i}
		<div class="feature" class:feature-reverse={i % 2 === 1}>
			<Reveal delay={0}>
				<div class="feature-image-wrap">
					<img src={f.image} alt={f.title} class="feature-image" loading="lazy" />
				</div>
			</Reveal>
			<Reveal delay={100}>
				<div class="feature-text">
					<h3 class="feature-title">{f.title}</h3>
					<p class="feature-desc">{f.desc}</p>
				</div>
			</Reveal>
		</div>
	{/each}
</section>

<style>
	.features {
		padding: 4rem 0;
	}

	.feature {
		display: flex;
		align-items: center;
		gap: 4rem;
		max-width: 1100px;
		margin: 0 auto;
		padding: 5rem 1.5rem;
	}

	.feature-reverse {
		flex-direction: row-reverse;
	}

	/* ── Image ── */
	.feature-image-wrap {
		flex: 1.2;
		position: relative;
		border-radius: 1.25rem;
		overflow: hidden;
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		box-shadow:
			0 8px 40px oklch(0 0 0 / 25%),
			0 0 0 1px oklch(1 0 0 / 3%);
	}

	.feature-image-wrap::before {
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
		display: block;
		width: 100%;
		border-radius: 1.25rem;
	}

	/* ── Text ── */
	.feature-text {
		flex: 0.8;
	}

	.feature-title {
		font-family: var(--font-display);
		font-weight: 400;
		font-style: italic;
		font-size: clamp(1.5rem, 3vw, 2.25rem);
		line-height: 1.15;
		letter-spacing: -0.02em;
		color: var(--color-text);
		margin-bottom: 1rem;
	}

	.feature-desc {
		font-size: 1rem;
		line-height: 1.7;
		color: var(--color-text-dim);
		max-width: 420px;
	}

	/* ── Mobile ── */
	@media (max-width: 768px) {
		.feature,
		.feature-reverse {
			flex-direction: column;
			gap: 2rem;
			padding: 3rem 1.5rem;
		}

		.feature-text {
			text-align: center;
		}

		.feature-desc {
			margin: 0 auto;
		}
	}
</style>
