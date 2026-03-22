<script lang="ts">
	import Nav from '$lib/components/Nav.svelte';
	import Hero from '$lib/components/Hero.svelte';
	import Demo from '$lib/components/Demo.svelte';
	import Features from '$lib/components/Features.svelte';
	import HowItWorks from '$lib/components/HowItWorks.svelte';
	import Pricing from '$lib/components/Pricing.svelte';
	import Cta from '$lib/components/Cta.svelte';
	import Footer from '$lib/components/Footer.svelte';

	let glowX = $state(0);
	let glowY = $state(0);
	let glowVisible = $state(false);

	function handleMouse(e: MouseEvent) {
		glowX = e.clientX;
		glowY = e.clientY;
		if (!glowVisible) glowVisible = true;
	}
</script>

<svelte:window onmousemove={handleMouse} />

<!-- Cursor-following ambient glow (desktop) -->
<div
	class="cursor-glow"
	class:cursor-glow-visible={glowVisible}
	style="--gx: {glowX}px; --gy: {glowY}px;"
	aria-hidden="true"
></div>

<!-- Living atmosphere — breathing gradient orbs -->
<div class="atmosphere" aria-hidden="true">
	<div class="atmo-orb atmo-1"></div>
	<div class="atmo-orb atmo-2"></div>
	<div class="atmo-orb atmo-3"></div>
	<div class="atmo-orb atmo-4"></div>
</div>

<Nav />

<main>
	<Hero />
	<Demo />
	<Features />
	<HowItWorks />
	<Pricing />
	<Cta />
</main>

<Footer />

<style>
	/* ── Cursor glow ── */
	.cursor-glow {
		position: fixed;
		width: 400px;
		height: 400px;
		border-radius: 50%;
		pointer-events: none;
		z-index: 1;
		background: radial-gradient(circle, oklch(0.78 0.12 75 / 4%) 0%, transparent 70%);
		transform: translate(calc(var(--gx) - 200px), calc(var(--gy) - 200px));
		transition: opacity 0.6s ease;
		opacity: 0;
		will-change: transform;
	}

	.cursor-glow-visible {
		opacity: 1;
	}

	@media (max-width: 768px) {
		.cursor-glow { display: none; }
	}

	@media (prefers-reduced-motion: reduce) {
		.cursor-glow { display: none; }
	}

	/* ── Living atmosphere ── */
	.atmosphere {
		position: fixed;
		inset: 0;
		z-index: -1;
		overflow: hidden;
		pointer-events: none;
	}

	.atmo-orb {
		position: absolute;
		border-radius: 50%;
		filter: blur(120px);
		will-change: transform, opacity;
	}

	.atmo-1 {
		width: 700px;
		height: 700px;
		top: -15%;
		left: 25%;
		background: oklch(0.55 0.08 240 / 5%);
		animation: drift-1 24s ease-in-out infinite;
	}

	.atmo-2 {
		width: 550px;
		height: 550px;
		top: 35%;
		right: -8%;
		background: oklch(0.78 0.12 75 / 3.5%);
		animation: drift-2 28s ease-in-out infinite;
		animation-delay: -8s;
	}

	.atmo-3 {
		width: 500px;
		height: 500px;
		bottom: 15%;
		left: -8%;
		background: oklch(0.60 0.10 300 / 3%);
		animation: drift-3 20s ease-in-out infinite;
		animation-delay: -4s;
	}

	.atmo-4 {
		width: 400px;
		height: 400px;
		top: 65%;
		left: 45%;
		background: oklch(0.65 0.08 200 / 2.5%);
		animation: drift-4 26s ease-in-out infinite;
		animation-delay: -12s;
	}

	@media (prefers-reduced-motion: reduce) {
		.atmo-orb { animation: none !important; }
	}

	@keyframes drift-1 {
		0%, 100% { transform: translate(0, 0) scale(1); opacity: 1; }
		33% { transform: translate(40px, -30px) scale(1.05); opacity: 0.7; }
		66% { transform: translate(-25px, 20px) scale(0.95); opacity: 0.9; }
	}

	@keyframes drift-2 {
		0%, 100% { transform: translate(0, 0) scale(1); opacity: 1; }
		33% { transform: translate(-35px, 25px) scale(1.08); opacity: 0.6; }
		66% { transform: translate(20px, -35px) scale(0.97); opacity: 0.85; }
	}

	@keyframes drift-3 {
		0%, 100% { transform: translate(0, 0) scale(1); }
		50% { transform: translate(30px, 30px) scale(1.06); }
	}

	@keyframes drift-4 {
		0%, 100% { transform: translate(0, 0) scale(1); opacity: 1; }
		40% { transform: translate(-20px, -25px) scale(1.04); opacity: 0.7; }
		70% { transform: translate(15px, 20px) scale(0.96); opacity: 0.9; }
	}

	main {
		position: relative;
		z-index: 2;
	}
</style>
