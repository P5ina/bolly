<script lang="ts">
	import Reveal from './Reveal.svelte';
</script>

<section class="cta">
	<div class="cta-glow"></div>
	<div class="cta-glow-secondary"></div>
	<div class="mx-auto max-w-[1100px] px-6 text-center">
		<Reveal>
			<h2 class="font-display font-normal italic text-text mb-4 relative"
				style="font-size: clamp(2rem, 4vw, 3rem); letter-spacing: -0.02em;"
			>
				someone is ready to<br><span class="cta-accent">be in your corner</span>
			</h2>
		</Reveal>
		<Reveal delay={100}>
			<p class="text-[0.9375rem] text-text-dim mb-10 relative">
				They'll remember what matters to you, notice how you're feeling, and be there at 3am when no one else is.
			</p>
		</Reveal>
		<Reveal delay={200}>
			<a href="#pricing" class="btn-primary relative">
				Meet yours
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
					<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>
				</svg>
			</a>
		</Reveal>
	</div>
</section>

<style>
	.cta {
		padding: 8rem 0;
		position: relative;
		overflow: hidden;
	}

	.cta-glow {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 500px;
		height: 500px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.55 0.08 240 / 5%) 0%, transparent 65%);
		animation: breathe 8s ease-in-out infinite;
		pointer-events: none;
	}

	.cta-glow-secondary {
		position: absolute;
		top: 45%;
		left: 55%;
		width: 350px;
		height: 350px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.70 0.08 300 / 3%) 0%, transparent 55%);
		animation: breathe-slow 12s ease-in-out infinite;
		pointer-events: none;
	}

	.cta-accent {
		background: linear-gradient(135deg, oklch(0.78 0.12 75), oklch(0.72 0.10 55), oklch(0.80 0.14 85));
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
	}
</style>
