<script lang="ts">
	import { Canvas, extend } from '@threlte/core';
	import * as THREE from 'three/webgpu';
	import GlassSceneContent from './GlassSceneContent.svelte';

	extend(THREE);

	let renderMode = $state<'manual' | 'on-demand'>('manual');
</script>

<div class="glass-scene">
	<Canvas
		{renderMode}
		createRenderer={(canvas) => {
			const renderer = new THREE.WebGPURenderer({
				canvas,
				antialias: true,
				forceWebGL: false,
			});

			renderer.init().then(() => {
				renderMode = 'on-demand';
			});

			renderer.toneMapping = THREE.NoToneMapping;

			return renderer;
		}}
	>
		<GlassSceneContent />
	</Canvas>
</div>

<style>
	.glass-scene {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}

	.glass-scene :global(canvas) {
		width: 100% !important;
		height: 100% !important;
	}
</style>
