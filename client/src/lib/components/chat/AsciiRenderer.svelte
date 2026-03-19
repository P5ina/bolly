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
	let asciiOutput = $state("");

	const COLS = 56;
	const ROWS = 32;

	// Only use visually "round" characters — no colons, semicolons, dashes, equals
	// At tiny font sizes those create horizontal line artifacts
	const RAMP = " .+xo*#%@";
	const RAMP_LEN = RAMP.length;

	// Smoothed luminance range to prevent flickering from frame-to-frame jumps
	let smoothMin = 0.1;
	let smoothMax = 0.9;
	const SMOOTH = 0.1; // lerp factor — lower = more stable

	function renderAscii() {
		if (!canvasRef) return;
		const ctx = canvasRef.getContext("2d", { willReadFrequently: true });
		if (!ctx) return;

		const threlteCanvas = containerRef?.querySelector("canvas");
		if (!threlteCanvas) return;

		canvasRef.width = COLS;
		canvasRef.height = ROWS;
		ctx.clearRect(0, 0, COLS, ROWS);
		ctx.drawImage(threlteCanvas, 0, 0, COLS, ROWS);

		const imageData = ctx.getImageData(0, 0, COLS, ROWS);
		const data = imageData.data;

		// First pass: find luminance range
		let frameMin = 1;
		let frameMax = 0;
		const luminances = new Float32Array(COLS * ROWS);

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

		// Smooth the range across frames to prevent flicker
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

	<!-- ASCII output -->
	<pre
		class="ascii-display"
		class:ascii-thinking={thinking}
		style="color: {moodColor}; {thinking ? `text-shadow: 0 0 8px ${moodColor}40, 0 0 20px ${moodColor}20;` : ''}"
	>{asciiOutput}</pre>
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
		opacity: 0.6;
		transition: color 0.8s ease, text-shadow 0.8s ease, opacity 0.8s ease;
		white-space: pre;
	}

	.ascii-thinking {
		opacity: 0.85;
	}
</style>
