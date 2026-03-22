<script lang="ts">
	import { onMount } from 'svelte';

	let sectionEl: HTMLElement | undefined = $state();
	let progress = $state(0);

	onMount(() => {
		if (!sectionEl) return;
		function onScroll() {
			if (!sectionEl) return;
			const rect = sectionEl.getBoundingClientRect();
			const h = sectionEl.offsetHeight - window.innerHeight;
			progress = Math.max(0, Math.min(1, -rect.top / h));
		}
		window.addEventListener('scroll', onScroll, { passive: true });
		onScroll();
		return () => window.removeEventListener('scroll', onScroll);
	});

	let word1 = $derived(Math.min(1, Math.max(0, (progress - 0.0) / 0.15)));
	let word2 = $derived(Math.min(1, Math.max(0, (progress - 0.15) / 0.15)));
	let word3 = $derived(Math.min(1, Math.max(0, (progress - 0.30) / 0.15)));
	let fade = $derived(Math.min(1, Math.max(0, (progress - 0.55) / 0.2)));
</script>

<section class="scroll-reveal" bind:this={sectionEl}>
	<div class="scroll-sticky">
		<h2 class="scroll-text" style="opacity: {1 - fade}; transform: scale({1 - fade * 0.1});">
			<span class="scroll-word" style="opacity: {word1}; transform: translateY({(1 - word1) * 30}px);">not a chatbot.</span>
			<br />
			<span class="scroll-word" style="opacity: {word2}; transform: translateY({(1 - word2) * 30}px);">a presence that</span>
			<br />
			<span class="scroll-word scroll-accent" style="opacity: {word3}; transform: translateY({(1 - word3) * 30}px);">actually gets you.</span>
		</h2>
	</div>
</section>

<style>
	.scroll-reveal {
		height: 300vh;
		position: relative;
	}

	.scroll-sticky {
		position: sticky;
		top: 0;
		height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.scroll-text {
		font-family: var(--font-display);
		font-weight: 400;
		font-style: italic;
		font-size: clamp(2.5rem, 5.5vw, 4.5rem);
		line-height: 1.2;
		letter-spacing: -0.03em;
		color: var(--color-text);
		text-align: center;
		will-change: transform, opacity;
	}

	.scroll-word {
		display: inline-block;
		will-change: transform, opacity;
	}

	.scroll-accent {
		background: linear-gradient(135deg, oklch(0.78 0.12 75), oklch(0.72 0.10 55), oklch(0.80 0.14 85));
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
	}

	@media (max-width: 768px) {
		.scroll-reveal {
			height: 200vh;
		}

		.scroll-text {
			padding: 0 1.5rem;
		}
	}
</style>
