<script lang="ts">
	import { onMount } from "svelte";

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

	// ── Background shader (renders to RT for refraction) ───────────────

	const bgFrag = /* glsl */ `
		precision highp float;
		uniform float uTime;
		varying vec2 vUv;

		float terrain(vec2 uv, float t) {
			float w = 0.0;
			w += sin(uv.x * 2.2 + t * 0.8 + uv.y * 0.5) * 0.35;
			w += sin(uv.x * 1.1 - t * 0.5 + 3.0) * 0.25;
			w += sin(uv.x * 3.5 + t * 1.2 + uv.y * 1.5 + 1.0) * 0.15;
			w += sin(uv.x * 0.8 + t * 0.3 - 2.0) * 0.2;
			w += cos(uv.x * 1.8 + uv.y * 2.0 + t * 0.7) * 0.12;
			return w;
		}

		void main() {
			vec2 uv = vUv;
			float t = uTime * 0.015;
			vec3 deep = vec3(0.03, 0.06, 0.20);
			vec3 mid  = vec3(0.07, 0.12, 0.35);
			vec3 purp = vec3(0.15, 0.08, 0.38);
			vec3 lite = vec3(0.12, 0.18, 0.40);

			vec3 c = mix(deep, mid, smoothstep(0.0, 0.4, uv.y));
			c = mix(c, purp, smoothstep(0.5, 1.0, uv.y));

			float w = terrain(uv, t);
			c = mix(c, lite, smoothstep(-0.1, 0.3, w - uv.y * 0.8 + 0.2) * 0.3);
			c = mix(c, deep * 0.6, smoothstep(0.2, -0.1, w - uv.y * 0.6 + 0.1) * 0.35);

			// Caustic
			float curve = 0.55 + 0.25 * sin(uv.x * 2.5 + t * 0.4) + 0.1 * cos(uv.x * 4.0 - t * 0.6);
			float dist = abs(uv.y - curve);
			c += vec3(0.2, 0.22, 0.3) * exp(-dist * dist * 800.0) * 0.25;

			gl_FragColor = vec4(c, 1.0);
		}
	`;

	// ── Glass shader with chromatic dispersion ─────────────────────────

	const glassVert = /* glsl */ `
		varying vec3 vNormal;
		varying vec3 vViewPos;
		varying vec3 vWorldNormal;

		void main() {
			vNormal = normalize(normalMatrix * normal);
			vec4 mvPos = modelViewMatrix * vec4(position, 1.0);
			vViewPos = mvPos.xyz;
			vWorldNormal = normalize((modelMatrix * vec4(normal, 0.0)).xyz);
			gl_Position = projectionMatrix * mvPos;
		}
	`;

	const glassFrag = /* glsl */ `
		precision highp float;

		uniform sampler2D uBackground;
		uniform vec3 uColor;
		uniform float uGlow;
		uniform float uTime;
		uniform vec2 uResolution;

		varying vec3 vNormal;
		varying vec3 vViewPos;
		varying vec3 vWorldNormal;

		void main() {
			vec3 N = normalize(vNormal);
			vec3 V = normalize(-vViewPos);
			vec2 screenUV = gl_FragCoord.xy / uResolution;

			// --- Chromatic dispersion: per-channel refraction ---
			// Different IOR per wavelength (red < green < blue)
			float iorR = 1.0 / 1.12;
			float iorG = 1.0 / 1.16;
			float iorB = 1.0 / 1.22;

			vec3 refR = refract(-V, N, iorR);
			vec3 refG = refract(-V, N, iorG);
			vec3 refB = refract(-V, N, iorB);

			// Refraction strength scales with angle
			float edgeFactor = 1.0 - abs(dot(N, V));
			float strength = 0.08 + edgeFactor * 0.12;

			float R = texture2D(uBackground, screenUV + refR.xy * strength).r;
			float G = texture2D(uBackground, screenUV + refG.xy * strength).g;
			float B = texture2D(uBackground, screenUV + refB.xy * strength).b;

			vec3 refracted = vec3(R, G, B);

			// --- Fresnel ---
			float fresnel = pow(1.0 - max(dot(N, V), 0.0), 4.0);

			// --- Specular highlights ---
			vec3 L1 = normalize(vec3(3.0, 2.0, 4.0));
			vec3 H1 = normalize(L1 + V);
			float spec1 = pow(max(dot(N, H1), 0.0), 160.0);

			vec3 L2 = normalize(vec3(-1.5, 3.0, 1.0));
			vec3 H2 = normalize(L2 + V);
			float spec2 = pow(max(dot(N, H2), 0.0), 200.0) * 0.4;

			// --- Surface caustic shimmer ---
			float caustic = max(
				sin(vWorldNormal.x * 10.0 + uTime * 2.0) *
				cos(vWorldNormal.y * 8.0 + uTime * 1.5) *
				sin(vWorldNormal.z * 9.0 + uTime * 1.2),
				0.0
			) * 0.04;

			// --- Compose ---
			vec3 color = refracted;

			// Edge tint (subtle mood color at rim)
			color = mix(color, color + uColor * 0.15, fresnel);

			// Specular
			color += vec3(0.9, 0.93, 1.0) * spec1 * 0.6;
			color += vec3(0.8, 0.85, 1.0) * spec2;

			// Surface shimmer
			color += uColor * caustic;

			// Thinking glow
			color += uColor * uGlow * fresnel * 0.15;

			// Alpha: translucent with visible refraction, bright edges
			float alpha = 0.15 + fresnel * 0.5 + spec1 * 0.6 + spec2 * 0.3 + caustic;
			alpha = clamp(alpha, 0.0, 0.95);

			gl_FragColor = vec4(color, alpha);
		}
	`;

	const quadVert = /* glsl */ `
		varying vec2 vUv;
		void main() {
			vUv = uv;
			gl_Position = vec4(position, 1.0);
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

	onMount(async () => {
		if (!container) return;

		const THREE = await import("three");

		const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
		renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
		renderer.setClearColor(0x000000, 0);
		renderer.autoClear = false;
		container.appendChild(renderer.domElement);

		// ── Background pass (render to RT for refraction sampling) ──

		const bgScene = new THREE.Scene();
		const bgCam = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
		const bgUniforms = { uTime: { value: 0 } };
		const bgMat = new THREE.ShaderMaterial({
			uniforms: bgUniforms,
			vertexShader: quadVert,
			fragmentShader: bgFrag,
		});
		bgScene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), bgMat));

		const bgRT = new THREE.WebGLRenderTarget(512, 512, {
			minFilter: THREE.LinearFilter,
			magFilter: THREE.LinearFilter,
		});

		// ── Glass blob scene ──

		const glassScene = new THREE.Scene();
		const cam = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
		cam.position.set(0, 0, 3.0);

		const creatureGeo = new THREE.IcosahedronGeometry(1, 6);
		const basePositions = new Float32Array(creatureGeo.attributes.position.array);

		const glassUniforms = {
			uBackground: { value: bgRT.texture },
			uColor: { value: new THREE.Color(moodColors[matchMood(mood)]) },
			uGlow: { value: 0 },
			uTime: { value: 0 },
			uResolution: { value: new THREE.Vector2() },
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
		glassScene.add(creature);

		// ── Resize ──

		function resize() {
			if (!container) return;
			const w = container.clientWidth;
			const h = container.clientHeight;
			renderer.setSize(w, h);
			cam.aspect = w / h;
			cam.updateProjectionMatrix();
			const pr = renderer.getPixelRatio();
			glassUniforms.uResolution.value.set(w * pr, h * pr);
		}
		resize();
		const ro = new ResizeObserver(resize);
		ro.observe(container);

		// ── Animation ──

		let running = true;
		let skip = false;
		let prevTime = performance.now();
		let t = 0;
		let glowSmooth = 0;

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
			const col = new THREE.Color(moodColors[resolved]);

			// Update uniforms
			bgUniforms.uTime.value = t;
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

			// Smooth normals: displacement is radial, so normal = normalized position
			const nrm = creatureGeo.attributes.normal;
			for (let i = 0; i < pos.count; i++) {
				const x = pos.getX(i);
				const y = pos.getY(i);
				const z = pos.getZ(i);
				const len = Math.sqrt(x * x + y * y + z * z) || 1;
				nrm.setXYZ(i, x / len, y / len, z / len);
			}
			nrm.needsUpdate = true;

			creature.rotation.y += delta * (thinkingRef ? 0.4 : energy.rotSpeed);
			creature.rotation.x = Math.sin(t * 0.3) * 0.1;

			// Pass 1: background → render target (for refraction sampling)
			renderer.setRenderTarget(bgRT);
			renderer.clear();
			renderer.render(bgScene, bgCam);

			// Pass 2: background → screen (visible behind the blob)
			renderer.setRenderTarget(null);
			renderer.clear();
			renderer.render(bgScene, bgCam);

			// Pass 3: glass blob → screen on top (samples bgRT for refraction)
			renderer.render(glassScene, cam);
		}
		requestAnimationFrame(animate);

		return () => {
			running = false;
			ro.disconnect();
			creatureGeo.dispose();
			glassMat.dispose();
			bgMat.dispose();
			bgRT.dispose();
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
