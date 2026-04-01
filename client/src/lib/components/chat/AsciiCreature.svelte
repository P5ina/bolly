<script lang="ts">
	import { Canvas } from "@threlte/core";
	import CreatureScene from "./CreatureScene.svelte";

	let { thinking = false, mood = "calm" }: { thinking?: boolean; mood?: string } = $props();
</script>

<div class="ascii-creature-wrap">
	<Canvas>
		<CreatureScene {thinking} {mood} />
	</Canvas>
	<div class="ascii-overlay"></div>
</div>

<style>
	.ascii-creature-wrap {
		position: relative;
		width: 180px;
		height: 180px;
		margin: 0 auto;
		/* ASCII post-processing via CSS */
		image-rendering: pixelated;
	}

	.ascii-creature-wrap :global(canvas) {
		width: 100% !important;
		height: 100% !important;
		image-rendering: pixelated;
	}

	/* subtle scanline overlay for CRT/ASCII feel */
	.ascii-overlay {
		position: absolute;
		inset: 0;
		pointer-events: none;
		background: repeating-linear-gradient(
			0deg,
			transparent 0px,
			transparent 2px,
			oklch(var(--shade) / 8%) 2px,
			oklch(var(--shade) / 8%) 4px
		);
		mix-blend-mode: multiply;
	}
</style>
