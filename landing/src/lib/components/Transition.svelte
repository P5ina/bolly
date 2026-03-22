<script lang="ts">
	import { onMount } from 'svelte';

	let sectionEl: HTMLElement | undefined = $state();
	let videoEl: HTMLVideoElement | undefined = $state();
	let progress = $state(0);

	onMount(() => {
		if (!sectionEl || !videoEl) return;

		let ticking = false;
		function onScroll() {
			if (!ticking) {
				requestAnimationFrame(() => {
					if (!sectionEl || !videoEl) return;
					const rect = sectionEl.getBoundingClientRect();
					const h = sectionEl.offsetHeight - window.innerHeight;
					const p = Math.max(0, Math.min(1, -rect.top / h));
					progress = p;
					if (videoEl.duration) {
						videoEl.currentTime = p * videoEl.duration;
					}
					ticking = false;
				});
				ticking = true;
			}
		}

		window.addEventListener('scroll', onScroll, { passive: true });
		onScroll();
		return () => window.removeEventListener('scroll', onScroll);
	});
</script>

<section class="transition" bind:this={sectionEl}>
	<div class="transition-sticky">
		<video
			bind:this={videoEl}
			muted
			playsinline
			preload="auto"
			class="transition-video"
			src="/assets/transition-shatter.mp4"
		></video>
		<div class="transition-text" style="opacity: {Math.min(1, progress * 3)}; transform: scale({0.9 + progress * 0.1}) translateY({(1 - Math.min(1, progress * 3)) * 30}px);">
			<span class="transition-line">what if your AI</span>
			<span class="transition-line transition-accent" style="opacity: {Math.min(1, Math.max(0, (progress - 0.2) * 4))};">actually knew you?&nbsp;</span>
		</div>
	</div>
</section>

<style>
	.transition {
		height: 300vh;
		position: relative;
	}

	.transition-sticky {
		position: sticky;
		top: 0;
		height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		overflow: visible;
		background: oklch(0.02 0.01 260);
	}

	.transition-video {
		width: 100%;
		height: 100%;
		object-fit: cover;
		filter: brightness(0.7);
	}

	.transition-text {
		position: absolute;
		z-index: 2;
		text-align: center;
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		will-change: transform, opacity;
		padding: 0 2rem;
		max-width: 100%;
	}

	.transition-line {
		display: block;
		font-family: var(--font-display);
		font-weight: 300;
		font-style: italic;
		font-size: clamp(2.5rem, 6vw, 5rem);
		letter-spacing: -0.04em;
		line-height: 1.1;
		color: oklch(0.95 0.01 75);
		text-shadow: 0 2px 20px oklch(0 0 0 / 80%), 0 4px 40px oklch(0 0 0 / 50%);
	}

	.transition-accent {
		background: linear-gradient(135deg, oklch(0.78 0.12 75), oklch(0.72 0.10 55), oklch(0.85 0.14 85));
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
	}

	@media (max-width: 768px) {
		.transition-line {
			font-size: clamp(1.75rem, 8vw, 3rem);
		}
	}
</style>
