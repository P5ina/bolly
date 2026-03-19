<script lang="ts">
	import { onMount } from "svelte";

	let {
		mood = "calm",
		thinking = false,
		voiceAmplitude = 0,
	}: {
		mood?: string;
		thinking?: boolean;
		voiceAmplitude?: number;
	} = $props();

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

	onMount(async () => {
		if (!container) return;

		const THREE = await import("three/webgpu");
		const TSL = await import("three/tsl");
		const {
			Fn, uniform, float, vec3, vec4,
			uv, positionLocal, time, screenUV,
			sin, cos, abs, mix, smoothstep, pow,
		} = TSL;

		const renderer = new THREE.WebGPURenderer({ antialias: true });
		await renderer.init();
		renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
		renderer.toneMapping = THREE.NoToneMapping;
		container.appendChild(renderer.domElement);

		const scene = new THREE.Scene();

		const cam = new THREE.PerspectiveCamera(50, 1, 0.1, 100);
		cam.position.set(0, 0, 5);
		cam.lookAt(0, 0, 0);

		// ── Background (TSL wave shader as scene.backgroundNode) ──

		const bgNode = Fn(() => {
			const st = screenUV;
			const t = time.mul(0.02);

			// Full palette — matching the original GLSL shader
			const deepBlue  = vec3(0.015, 0.035, 0.14);
			const midBlue   = vec3(0.04, 0.07, 0.24);
			const purple    = vec3(0.10, 0.05, 0.26);
			const indigo    = vec3(0.07, 0.04, 0.22);
			const lightBlue = vec3(0.08, 0.12, 0.28);
			const mist      = vec3(0.12, 0.16, 0.30);

			// 3-stop vertical gradient
			const c = mix(deepBlue, midBlue, smoothstep(float(0.0), float(0.4), st.y)).toVar();
			c.assign(mix(c, indigo, smoothstep(float(0.3), float(0.7), st.y)));
			c.assign(mix(c, purple, smoothstep(float(0.6), float(1.0), st.y)));

			// Terrain waves (5 layered sine/cos)
			const w = sin(st.x.mul(2.2).add(t.mul(0.8)).add(st.y.mul(0.5))).mul(0.35)
				.add(sin(st.x.mul(1.1).sub(t.mul(0.5)).add(3.0)).mul(0.25))
				.add(sin(st.x.mul(3.5).add(t.mul(1.2)).add(st.y.mul(1.5)).add(1.0)).mul(0.15))
				.add(sin(st.x.mul(0.8).add(t.mul(0.3)).sub(2.0)).mul(0.2))
				.add(cos(st.x.mul(1.8).add(st.y.mul(2.0)).add(t.mul(0.7))).mul(0.12));

			// Lower mist/light area
			c.assign(mix(c, lightBlue, smoothstep(float(-0.1), float(0.3), w.sub(st.y.mul(0.8)).add(0.2)).mul(0.3)));

			// Upper wave crest
			c.assign(mix(c, mist, smoothstep(float(-0.05), float(0.15), w.sub(st.y.mul(1.2)).add(0.5)).mul(0.2)));

			// Deep shadow in wave valleys
			c.assign(mix(c, deepBlue.mul(0.6), smoothstep(float(0.2), float(-0.1), w.sub(st.y.mul(0.6)).add(0.1)).mul(0.4)));

			// Secondary wave layer (slower, larger scale)
			const w2 = sin(st.x.mul(0.6).mul(2.2).add(5.0).add(t.mul(0.8)).add(st.y.mul(0.6).mul(0.5))).mul(0.35)
				.add(sin(st.x.mul(0.6).mul(1.1).add(5.0).sub(t.mul(0.5)).add(3.0)).mul(0.25))
				.add(cos(st.x.mul(0.6).mul(1.8).add(5.0).add(st.y.mul(0.6).mul(2.0)).add(t.mul(0.7))).mul(0.12));
			c.assign(mix(c, midBlue.mul(1.1), smoothstep(float(-0.05), float(0.2), w2.sub(st.y.mul(0.9)).add(0.35)).mul(0.2)));

			// Specular caustic line
			const curve = float(0.55)
				.add(sin(st.x.mul(2.5).add(t.mul(0.6))).mul(0.25))
				.add(cos(st.x.mul(4.0).sub(t.mul(0.9)).add(1.5)).mul(0.1))
				.add(sin(st.x.mul(6.0).add(t.mul(1.2)).add(3.0)).mul(0.08));
			const dist = abs(st.y.sub(curve));
			// exp(-dist²*1200) for thin line + exp(-dist²*60) for glow
			const line = pow(float(2.718), dist.mul(dist).mul(-1200.0)).mul(0.35);
			const glow = pow(float(2.718), dist.mul(dist).mul(-60.0)).mul(0.08);
			c.addAssign(vec3(0.35, 0.40, 0.55).mul(line.add(glow)));

			// Secondary faint caustic
			const curve2 = float(0.4)
				.add(sin(st.x.mul(2.0).add(t.mul(0.3)).add(2.0)).mul(0.2))
				.add(cos(st.x.mul(3.5).sub(t.mul(0.5))).mul(0.08));
			const dist2 = abs(st.y.sub(curve2));
			c.addAssign(vec3(0.35, 0.40, 0.55).mul(
				pow(float(2.718), dist2.mul(dist2).mul(-1200.0)).mul(0.1)
			));

			// Vignette
			const vigX = st.x.sub(0.5).mul(1.2);
			const vigY = st.y.sub(0.5).mul(1.4);
			const vig = smoothstep(float(-0.1), float(0.6),
				float(1.0).sub(vigX.mul(vigX).add(vigY.mul(vigY)).pow(0.5))
			);
			c.mulAssign(float(0.7).add(vig.mul(0.3)));

			return c;
		});
		scene.backgroundNode = bgNode();

		// ── Lighting ──

		scene.add(new THREE.AmbientLight(0x334477, 0.3));

		const keyLight = new THREE.DirectionalLight(0xffffff, 2.0);
		keyLight.position.set(3, 2, 4);
		scene.add(keyLight);

		const rimLight = new THREE.PointLight(0x8899cc, 1.0);
		rimLight.position.set(-3, 1.5, -2);
		scene.add(rimLight);

		const fillLight = new THREE.PointLight(0x6677aa, 0.4);
		fillLight.position.set(0, -3, 1);
		scene.add(fillLight);

		// ── Glass blob ──

		const creatureGeo = new THREE.IcosahedronGeometry(1, 6);

		const uSpeed = uniform(0.8);
		const uIntensity = uniform(0.12);
		const uBreathe = uniform(1.0);
		const uMoodColor = uniform(new THREE.Color(moodColors[matchMood(mood)]));

		const displacedPos = Fn(() => {
			const pos = positionLocal.toVar();
			const t = time;
			const noise = sin(pos.x.mul(2.1).add(t.mul(uSpeed).mul(0.7)))
				.mul(cos(pos.y.mul(1.8).add(t.mul(uSpeed).mul(0.5))))
				.mul(sin(pos.z.mul(2.5).add(t.mul(uSpeed).mul(0.9))));
			return pos.mul(uBreathe.add(noise.mul(uIntensity)));
		});

		const glassMat = new THREE.MeshPhysicalNodeMaterial();
		glassMat.positionNode = displacedPos();
		glassMat.colorNode = uMoodColor.mul(0.05).add(vec3(0.01, 0.02, 0.04));
		glassMat.transmission = 0.96;
		glassMat.ior = 1.45;
		glassMat.thickness = 1.2;
		glassMat.roughness = 0.02;
		glassMat.dispersion = 0.4;
		glassMat.attenuationColor = new THREE.Color(0x7788cc);
		glassMat.attenuationDistance = 2.5;
		glassMat.clearcoat = 0.6;
		glassMat.clearcoatRoughness = 0.05;
		glassMat.specularIntensity = 1.5;
		glassMat.specularColor = new THREE.Color(0xccddff);
		glassMat.envMapIntensity = 0.5;
		glassMat.transparent = true;
		glassMat.side = THREE.DoubleSide;

		const creature = new THREE.Mesh(creatureGeo, glassMat);
		creature.scale.setScalar(0.7);
		creature.position.set(1.8, -0.1, 0); // right side, slightly below center
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

			uMoodColor.value.set(moodColors[resolved]);

			const amp = voiceRef ?? 0;
			const isSpeaking = amp > 0.01;
			uSpeed.value = thinkingRef ? 3.0 : isSpeaking ? energy.speed + amp * 3.0 : energy.speed;
			uIntensity.value = thinkingRef ? 0.25 : isSpeaking ? energy.intensity + amp * 0.3 : energy.intensity;
			uBreathe.value = 1.0
				+ Math.sin(t * (thinkingRef ? 2.5 : energy.breatheRate)) * (thinkingRef ? 0.06 : energy.breatheDepth)
				+ (isSpeaking ? amp * 0.12 : 0);

			keyLight.color.set(moodColors[resolved]);
			keyLight.intensity = thinkingRef ? 2.5 : 2.0;
			fillLight.color.set(moodColors[resolved]);

			creature.rotation.y += delta * (thinkingRef ? 0.4 : energy.rotSpeed);
			creature.rotation.x = Math.sin(t * 0.3) * 0.1;

			glassMat.dispersion = thinkingRef ? 0.6 : 0.4;

			renderer.render(scene, cam);
		}
		requestAnimationFrame(animate);

		return () => {
			running = false;
			ro.disconnect();
			creatureGeo.dispose();
			glassMat.dispose();
			renderer.dispose();
			renderer.domElement.remove();
		};
	});
</script>

<div class="scene-root" bind:this={container}></div>

<style>
	.scene-root {
		position: absolute;
		inset: 0;
		z-index: 0;
		pointer-events: none;
	}

	.scene-root :global(canvas) {
		display: block;
		width: 100% !important;
		height: 100% !important;
	}
</style>
