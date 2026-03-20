<script lang="ts">
	import { Canvas } from '@threlte/core';
	import GlassSpheresScene from './GlassSpheresScene.svelte';
</script>

<!-- Scroll spacer -->
<div class="scroll-spacer"></div>

<!-- Fixed 3D scene -->
<div class="scene-root">
	<Canvas>
		<GlassSpheresScene />
	</Canvas>
</div>

<style>
	.scroll-spacer {
		height: 500vh;
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
