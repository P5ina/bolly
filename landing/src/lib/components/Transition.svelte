<script lang="ts">
	import { onMount } from 'svelte';

	let sectionEl: HTMLElement | undefined = $state();
	let videoEl: HTMLVideoElement | undefined = $state();
	let progress = $state(0);

	onMount(() => {
		if (!sectionEl || !videoEl) return;

		function onScroll() {
			if (!sectionEl || !videoEl) return;
			const rect = sectionEl.getBoundingClientRect();
			const h = sectionEl.offsetHeight - window.innerHeight;
			const p = Math.max(0, Math.min(1, -rect.top / h));
			progress = p;
			if (videoEl.duration) {
				videoEl.currentTime = p * videoEl.duration;
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
		overflow: hidden;
		background: oklch(0.02 0.01 260);
	}

	.transition-video {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
</style>
