<script lang="ts">
	import { onMount } from "svelte";
	import * as THREE from "three";

	let { mood = "calm", thinking = false }: { mood?: string; thinking?: boolean } = $props();

	let container: HTMLDivElement | undefined = $state();

	const moodColors: Record<string, [number, number, number]> = {
		calm:         [0.54, 0.71, 0.97],
		curious:      [0.66, 0.85, 0.92],
		excited:      [0.97, 0.77, 0.44],
		warm:         [0.94, 0.70, 0.48],
		happy:        [0.97, 0.86, 0.44],
		reflective:   [0.73, 0.56, 0.81],
		melancholy:   [0.50, 0.55, 0.60],
		sad:          [0.42, 0.48, 0.55],
		playful:      [0.51, 0.88, 0.67],
		focused:      [0.46, 0.84, 0.77],
		creative:     [0.82, 0.71, 0.87],
		energetic:    [0.98, 0.84, 0.63],
		loving:       [0.95, 0.58, 0.54],
		peaceful:     [0.68, 0.84, 0.95],
		anxious:      [0.58, 0.63, 0.67],
		tired:        [0.63, 0.58, 0.49],
		tender:       [0.96, 0.72, 0.69],
		contemplative:[0.66, 0.58, 0.78],
		mischievous:  [0.35, 0.84, 0.55],
		joyful:       [0.98, 0.88, 0.33],
		worried:      [0.52, 0.57, 0.62],
	};

	function getMoodColor(m: string): [number, number, number] {
		const key = m.toLowerCase();
		if (moodColors[key]) return moodColors[key];
		for (const k of Object.keys(moodColors)) {
			if (key.includes(k)) return moodColors[k];
		}
		return moodColors.calm;
	}

	const vertexShader = /* glsl */ `
		varying vec2 vUv;
		void main() {
			vUv = uv;
			gl_Position = vec4(position, 1.0);
		}
	`;

	const fragmentShader = /* glsl */ `
		precision highp float;

		uniform float uTime;
		uniform vec3 uAccent;
		uniform float uEnergy;

		varying vec2 vUv;

		// Smooth wave function — stacked sine curves for organic landscape
		float wave(vec2 uv, float freq, float speed, float phase) {
			return sin(uv.x * freq + uTime * speed + phase)
			     * cos(uv.x * freq * 0.7 + uTime * speed * 0.6 + phase * 1.3)
			     * 0.5 + 0.5;
		}

		// Layered wave field for terrain-like shapes
		float terrain(vec2 uv) {
			float t = uTime * 0.015;
			float w = 0.0;
			w += sin(uv.x * 2.2 + t * 0.8 + uv.y * 0.5) * 0.35;
			w += sin(uv.x * 1.1 - t * 0.5 + 3.0) * 0.25;
			w += sin(uv.x * 3.5 + t * 1.2 + uv.y * 1.5 + 1.0) * 0.15;
			w += sin(uv.x * 0.8 + t * 0.3 - 2.0) * 0.2;
			w += cos(uv.x * 1.8 + uv.y * 2.0 + t * 0.7) * 0.12;
			return w;
		}

		// Specular caustic line — distance to a parametric curve
		float causticLine(vec2 uv) {
			float t = uTime * 0.02;
			// Main curve
			float curve = 0.55 + 0.25 * sin(uv.x * 2.5 + t * 0.6)
			                    + 0.1 * cos(uv.x * 4.0 - t * 0.9 + 1.5)
			                    + 0.08 * sin(uv.x * 6.0 + t * 1.2 + 3.0);
			float dist = abs(uv.y - curve);
			// Thin line with controlled brightness
			float line = exp(-dist * dist * 1200.0) * 0.35;
			// Broader soft glow around line
			float glow = exp(-dist * dist * 60.0) * 0.08;
			return line + glow;
		}

		void main() {
			vec2 uv = vUv;
			float t = uTime * 0.02 + uEnergy * 0.01;

			// --- Base gradient: deep blue bottom → purple/indigo top ---
			vec3 deepBlue  = vec3(0.015, 0.035, 0.14);
			vec3 midBlue   = vec3(0.04, 0.07, 0.24);
			vec3 purple    = vec3(0.10, 0.05, 0.26);
			vec3 indigo    = vec3(0.07, 0.04, 0.22);
			vec3 lightBlue = vec3(0.08, 0.12, 0.28);
			vec3 mist      = vec3(0.12, 0.16, 0.30);

			// Vertical gradient
			vec3 base = mix(deepBlue, midBlue, smoothstep(0.0, 0.4, uv.y));
			base = mix(base, indigo, smoothstep(0.3, 0.7, uv.y));
			base = mix(base, purple, smoothstep(0.6, 1.0, uv.y));

			// --- Flowing wave layers ---
			float w = terrain(uv);

			// Lower mist/light area
			float mistMask = smoothstep(-0.1, 0.3, w - uv.y * 0.8 + 0.2);
			base = mix(base, lightBlue, mistMask * 0.3);

			// Upper wave crest
			float crest = smoothstep(-0.05, 0.15, w - uv.y * 1.2 + 0.5);
			base = mix(base, mist, crest * 0.2);

			// Deep shadow in wave valleys
			float valley = smoothstep(0.2, -0.1, w - uv.y * 0.6 + 0.1);
			base = mix(base, deepBlue * 0.6, valley * 0.4);

			// Secondary wave layer (slower, larger)
			float w2 = terrain(uv * 0.6 + vec2(5.0, 0.0));
			float fold = smoothstep(-0.05, 0.2, w2 - uv.y * 0.9 + 0.35);
			base = mix(base, midBlue * 1.1, fold * 0.2);

			// --- Specular caustic line ---
			float caustic = causticLine(uv);
			vec3 lineColor = vec3(0.35, 0.40, 0.55);
			base += lineColor * caustic;

			// Secondary faint caustic
			float caustic2 = causticLine(uv * vec2(1.0, 1.0) + vec2(0.15, -0.2));
			base += lineColor * caustic2 * 0.1;

			// --- Mood accent bleed ---
			float accentMask = smoothstep(0.3, 0.7, uv.y) * smoothstep(0.3, 0.6, 0.5 + 0.5 * sin(uv.x * 3.0 + uTime * 0.03));
			base += uAccent * uAccent * 0.04 * accentMask;

			// --- Energy boost (thinking) ---
			base *= 1.0 + uEnergy * 0.1;
			float energyCaustic = exp(-pow(abs(uv.y - 0.5 - 0.3 * sin(uv.x * 3.0 + uTime * 0.15)), 2.0) * 300.0);
			base += vec3(0.2, 0.25, 0.4) * energyCaustic * uEnergy * 0.2;

			// --- Vignette ---
			float vig = 1.0 - length((uv - 0.5) * vec2(1.2, 1.4));
			vig = smoothstep(-0.1, 0.6, vig);
			base *= 0.7 + vig * 0.3;

			gl_FragColor = vec4(max(base, 0.0), 1.0);
		}
	`;

	// Store uniform refs for reactive updates
	let accentUniform: THREE.Vector3 | null = null;
	let thinkingRef = false;

	// Reactive mood color
	$effect(() => {
		if (!accentUniform) return;
		const [r, g, b] = getMoodColor(mood);
		accentUniform.set(r, g, b);
	});

	// Reactive thinking state — stored in a ref the render loop reads
	$effect(() => {
		thinkingRef = thinking;
	});

	onMount(() => {
		if (!container) return;

		const renderer = new THREE.WebGLRenderer({ antialias: false, alpha: false, powerPreference: "low-power" });
		renderer.setPixelRatio(Math.min(window.devicePixelRatio * 0.5, 1));
		container.appendChild(renderer.domElement);

		const scene = new THREE.Scene();
		const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

		const accent = new THREE.Vector3(...getMoodColor(mood));
		accentUniform = accent;

		const uniforms = {
			uTime: { value: 0 },
			uAccent: { value: accent },
			uEnergy: { value: 0 },
		};

		const geo = new THREE.PlaneGeometry(2, 2);
		const mat = new THREE.ShaderMaterial({ uniforms, vertexShader, fragmentShader });
		const mesh = new THREE.Mesh(geo, mat);
		scene.add(mesh);

		function resize() {
			if (!container) return;
			renderer.setSize(container.clientWidth, container.clientHeight);
		}
		resize();

		const ro = new ResizeObserver(resize);
		ro.observe(container);

		let running = true;
		let skip = false;
		const clock = new THREE.Clock();
		let energy = 0;

		function frame() {
			if (!running) return;
			requestAnimationFrame(frame);

			// ~30fps
			skip = !skip;
			if (skip) return;

			const delta = clock.getDelta();
			uniforms.uTime.value += delta;

			// Smooth energy transition
			const target = thinkingRef ? 1.0 : 0.0;
			energy += (target - energy) * Math.min(delta * 2.0, 0.1);
			uniforms.uEnergy.value = energy;

			renderer.render(scene, camera);
		}
		requestAnimationFrame(frame);

		return () => {
			running = false;
			accentUniform = null;
			ro.disconnect();
			geo.dispose();
			mat.dispose();
			renderer.dispose();
			renderer.domElement.remove();
		};
	});
</script>

<div class="bg-shader" bind:this={container}></div>

<style>
	.bg-shader {
		position: absolute;
		inset: 0;
		z-index: 0;
		pointer-events: none;
	}

	.bg-shader :global(canvas) {
		display: block;
		width: 100% !important;
		height: 100% !important;
	}
</style>
