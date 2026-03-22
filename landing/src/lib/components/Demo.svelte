<script lang="ts">
	import Reveal from './Reveal.svelte';
</script>

<section class="demo" id="demo">
	<div class="demo-inner">
		<Reveal>
			<p class="section-label">See it in action</p>
		</Reveal>
		<Reveal delay={100}>
			<h2 class="section-title">not a demo. a real conversation.</h2>
		</Reveal>
		<Reveal delay={200}>
			<div class="demo-video-wrap">
				<video
					class="demo-video"
					autoplay
					muted
					loop
					playsinline
				>
					<source src="/assets/demo.mp4" type="video/mp4" />
				</video>
			</div>
		</Reveal>
	</div>
</section>

<style>
	.demo {
		padding: 5rem 0 7rem;
		position: relative;
	}

	.demo-inner {
		max-width: 1100px;
		margin: 0 auto;
		padding: 0 1.5rem;
	}

	.section-label {
		font-size: 0.8rem;
		letter-spacing: 0.15em;
		text-transform: uppercase;
		color: var(--color-warm-dim);
		margin-bottom: 1rem;
	}

	.section-title {
		font-family: var(--font-display);
		font-weight: 400;
		font-style: italic;
		font-size: clamp(1.75rem, 3.5vw, 2.5rem);
		line-height: 1.15;
		letter-spacing: -0.02em;
		color: var(--color-text);
		margin-bottom: 2.5rem;
	}

	.demo-video-wrap {
		position: relative;
		border-radius: 1rem;
		overflow: hidden;
		border: 1px solid var(--glass-border);
		border-top-color: var(--glass-border-top);
		box-shadow:
			0 8px 40px oklch(0 0 0 / 30%),
			0 0 0 1px oklch(1 0 0 / 3%);
	}

	/* specular highlight */
	.demo-video-wrap::before {
		content: '';
		position: absolute;
		top: 0;
		left: 8%;
		right: 8%;
		height: 1px;
		background: linear-gradient(90deg, transparent, oklch(1 0 0 / 20%), transparent);
		pointer-events: none;
		z-index: 2;
	}

	.demo-video {
		display: block;
		width: 100%;
		border-radius: 1rem;
	}
</style>
