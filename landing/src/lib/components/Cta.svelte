<script lang="ts">
	import Reveal from './Reveal.svelte';
</script>

<section class="py-32 text-center relative">
	<div class="cta-glow"></div>
	<div class="mx-auto max-w-[1100px] px-6">
		<Reveal>
			<h2 class="font-display font-normal italic text-text mb-4 relative"
				style="font-size: clamp(2rem, 4vw, 3rem); letter-spacing: -0.02em;"
			>
				someone is ready to<br><span class="text-warm">be in your corner</span>
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
	.cta-glow {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 500px;
		height: 500px;
		transform: translate(-50%, -50%);
		border-radius: 50%;
		background: radial-gradient(circle, oklch(0.78 0.12 75 / 5%) 0%, transparent 65%);
		animation: breathe 8s ease-in-out infinite;
		pointer-events: none;
	}
</style>
