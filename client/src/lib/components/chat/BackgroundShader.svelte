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

		float hash(vec2 p) {
			return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
		}

		float noise(vec2 p) {
			vec2 i = floor(p);
			vec2 f = fract(p);
			f = f * f * (3.0 - 2.0 * f);
			float a = hash(i);
			float b = hash(i + vec2(1.0, 0.0));
			float c = hash(i + vec2(0.0, 1.0));
			float d = hash(i + vec2(1.0, 1.0));
			return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
		}

		float fbm(vec2 p) {
			float v = 0.0;
			float a = 0.5;
			mat2 rot = mat2(0.8, 0.6, -0.6, 0.8);
			for (int i = 0; i < 5; i++) {
				v += a * noise(p);
				p = rot * p * 2.0;
				a *= 0.5;
			}
			return v;
		}

		void main() {
			vec2 uv = vUv;
			float t = uTime * (0.025 + uEnergy * 0.02);

			// Domain warp — organic flow
			vec2 warp = vec2(
				fbm(uv * 3.0 + t * 0.7),
				fbm(uv * 3.0 + t * 0.7 + 5.2)
			);

			// Layered noise fields
			float n1 = fbm(uv * 2.0 + warp * 0.5 + t * 0.4);
			float n2 = fbm(uv * 3.5 - t * 0.25 + 10.0);
			float n3 = fbm(uv * 1.8 + warp * 0.3 + t * 0.15 + 20.0);

			// Dark palette
			vec3 abyss  = vec3(0.008, 0.008, 0.018);
			vec3 navy   = vec3(0.035, 0.05, 0.14);
			vec3 indigo = vec3(0.065, 0.03, 0.15);
			vec3 teal   = vec3(0.02, 0.085, 0.10);
			vec3 deep   = vec3(0.04, 0.02, 0.08);

			// Composite
			vec3 color = abyss;
			color = mix(color, navy,   smoothstep(0.28, 0.68, n1) * 0.75);
			color = mix(color, indigo,  smoothstep(0.35, 0.78, n2) * 0.50);
			color = mix(color, teal,    smoothstep(0.30, 0.72, n3) * 0.45);
			color = mix(color, deep,    smoothstep(0.5, 0.9, n1 * n2) * 0.30);

			// Mood accent — Bolly's warmth bleeds through the environment
			float accentMask = smoothstep(0.3, 0.75, n1) * smoothstep(0.25, 0.65, n3);
			vec3 moodGlow = uAccent * uAccent; // square for richer saturation in darks
			color += moodGlow * 0.09 * accentMask;
			// Secondary diffuse wash at larger scale
			float accentWash = smoothstep(0.2, 0.6, fbm(uv * 1.2 + t * 0.08 + 30.0));
			color += uAccent * 0.03 * accentWash;

			// Vignette
			float vig = 1.0 - length((vUv - 0.5) * 1.4);
			vig = smoothstep(0.0, 0.65, vig);
			color *= 0.6 + vig * 0.4;

			// Energy boost
			color *= 1.0 + uEnergy * 0.25;

			// Film grain
			float grain = (hash(vUv * 500.0 + fract(uTime * 100.0)) - 0.5) * 0.012;
			color += grain;

			gl_FragColor = vec4(max(color, 0.0), 1.0);
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
