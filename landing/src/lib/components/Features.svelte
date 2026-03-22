<script lang="ts">
	import Reveal from './Reveal.svelte';

	const features = [
		{
			title: 'feels your mood',
			desc: "Notices when you're stressed, tired, or excited — not because you said so, but from how you write.",
			video: '/assets/feature-mood.mp4',
		},
		{
			title: 'remembers everything',
			desc: "Every conversation, every detail. It builds a living memory of who you are and what matters to you.",
			video: '/assets/feature-memory.mp4',
		},
		{
			title: 'transparent tool calls',
			desc: "You see everything the agent does — every tool call, every decision. Nothing hidden.",
			video: '/assets/feature-transparent.mp4',
		},
	];
</script>

<section class="features" id="features">
	{#each features as f, i}
		<div class="feature" class:feature-reverse={i % 2 === 1}>
			<Reveal delay={0}>
				<div class="feature-visual">
					<video autoplay muted loop playsinline class="feature-video" src={f.video}></video>
				</div>
			</Reveal>
			<Reveal delay={150}>
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

	.feature-visual {
		flex: 1.2;
		border-radius: 1.25rem;
		overflow: hidden;
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		box-shadow:
			0 8px 40px oklch(0 0 0 / 25%),
			0 0 0 1px oklch(1 0 0 / 3%);
		position: relative;
	}

	.feature-visual::before {
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

	.feature-video {
		display: block;
		width: 100%;
		border-radius: 1.25rem;
	}

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
