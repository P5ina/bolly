<script lang="ts">
	import { onMount } from "svelte";
	import * as THREE from "three";

	let { thinking = false, mood = "calm", voiceAmplitude = 0 }: { thinking?: boolean; mood?: string; voiceAmplitude?: number } = $props();

	let container: HTMLDivElement | undefined = $state();

	// ── Mood data ──────────────────────────────────────────────────────

	const moodColors: Record<string, string> = {
		calm: "#8ab4f8", curious: "#a8d8ea", excited: "#f8c471", warm: "#f0b27a",
		happy: "#f7dc6f", joyful: "#f9e154", reflective: "#bb8fce", contemplative: "#a993c7",
		melancholy: "#7f8c9a", sad: "#6b7b8d", worried: "#85929e", anxious: "#95a0ab",
		playful: "#82e0aa", mischievous: "#58d68d", focused: "#76d7c4", tired: "#a0937d",
		peaceful: "#aed6f1", loving: "#f1948a", tender: "#f5b7b1", creative: "#d2b4de",
		energetic: "#fad7a0",
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
		if (moodColors[m]) return m;
		for (const k of Object.keys(moodColors).sort((a, b) => b.length - a.length)) {
			if (m.includes(k)) return k;
		}
		return "calm";
	}

	// ── Glass shaders ──────────────────────────────────────────────────

	const glassVert = /* glsl */ `
		varying vec3 vNormal;
		varying vec3 vViewPos;
		varying vec3 vWorldNormal;
		varying float vDisplacement;

		void main() {
			vNormal = normalize(normalMatrix * normal);
			vec4 mvPos = modelViewMatrix * vec4(position, 1.0);
			vViewPos = mvPos.xyz;
			vWorldNormal = normalize((modelMatrix * vec4(normal, 0.0)).xyz);
			vDisplacement = length(position) - 1.0; // deviation from unit sphere
			gl_Position = projectionMatrix * mvPos;
		}
	`;

	const glassFrag = /* glsl */ `
		precision highp float;

		uniform vec3 uColor;
		uniform float uGlow;
		uniform float uTime;

		varying vec3 vNormal;
		varying vec3 vViewPos;
		varying vec3 vWorldNormal;
		varying float vDisplacement;

		void main() {
			vec3 N = normalize(vNormal);
			vec3 V = normalize(-vViewPos);

			// --- Fresnel: glass edges glow brighter ---
			float fresnel = pow(1.0 - max(dot(N, V), 0.0), 3.5);

			// --- Key light specular (sharp highlight) ---
			vec3 lightDir = normalize(vec3(3.0, 2.0, 4.0));
			vec3 H = normalize(lightDir + V);
			float spec1 = pow(max(dot(N, H), 0.0), 80.0);

			// --- Rim light (back edge definition) ---
			vec3 rimDir = normalize(vec3(-2.5, 1.5, -3.0));
			float rim = pow(1.0 - max(dot(N, V), 0.0), 2.5)
			          * (dot(N, rimDir) * 0.4 + 0.6);

			// --- Secondary specular (smaller, offset) ---
			vec3 lightDir2 = normalize(vec3(-1.0, 3.0, 2.0));
			vec3 H2 = normalize(lightDir2 + V);
			float spec2 = pow(max(dot(N, H2), 0.0), 120.0) * 0.5;

			// --- Environment fake reflection (normal-based gradient) ---
			float envUp = vWorldNormal.y * 0.5 + 0.5;
			vec3 envColor = mix(
				vec3(0.03, 0.06, 0.18), // dark blue below
				vec3(0.10, 0.08, 0.22), // purple above
				envUp
			);

			// --- Caustic shimmer on surface ---
			float caustic = sin(vWorldNormal.x * 8.0 + uTime * 1.5)
			              * cos(vWorldNormal.y * 6.0 + uTime * 1.2)
			              * sin(vWorldNormal.z * 7.0 + uTime * 0.9);
			caustic = max(caustic, 0.0) * 0.08;

			// --- Compose glass ---
			vec3 glassBase = uColor * 0.08 + envColor * 0.3;
			vec3 edgeGlow = vec3(0.5, 0.6, 0.85); // cool specular

			vec3 color = glassBase;
			color += edgeGlow * fresnel * 0.45;
			color += vec3(0.9, 0.92, 1.0) * spec1 * 0.7;
			color += vec3(0.8, 0.85, 1.0) * spec2;
			color += uColor * rim * 0.2;
			color += uColor * caustic;

			// Displacement-based internal glow (brighter where deformed)
			color += uColor * abs(vDisplacement) * 0.4;

			// Thinking glow
			color += uColor * uGlow * 0.12;
			color += edgeGlow * uGlow * fresnel * 0.2;

			// Alpha: transparent center, opaque edges
			float alpha = 0.08 + fresnel * 0.55 + spec1 * 0.4 + spec2 * 0.2 + caustic;
			alpha = clamp(alpha, 0.0, 0.85);

			gl_FragColor = vec4(color, alpha);
		}
	`;

	// ── Reactive refs ──────────────────────────────────────────────────

	let moodRef = mood;
	let thinkingRef = thinking;
	let voiceRef = voiceAmplitude;

	$effect(() => { moodRef = mood; });
	$effect(() => { thinkingRef = thinking; });
	$effect(() => { voiceRef = voiceAmplitude; });

	// ── Mount ──────────────────────────────────────────────────────────

	onMount(() => {
		if (!container) return;

		const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
		renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
		renderer.setClearColor(0x000000, 0);
		container.appendChild(renderer.domElement);

		const scene = new THREE.Scene();
		const cam = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
		cam.position.set(0, 0, 3.0);

		// Creature mesh — glass blob
		const creatureGeo = new THREE.IcosahedronGeometry(1, 6);
		const basePositions = new Float32Array(creatureGeo.attributes.position.array);

		const glassUniforms = {
			uColor: { value: new THREE.Color(moodColors[matchMood(mood)]) },
			uGlow: { value: 0 },
			uTime: { value: 0 },
		};

		const glassMat = new THREE.ShaderMaterial({
			uniforms: glassUniforms,
			vertexShader: glassVert,
			fragmentShader: glassFrag,
			transparent: true,
			depthWrite: false,
			side: THREE.DoubleSide,
		});

		const creature = new THREE.Mesh(creatureGeo, glassMat);
		scene.add(creature);

		// Inner core glow — small bright sphere inside
		const coreGeo = new THREE.IcosahedronGeometry(0.35, 4);
		const coreMat = new THREE.ShaderMaterial({
			uniforms: glassUniforms,
			vertexShader: glassVert,
			fragmentShader: glassFrag,
			transparent: true,
			depthWrite: false,
		});
		const core = new THREE.Mesh(coreGeo, coreMat);
		scene.add(core);

		// Resize
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

		// Animation
		let running = true;
		let skip = false;
		const clock = new THREE.Clock();
		let t = 0;
		let glowSmooth = 0;

		function animate() {
			if (!running) return;
			requestAnimationFrame(animate);

			skip = !skip;
			if (skip) return;

			const delta = clock.getDelta();
			t += delta;

			const resolved = matchMood(moodRef);
			const energy = moodEnergies[resolved] ?? defaultEnergy;
			const col = new THREE.Color(moodColors[resolved]);

			// Update uniforms
			glassUniforms.uColor.value.copy(col);
			glassUniforms.uTime.value = t;
			const targetGlow = thinkingRef ? 1.0 : 0.0;
			glowSmooth += (targetGlow - glowSmooth) * Math.min(delta * 3.0, 0.15);
			glassUniforms.uGlow.value = glowSmooth;

			// Vertex displacement
			const amp = voiceRef ?? 0;
			const isSpeaking = amp > 0.01;
			const speed = thinkingRef ? 3.0 : isSpeaking ? energy.speed + amp * 3.0 : energy.speed;
			const intensity = thinkingRef ? 0.25 : isSpeaking ? energy.intensity + amp * 0.3 : energy.intensity;
			const breathe = 1.0
				+ Math.sin(t * (thinkingRef ? 2.5 : energy.breatheRate)) * (thinkingRef ? 0.06 : energy.breatheDepth)
				+ (isSpeaking ? amp * 0.12 : 0);

			const pos = creatureGeo.attributes.position;
			for (let i = 0; i < pos.count; i++) {
				const bx = basePositions[i * 3];
				const by = basePositions[i * 3 + 1];
				const bz = basePositions[i * 3 + 2];
				const noise =
					Math.sin(bx * 2.1 + t * speed * 0.7) *
					Math.cos(by * 1.8 + t * speed * 0.5) *
					Math.sin(bz * 2.5 + t * speed * 0.9);
				const scale = breathe + noise * intensity;
				pos.setXYZ(i, bx * scale, by * scale, bz * scale);
			}
			pos.needsUpdate = true;
			creatureGeo.computeVertexNormals();

			creature.rotation.y += delta * (thinkingRef ? 0.4 : energy.rotSpeed);
			creature.rotation.x = Math.sin(t * 0.3) * 0.1;

			// Render
			renderer.render(scene, cam);
		}
		requestAnimationFrame(animate);

		return () => {
			running = false;
			ro.disconnect();
			creatureGeo.dispose();
			glassMat.dispose();
			coreGeo.dispose();
			coreMat.dispose();
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
