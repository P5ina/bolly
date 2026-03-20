<script lang="ts">
	import { Canvas } from '@threlte/core';
	import GlassSpheresScene from './GlassSpheresScene.svelte';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<div class="scene-root">
	<Canvas>
		<GlassSpheresScene>
			{@render children()}
		</GlassSpheresScene>
	</Canvas>
</div>

<style>
	.scene-root {
		position: fixed;
		inset: 0;
		width: 100vw;
		height: 100vh;
	}

	/* Required for occlude="blending" */
	.scene-root :global(canvas) {
		pointer-events: none !important;
	}
</style>
