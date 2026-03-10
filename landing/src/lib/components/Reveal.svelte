<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children, delay = 0 }: { children: Snippet; delay?: number } = $props();

	let el: HTMLDivElement | undefined = $state();
	let visible = $state(false);

	$effect(() => {
		if (!el) return;
		const observer = new IntersectionObserver(
			(entries) => {
				if (entries[0].isIntersecting) {
					visible = true;
				}
			},
			{ threshold: 0.1, rootMargin: '0px 0px -40px 0px' }
		);
		observer.observe(el);
		return () => observer.disconnect();
	});
</script>

<div
	bind:this={el}
	class="reveal"
	class:visible
	style="transition-delay: {delay}ms"
>
	{@render children()}
</div>

<style>
	.reveal {
		opacity: 0;
		transform: translateY(24px);
		transition: all 0.8s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.reveal.visible {
		opacity: 1;
		transform: translateY(0);
	}
</style>
