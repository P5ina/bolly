<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	let { thinking = false, mood = "calm", voiceAmplitude = 0 }: { thinking?: boolean; mood?: string; voiceAmplitude?: number } = $props();

	let container: HTMLDivElement | undefined = $state();

	const moodColors: Record<string, number> = {
		calm: 0x8ab4f8, curious: 0xa8d8ea, excited: 0xf8c471, warm: 0xf0b27a,
		happy: 0xf7dc6f, joyful: 0xf9e154, reflective: 0xbb8fce, contemplative: 0xa993c7,
		melancholy: 0x7f8c9a, sad: 0x6b7b8d, worried: 0x85929e, anxious: 0x95a0ab,
		playful: 0x82e0aa, mischievous: 0x58d68d, focused: 0x76d7c4, tired: 0xa0937d,
		peaceful: 0xaed6f1, loving: 0xf1948a, tender: 0xf5b7b1, creative: 0xd2b4de,
		energetic: 0xfad7a0,
	};

	type Energy = { speed: number; intensity: number; breatheRate: number; breatheDepth: number; rotSpeed: number };
	const moodEnergies: Record<string, Energy> = {
		calm:          { speed: 0.8,  intensity: 0.12, breatheRate: 1.2, breatheDepth: 0.04, rotSpeed: 0.15 },
		excited:       { speed: 2.0,  intensity: 0.20, breatheRate: 2.0, breatheDepth: 0.06, rotSpeed: 0.35 },
		energetic:     { speed: 2.2,  intensity: 0.22, breatheRate: 2.2, breatheDepth: 0.07, rotSpeed: 0.40 },
		playful:       { speed: 1.8,  intensity: 0.18, breatheRate: 1.8, breatheDepth: 0.05, rotSpeed: 0.30 },
		mischievous:   { speed: 1.9,  intensity: 0.19, breatheRate: 1.9, breatheDepth: 0.05, rotSpeed: 0.32 },
		curious:       { speed: 1.2,  intensity: 0.15, breatheRate: 1.5, breatheDepth: 0.05, rotSpeed: 0.22 },
		reflective:    { speed: 0.5,  intensity: 0.08, breatheRate: 0.8, breatheDepth: 0.03, rotSpeed: 0.08 },
		contemplative: { speed: 0.5,  intensity: 0.08, breatheRate: 0.8, breatheDepth: 0.03, rotSpeed: 0.08 },
		melancholy:    { speed: 0.4,  intensity: 0.06, breatheRate: 0.6, breatheDepth: 0.02, rotSpeed: 0.05 },
		sad:           { speed: 0.3,  intensity: 0.05, breatheRate: 0.5, breatheDepth: 0.02, rotSpeed: 0.04 },
		tired:         { speed: 0.3,  intensity: 0.05, breatheRate: 0.5, breatheDepth: 0.02, rotSpeed: 0.04 },
		worried:       { speed: 1.3,  intensity: 0.16, breatheRate: 1.6, breatheDepth: 0.03, rotSpeed: 0.20 },
		anxious:       { speed: 1.5,  intensity: 0.18, breatheRate: 1.8, breatheDepth: 0.03, rotSpeed: 0.25 },
		peaceful:      { speed: 0.6,  intensity: 0.10, breatheRate: 1.0, breatheDepth: 0.04, rotSpeed: 0.10 },
		loving:        { speed: 0.9,  intensity: 0.13, breatheRate: 1.3, breatheDepth: 0.05, rotSpeed: 0.18 },
		warm:          { speed: 0.9,  intensity: 0.13, breatheRate: 1.3, breatheDepth: 0.05, rotSpeed: 0.18 },
	};
	const defaultEnergy: Energy = { speed: 0.8, intensity: 0.12, breatheRate: 1.2, breatheDepth: 0.04, rotSpeed: 0.15 };

	function matchMood(raw: string): string {
		const m = raw.toLowerCase();
		if (moodColors[m] !== undefined) return m;
		for (const k of Object.keys(moodColors).sort((a, b) => b.length - a.length)) {
			if (m.includes(k)) return k;
		}
		return "calm";
	}

	let moodRef = mood;
	let thinkingRef = thinking;
	let voiceRef = voiceAmplitude;
	$effect(() => { moodRef = mood; });
	$effect(() => { thinkingRef = thinking; });
	$effect(() => { voiceRef = voiceAmplitude; });

	let cleanup: (() => void) | null = null;
	onDestroy(() => cleanup?.());

	onMount(async () => {
		if (!container) return;

		// WebGPU imports (auto-fallback to WebGL2)
		const THREE = await import("three/webgpu");
		const TSL = await import("three/tsl");

		const {
			Fn, uniform, float, vec3, vec4, color,
			uv, positionLocal, normalLocal, time,
			sin, cos, dot, pow, mix, smoothstep, abs, max,
		} = TSL;

		const renderer = new THREE.WebGPURenderer({ antialias: true, alpha: false });
		await renderer.init();
		renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
		renderer.toneMapping = THREE.ACESFilmicToneMapping;
		renderer.toneMappingExposure = 1.2;
		container.appendChild(renderer.domElement);

		const scene = new THREE.Scene();
		const cam = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
		cam.position.set(0, 0, 3.0);

		// ── Scene background (wave shader via backgroundNode) ──
		// This is what the glass refracts through

		const bgColor = Fn(() => {
			const st = uv();
			const t = time.mul(0.015);

			const deep = vec3(0.04, 0.08, 0.25);
			const mid = vec3(0.10, 0.16, 0.45);
			const purp = vec3(0.22, 0.12, 0.48);
			const lite = vec3(0.18, 0.25, 0.52);

			// Base vertical gradient
			let c = mix(deep, mid, smoothstep(float(0.0), float(0.4), st.y));
			c = mix(c, purp, smoothstep(float(0.5), float(1.0), st.y));

			// Wave terrain
			const w = sin(st.x.mul(2.2).add(t.mul(0.8)).add(st.y.mul(0.5))).mul(0.35)
				.add(sin(st.x.mul(1.1).sub(t.mul(0.5)).add(3.0)).mul(0.25))
				.add(sin(st.x.mul(3.5).add(t.mul(1.2)).add(st.y.mul(1.5)).add(1.0)).mul(0.15))
				.add(cos(st.x.mul(1.8).add(st.y.mul(2.0)).add(t.mul(0.7))).mul(0.12));

			// Mist
			c = mix(c, lite, smoothstep(float(-0.1), float(0.3), w.sub(st.y.mul(0.8)).add(0.2)).mul(0.3));

			// Caustic line
			const curve = float(0.55).add(sin(st.x.mul(2.5).add(t.mul(0.4))).mul(0.25))
				.add(cos(st.x.mul(4.0).sub(t.mul(0.6))).mul(0.1));
			const dist = abs(st.y.sub(curve));
			const caustic = vec3(0.25, 0.28, 0.38).mul(
				pow(float(2.718), dist.mul(dist).mul(-800.0).negate()).mul(0.3)
			);
			c = c.add(caustic);

			return vec4(c, float(1.0));
		});

		scene.backgroundNode = bgColor();

		// ── Lighting ──

		scene.add(new THREE.AmbientLight(0x4466aa, 0.6));

		const keyLight = new THREE.DirectionalLight(0xffffff, 5.0);
		keyLight.position.set(3, 2, 4);
		scene.add(keyLight);

		const rimLight = new THREE.PointLight(0xaabbee, 3.0);
		rimLight.position.set(-3, 1.5, -2);
		scene.add(rimLight);

		const fillLight = new THREE.PointLight(0x6677aa, 1.0);
		fillLight.position.set(0, -3, 1);
		scene.add(fillLight);

		// Top accent light for specular highlight
		const topLight = new THREE.PointLight(0xffffff, 2.0);
		topLight.position.set(0, 4, 2);
		scene.add(topLight);

		// ── Glass blob ──

		const creatureGeo = new THREE.IcosahedronGeometry(1, 6);
		const basePositions = new Float32Array(creatureGeo.attributes.position.array);

		// Uniforms for animation
		const uSpeed = uniform(0.8);
		const uIntensity = uniform(0.12);
		const uBreathe = uniform(1.0);
		const uMoodColor = uniform(new THREE.Color(moodColors[matchMood(mood)]));

		// GPU vertex displacement via positionNode
		const displacedPos = Fn(() => {
			const pos = positionLocal.toVar();
			const t = time;

			const noise = sin(pos.x.mul(2.1).add(t.mul(uSpeed).mul(0.7)))
				.mul(cos(pos.y.mul(1.8).add(t.mul(uSpeed).mul(0.5))))
				.mul(sin(pos.z.mul(2.5).add(t.mul(uSpeed).mul(0.9))));

			const scale = uBreathe.add(noise.mul(uIntensity));
			return pos.mul(scale);
		});

		// Generate environment map from the scene background
		// (gives reflections something to show)
		const pmrem = new THREE.PMREMGenerator(renderer);
		const envRT = pmrem.fromScene(scene);
		scene.environment = envRT.texture;

		const glassMat = new THREE.MeshPhysicalNodeMaterial();
		glassMat.positionNode = displacedPos();

		// Glass properties — visible on dark backgrounds
		glassMat.colorNode = uMoodColor.mul(0.08).add(vec3(0.01, 0.02, 0.06));
		glassMat.transmission = 0.95;
		glassMat.ior = 1.5;
		glassMat.thickness = 1.5;
		glassMat.roughness = 0.02;
		glassMat.metalness = 0.0;
		glassMat.dispersion = 0.5;
		glassMat.attenuationColor = new THREE.Color(0x6677cc);
		glassMat.attenuationDistance = 2.0;
		glassMat.clearcoat = 1.0;
		glassMat.clearcoatRoughness = 0.05;
		glassMat.specularIntensity = 2.0;
		glassMat.specularColor = new THREE.Color(0xccddff);
		glassMat.envMapIntensity = 2.5;
		glassMat.transparent = true;
		glassMat.side = THREE.DoubleSide;

		const creature = new THREE.Mesh(creatureGeo, glassMat);
		scene.add(creature);

		// ── Resize ──

		function resize() {
			if (!container) return;
			const w = container.clientWidth;
			const h = container.clientHeight;
			renderer.setSize(w, h);
			cam.aspect = w / h;
			cam.updateProjectionMatrix();
		}
		resize();
		const ro = new ResizeObserver(resize);
		ro.observe(container);

		// ── Animate ──

		let running = true;
		let prevTime = performance.now();
		let t = 0;
		let skip = false;

		function animate() {
			if (!running) return;
			requestAnimationFrame(animate);

			skip = !skip;
			if (skip) return;

			const now = performance.now();
			const delta = (now - prevTime) / 1000;
			prevTime = now;
			t += delta;

			const resolved = matchMood(moodRef);
			const energy = moodEnergies[resolved] ?? defaultEnergy;

			// Update uniforms
			uMoodColor.value.set(moodColors[resolved]);

			const amp = voiceRef ?? 0;
			const isSpeaking = amp > 0.01;
			uSpeed.value = thinkingRef ? 3.0 : isSpeaking ? energy.speed + amp * 3.0 : energy.speed;
			uIntensity.value = thinkingRef ? 0.25 : isSpeaking ? energy.intensity + amp * 0.3 : energy.intensity;
			uBreathe.value = 1.0
				+ Math.sin(t * (thinkingRef ? 2.5 : energy.breatheRate)) * (thinkingRef ? 0.06 : energy.breatheDepth)
				+ (isSpeaking ? amp * 0.12 : 0);

			// Light follows mood
			keyLight.color.set(moodColors[resolved]);
			keyLight.intensity = thinkingRef ? 4.0 : 3.0;
			fillLight.color.set(moodColors[resolved]);

			// Rotation
			creature.rotation.y += delta * (thinkingRef ? 0.4 : energy.rotSpeed);
			creature.rotation.x = Math.sin(t * 0.3) * 0.1;

			// Dispersion reacts to energy
			glassMat.dispersion = thinkingRef ? 0.6 : 0.4;

			renderer.render(scene, cam);
		}
		requestAnimationFrame(animate);

		cleanup = () => {
			running = false;
			ro.disconnect();
			creatureGeo.dispose();
			glassMat.dispose();
			renderer.dispose();
			renderer.domElement.remove();
		};
	});
</script>

<div class="glass-blob" bind:this={container}></div>

<style>
	.glass-blob {
		width: 100%;
		height: 100%;
	}

	.glass-blob :global(canvas) {
		display: block;
		width: 100% !important;
		height: 100% !important;
	}
</style>
