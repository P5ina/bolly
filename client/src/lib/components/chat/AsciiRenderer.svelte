<script lang="ts">
	import { Canvas } from "@threlte/core";
	import CreatureScene from "./CreatureScene.svelte";

	let { thinking = false, mood = "calm" }: { thinking?: boolean; mood?: string } = $props();

	let containerRef = $state<HTMLDivElement | undefined>();
	let canvasRef = $state<HTMLCanvasElement | undefined>();
	let asciiOutput = $state("");

	const COLS = 48;
	const ROWS = 28;
	// Dense to sparse luminance ramp — reversed so bright = dense
	const ASCII_CHARS = " .'`^\",:;Il!i><~+_-?][}{1)(|\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";

	function luminanceToChar(l: number): string {
		const idx = Math.floor(l * (ASCII_CHARS.length - 1));
		return ASCII_CHARS[Math.min(idx, ASCII_CHARS.length - 1)];
	}

	function renderAscii() {
		if (!canvasRef) return;
		const ctx = canvasRef.getContext("2d", { willReadFrequently: true });
		if (!ctx) return;

		// Find the threlte canvas inside container
		const threlteCanvas = containerRef?.querySelector("canvas");
		if (!threlteCanvas) return;

		// Draw the 3D scene to our small sampling canvas
		canvasRef.width = COLS;
		canvasRef.height = ROWS;
		ctx.drawImage(threlteCanvas, 0, 0, COLS, ROWS);

		const imageData = ctx.getImageData(0, 0, COLS, ROWS);
		const data = imageData.data;

		let result = "";
		for (let y = 0; y < ROWS; y++) {
			for (let x = 0; x < COLS; x++) {
				const i = (y * COLS + x) * 4;
				const r = data[i];
				const g = data[i + 1];
				const b = data[i + 2];
				// Perceived luminance
				const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
				result += luminanceToChar(luminance);
			}
			result += "\n";
		}

		asciiOutput = result;
	}

	// Render loop
	$effect(() => {
		if (!containerRef) return;

		let running = true;
		function loop() {
			if (!running) return;
			renderAscii();
			requestAnimationFrame(loop);
		}
		// Small delay for threlte to initialize
		setTimeout(() => requestAnimationFrame(loop), 100);

		return () => {
			running = false;
		};
	});
</script>

<div class="ascii-creature">
	<!-- Hidden 3D canvas -->
	<div class="threlte-hidden" bind:this={containerRef}>
		<Canvas>
			<CreatureScene {thinking} {mood} />
		</Canvas>
	</div>

	<!-- Sampling canvas (hidden) -->
	<canvas bind:this={canvasRef} class="sample-canvas"></canvas>

	<!-- ASCII output -->
	<pre class="ascii-display" class:ascii-thinking={thinking}>{asciiOutput}</pre>
</div>

<style>
	.ascii-creature {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.threlte-hidden {
		position: absolute;
		width: 200px;
		height: 200px;
		opacity: 0;
		pointer-events: none;
		overflow: hidden;
	}

	.sample-canvas {
		display: none;
	}

	.ascii-display {
		font-family: var(--font-mono);
		font-size: 5.5px;
		line-height: 7px;
		letter-spacing: 1.5px;
		color: oklch(0.78 0.12 75 / 55%);
		text-align: center;
		margin: 0;
		user-select: none;
		transition: color 0.6s ease, text-shadow 0.6s ease;
		white-space: pre;
	}

	.ascii-thinking {
		color: oklch(0.82 0.14 75 / 75%);
		text-shadow:
			0 0 8px oklch(0.78 0.12 75 / 20%),
			0 0 20px oklch(0.78 0.12 75 / 8%);
	}
</style>
