<script lang="ts">
	import { Canvas } from "@threlte/core";
	import CreatureScene from "./CreatureScene.svelte";

	let { thinking = false, mood = "calm", voiceAmplitude = 0 }: { thinking?: boolean; mood?: string; voiceAmplitude?: number } = $props();

	const moodColors: Record<string, string> = {
		calm: "#8ab4f8",
		curious: "#a8d8ea",
		excited: "#f8c471",
		warm: "#f0b27a",
		happy: "#f7dc6f",
		joyful: "#f9e154",
		reflective: "#bb8fce",
		contemplative: "#a993c7",
		melancholy: "#7f8c9a",
		sad: "#6b7b8d",
		worried: "#85929e",
		anxious: "#95a0ab",
		playful: "#82e0aa",
		mischievous: "#58d68d",
		focused: "#76d7c4",
		tired: "#a0937d",
		peaceful: "#aed6f1",
		loving: "#f1948a",
		tender: "#f5b7b1",
		creative: "#d2b4de",
		energetic: "#fad7a0",
	};

	function matchMood(raw: string): string {
		const m = raw.toLowerCase();
		if (moodColors[m]) return m;
		const keys = Object.keys(moodColors).sort((a, b) => b.length - a.length);
		for (const key of keys) {
			if (m.includes(key)) return key;
		}
		return "calm";
	}

	const moodColor = $derived(moodColors[matchMood(mood)]);

	let containerRef = $state<HTMLDivElement | undefined>();
	let canvasRef = $state<HTMLCanvasElement | undefined>();
	let preRef = $state<HTMLPreElement | undefined>();

	const COLS = 40;
	const ROWS = 22;

	const RAMP = " .+xo*#%@";
	const RAMP_LEN = RAMP.length;

	let smoothMin = 0.1;
	let smoothMax = 0.9;
	const SMOOTH = 0.1;

	// Reusable buffer to avoid allocations
	const luminances = new Float32Array(COLS * ROWS);

	function renderAscii() {
		if (!canvasRef || !preRef) return;
		const ctx = canvasRef.getContext("2d", { willReadFrequently: true });
		if (!ctx) return;

		const threlteCanvas = containerRef?.querySelector("canvas") as HTMLCanvasElement | null;
		if (!threlteCanvas) return;
		// Use the buffer size if available, otherwise fall back to CSS layout size
		const cw = threlteCanvas.width || threlteCanvas.clientWidth;
		const ch = threlteCanvas.height || threlteCanvas.clientHeight;
		if (!cw || !ch) return;

		canvasRef.width = COLS;
		canvasRef.height = ROWS;
		ctx.clearRect(0, 0, COLS, ROWS);
		ctx.drawImage(threlteCanvas, 0, 0, COLS, ROWS);

		const imageData = ctx.getImageData(0, 0, COLS, ROWS);
		const data = imageData.data;

		let frameMin = 1;
		let frameMax = 0;

		for (let y = 0; y < ROWS; y++) {
			for (let x = 0; x < COLS; x++) {
				const i = (y * COLS + x) * 4;
				const a = data[i + 3];
				if (a < 80) {
					luminances[y * COLS + x] = -1;
					continue;
				}
				const r = data[i];
				const g = data[i + 1];
				const b = data[i + 2];
				const lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
				luminances[y * COLS + x] = lum;
				if (lum > 0.02) {
					if (lum < frameMin) frameMin = lum;
					if (lum > frameMax) frameMax = lum;
				}
			}
		}

		smoothMin += (frameMin - smoothMin) * SMOOTH;
		smoothMax += (frameMax - smoothMax) * SMOOTH;
		const range = smoothMax - smoothMin;
		const invRange = range > 0.01 ? 1.0 / range : 1.0;

		let result = "";
		for (let y = 0; y < ROWS; y++) {
			for (let x = 0; x < COLS; x++) {
				const lum = luminances[y * COLS + x];
				if (lum < 0) {
					result += " ";
				} else {
					let n = Math.max(0, (lum - smoothMin) * invRange);
					n = Math.pow(n, 0.7);
					const idx = Math.min(Math.floor(n * (RAMP_LEN - 1)), RAMP_LEN - 1);
					result += RAMP[idx];
				}
			}
			result += "\n";
		}

		// Direct DOM update — bypass Svelte reactivity to avoid diffing overhead
		preRef.textContent = result;
	}

	// Render loop — throttled to ~24fps (every other frame) to reduce getImageData cost
	$effect(() => {
		if (!containerRef) return;

		let running = true;
		let skip = false;

		function loop() {
			if (!running) return;
			skip = !skip;
			if (!skip) renderAscii();
			requestAnimationFrame(loop);
		}
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
			<CreatureScene {thinking} {mood} {voiceAmplitude} />
		</Canvas>
	</div>

	<!-- Sampling canvas (hidden) -->
	<canvas bind:this={canvasRef} class="sample-canvas"></canvas>

	<!-- ASCII output — updated via direct DOM manipulation, not Svelte state -->
	<pre
		bind:this={preRef}
		class="ascii-display"
		class:ascii-thinking={thinking}
		style="color: {moodColor}; {thinking ? `text-shadow: 0 0 8px ${moodColor}40, 0 0 20px ${moodColor}20;` : ''}"
	></pre>
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
		width: 300px;
		height: 300px;
		opacity: 0;
		pointer-events: none;
		overflow: hidden;
	}

	.sample-canvas {
		display: none;
	}

	.ascii-display {
		font-family: var(--font-mono);
		font-size: 6px;
		line-height: 7px;
		letter-spacing: 0;
		text-align: center;
		margin: 0;
		user-select: none;
		opacity: 0.8;
		transition: color 0.8s ease, text-shadow 0.8s ease, opacity 0.8s ease;
		white-space: pre;
		contain: strict;
	}

	.ascii-thinking {
		opacity: 1;
	}
</style>
