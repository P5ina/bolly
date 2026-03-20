<script lang="ts">
	import { Canvas, extend } from '@threlte/core';
	import * as THREE from 'three/webgpu';
	import SphereOverlayContent from './SphereOverlayContent.svelte';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();

	extend(THREE);

	let renderMode = $state<'manual' | 'always'>('manual');
	let rendererReady = $state(false);
</script>

<!-- Normal page content — fully interactive, also the capture source -->
<div id="page-capture-source" class="page-layer">
	{@render children()}
</div>

<!-- WebGPU sphere canvas -->
{#if typeof window !== 'undefined'}
<div class="sphere-layer">
	<Canvas
		{renderMode}
		createRenderer={(canvas) => {
			const renderer = new THREE.WebGPURenderer({
				canvas,
				antialias: true,
				forceWebGL: false,
			});

			renderer.init().then(() => {
				console.log('[SphereOverlay] WebGPU renderer ready');
				renderMode = 'always';
				rendererReady = true;
			}).catch((e: unknown) => {
				console.error('[SphereOverlay] WebGPU init failed:', e);
			});

			renderer.toneMapping = THREE.NoToneMapping;
			return renderer;
		}}
	>
		{#if rendererReady}
			<SphereOverlayContent />
		{/if}
	</Canvas>
</div>
{/if}

<style>
	.page-layer {
		position: relative;
		z-index: 1;
	}

	.sphere-layer {
		position: fixed;
		inset: 0;
		width: 100vw;
		height: 100vh;
		z-index: 5;
		pointer-events: none;
	}

	.sphere-layer :global(canvas) {
		width: 100% !important;
		height: 100% !important;
		pointer-events: none;
	}
</style>
