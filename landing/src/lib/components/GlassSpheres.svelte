<script lang="ts">
	import { Canvas } from '@threlte/core';
	import GlassSpheresScene from './GlassSpheresScene.svelte';

	interface Section {
		snippet: import('svelte').Snippet;
		y: number;
		width?: number;
		height?: number;
	}

	let { sections }: { sections: Section[] } = $props();

	// Total scroll height = enough to scroll through all sections
	const lastSection = sections[sections.length - 1];
	const scrollHeight = lastSection ? Math.abs(lastSection.y) * 120 + 1000 : 5000;
</script>

<!-- Invisible spacer to enable scroll -->
<div class="scroll-spacer" style="height: {scrollHeight}px"></div>

<!-- Fixed 3D scene -->
<div class="scene-root">
	<Canvas>
		<GlassSpheresScene {sections} />
	</Canvas>
</div>

<style>
	.scroll-spacer {
		position: relative;
		z-index: -1;
		pointer-events: none;
	}

	.scene-root {
		position: fixed;
		inset: 0;
		width: 100vw;
		height: 100vh;
	}

	.scene-root :global(canvas) {
		pointer-events: none !important;
	}
</style>
