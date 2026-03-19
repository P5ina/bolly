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

		// Fallback background color — prevents black artifacts in transmission RT
		scene.background = new THREE.Color(0x010210);

		// ── Skybox sphere — wraps around the blob for real 3D reflections ──

		const skyGeo = new THREE.SphereGeometry(30, 64, 32);
		const skyMat = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide });

		const skyColor = Fn(() => {
			const st = uv();
			const t = time.mul(0.02);

			// Very dark base — almost black
			const abyss = vec3(0.005, 0.01, 0.04);
			const deep  = vec3(0.01, 0.02, 0.07);
			const purp  = vec3(0.03, 0.015, 0.08);

			const c = mix(abyss, deep, smoothstep(float(0.0), float(0.5), st.y)).toVar();
			c.assign(mix(c, purp, smoothstep(float(0.5), float(1.0), st.y)));

			const TAU = float(6.2832);
			const sx = st.x.mul(TAU);

			// Caustic lines — lower frequency for smoother reflections in glass
			const curve1 = float(0.50)
				.add(sin(sx.mul(1.0).add(t.mul(0.6))).mul(0.20))
				.add(cos(sx.mul(1.0).sub(t.mul(0.9)).add(1.5)).mul(0.12));
			const dist1 = abs(st.y.sub(curve1));
			const core1 = pow(float(2.718), dist1.mul(dist1).mul(-2000.0)).mul(0.7);
			const glow1 = pow(float(2.718), dist1.mul(dist1).mul(-40.0)).mul(0.12);
			const halo1 = pow(float(2.718), dist1.mul(dist1).mul(-8.0)).mul(0.04);

			const curve2 = float(0.35)
				.add(sin(sx.mul(1.0).add(t.mul(0.4)).add(2.0)).mul(0.17))
				.add(cos(sx.mul(2.0).sub(t.mul(0.7))).mul(0.09));
			const dist2 = abs(st.y.sub(curve2));
			const core2 = pow(float(2.718), dist2.mul(dist2).mul(-2000.0)).mul(0.5);
			const glow2 = pow(float(2.718), dist2.mul(dist2).mul(-40.0)).mul(0.08);
			const halo2 = pow(float(2.718), dist2.mul(dist2).mul(-8.0)).mul(0.03);

			const curve3 = float(0.68)
				.add(sin(sx.mul(1.0).add(t.mul(0.8)).sub(1.0)).mul(0.15))
				.add(cos(sx.mul(1.0).add(t.mul(0.5)).add(4.0)).mul(0.08));
			const dist3 = abs(st.y.sub(curve3));
			const core3 = pow(float(2.718), dist3.mul(dist3).mul(-2000.0)).mul(0.35);
			const glow3 = pow(float(2.718), dist3.mul(dist3).mul(-40.0)).mul(0.06);

			const curve4 = float(0.22)
				.add(sin(sx.mul(2.0).add(t.mul(0.3)).add(3.5)).mul(0.12))
				.add(cos(sx.mul(1.0).sub(t.mul(0.6)).add(1.0)).mul(0.07));
			const dist4 = abs(st.y.sub(curve4));
			const core4 = pow(float(2.718), dist4.mul(dist4).mul(-2000.0)).mul(0.25);
			const glow4 = pow(float(2.718), dist4.mul(dist4).mul(-40.0)).mul(0.04);

			// Bright core: white-blue, glow: tinted, halo: subtle color wash
			const coreColor = vec3(0.55, 0.60, 0.85);
			const glowColor = vec3(0.20, 0.25, 0.50);
			const haloColor = vec3(0.08, 0.10, 0.25);

			c.addAssign(coreColor.mul(core1.add(core2).add(core3).add(core4)));
			c.addAssign(glowColor.mul(glow1.add(glow2).add(glow3).add(glow4)));
			c.addAssign(haloColor.mul(halo1.add(halo2)));

			return vec4(c, float(1.0));
		});

		skyMat.colorNode = skyColor();
		const skybox = new THREE.Mesh(skyGeo, skyMat);
		skybox.visible = false;
		scene.add(skybox);

		// ── Flat background (what you actually see on screen) ──

		const flatBg = Fn(() => {
			const st = screenUV;
			const t = time.mul(0.02);

			const abyss = vec3(0.005, 0.01, 0.04);
			const deep  = vec3(0.01, 0.02, 0.07);
			const purp  = vec3(0.03, 0.015, 0.08);

			const c = mix(abyss, deep, smoothstep(float(0.0), float(0.5), st.y)).toVar();
			c.assign(mix(c, purp, smoothstep(float(0.5), float(1.0), st.y)));

			// Caustic lines — same curves but using screen UV (not stretched)
			const curve1 = float(0.50)
				.add(sin(st.x.mul(2.5).add(t.mul(0.6))).mul(0.20))
				.add(cos(st.x.mul(4.5).sub(t.mul(0.9)).add(1.5)).mul(0.12))
				.add(sin(st.x.mul(7.0).add(t.mul(1.2)).add(3.0)).mul(0.07));
			const dist1 = abs(st.y.sub(curve1));
			const core1 = pow(float(2.718), dist1.mul(dist1).mul(-2000.0)).mul(0.7);
			const glow1 = pow(float(2.718), dist1.mul(dist1).mul(-40.0)).mul(0.12);
			const halo1 = pow(float(2.718), dist1.mul(dist1).mul(-8.0)).mul(0.04);

			const curve2 = float(0.32)
				.add(sin(st.x.mul(3.0).add(t.mul(0.4)).add(2.0)).mul(0.17))
				.add(cos(st.x.mul(5.5).sub(t.mul(0.7))).mul(0.09));
			const dist2 = abs(st.y.sub(curve2));
			const core2 = pow(float(2.718), dist2.mul(dist2).mul(-2000.0)).mul(0.5);
			const glow2 = pow(float(2.718), dist2.mul(dist2).mul(-40.0)).mul(0.08);
			const halo2 = pow(float(2.718), dist2.mul(dist2).mul(-8.0)).mul(0.03);

			const curve3 = float(0.70)
				.add(sin(st.x.mul(2.0).add(t.mul(0.8)).sub(1.0)).mul(0.14))
				.add(cos(st.x.mul(3.5).add(t.mul(0.5)).add(4.0)).mul(0.08));
			const dist3 = abs(st.y.sub(curve3));
			const core3 = pow(float(2.718), dist3.mul(dist3).mul(-2000.0)).mul(0.35);
			const glow3 = pow(float(2.718), dist3.mul(dist3).mul(-40.0)).mul(0.06);

			const curve4 = float(0.18)
				.add(sin(st.x.mul(4.0).add(t.mul(0.3)).add(3.5)).mul(0.11))
				.add(cos(st.x.mul(6.0).sub(t.mul(0.6)).add(1.0)).mul(0.06));
			const dist4 = abs(st.y.sub(curve4));
			const core4 = pow(float(2.718), dist4.mul(dist4).mul(-2000.0)).mul(0.25);
			const glow4 = pow(float(2.718), dist4.mul(dist4).mul(-40.0)).mul(0.04);

			const coreColor = vec3(0.55, 0.60, 0.85);
			const glowColor = vec3(0.20, 0.25, 0.50);
			const haloColor = vec3(0.08, 0.10, 0.25);

			c.addAssign(coreColor.mul(core1.add(core2).add(core3).add(core4)));
			c.addAssign(glowColor.mul(glow1.add(glow2).add(glow3).add(glow4)));
			c.addAssign(haloColor.mul(halo1.add(halo2)));

			return c;
		});
		scene.backgroundNode = flatBg();

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
		glassMat.color = new THREE.Color(0xffffff);
		glassMat.transmission = 0.99;
		glassMat.ior = 1.2;
		glassMat.thickness = 0.5;
		glassMat.roughness = 0.05;
		glassMat.metalness = 0.0;
		glassMat.dispersion = 0.15;
		glassMat.attenuationColor = new THREE.Color(0xffffff);
		glassMat.attenuationDistance = Infinity;
		glassMat.clearcoat = 0.1;
		glassMat.clearcoatRoughness = 0.0;
		glassMat.specularIntensity = 1.0;
		glassMat.specularColor = new THREE.Color(0xffffff);
		glassMat.envMapIntensity = 25;
		glassMat.transparent = true;
		glassMat.side = THREE.FrontSide;

		const creature = new THREE.Mesh(creatureGeo, glassMat);
		creature.scale.setScalar(1.2);
		creature.position.set(1.8, -0.1, 0);
		scene.add(creature);

		// Generate envmap from scene background for glass reflections
		const pmrem = new THREE.PMREMGenerator(renderer);
		let envNeedsUpdate = true;

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

			if (envNeedsUpdate) {
				creature.visible = false;
				skybox.visible = true;
				const envRT = pmrem.fromScene(scene);
				scene.environment = envRT.texture;
				skybox.visible = false;
				creature.visible = true;
				envNeedsUpdate = false;
			}

			renderer.render(scene, cam);
		}
		requestAnimationFrame(animate);

		return () => {
			running = false;
			ro.disconnect();
			creatureGeo.dispose();
			glassMat.dispose();
			pmrem.dispose();
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
