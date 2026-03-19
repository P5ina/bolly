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

	// ── Constants ──────────────────────────────────────────────────────

	const COLS = 48;
	const ROWS = 28;
	const RAMP = " .:;+xX#%@";

	// ── Font atlas ─────────────────────────────────────────────────────

	function createFontAtlas(): THREE.CanvasTexture {
		const size = 48;
		const canvas = document.createElement("canvas");
		canvas.width = RAMP.length * size;
		canvas.height = size;
		const ctx = canvas.getContext("2d")!;
		ctx.fillStyle = "#000";
		ctx.fillRect(0, 0, canvas.width, canvas.height);
		ctx.fillStyle = "#fff";
		ctx.font = `bold ${size * 0.75}px "JetBrains Mono", "Courier New", monospace`;
		ctx.textBaseline = "middle";
		ctx.textAlign = "center";
		for (let i = 0; i < RAMP.length; i++) {
			if (RAMP[i] !== " ") {
				ctx.fillText(RAMP[i], i * size + size / 2, size / 2 + 1);
			}
		}
		const tex = new THREE.CanvasTexture(canvas);
		tex.minFilter = THREE.LinearFilter;
		tex.magFilter = THREE.LinearFilter;
		return tex;
	}

	// ── ASCII fragment shader ──────────────────────────────────────────

	const asciiVert = /* glsl */ `
		varying vec2 vUv;
		void main() {
			vUv = uv;
			gl_Position = vec4(position, 1.0);
		}
	`;

	const asciiFrag = /* glsl */ `
		precision highp float;

		uniform sampler2D tScene;
		uniform sampler2D tFont;
		uniform float uCols;
		uniform float uRows;
		uniform float uRampLen;
		uniform vec3 uColor;
		uniform float uGlow;
		uniform vec2 uResolution;

		varying vec2 vUv;

		void main() {
			vec2 cellCount = vec2(uCols, uRows);
			vec2 cellSize = uResolution / cellCount;
			vec2 cell = floor(gl_FragCoord.xy / cellSize);

			// Sample creature at cell center (no Y flip — GL coords match RT coords)
			vec2 sceneUV = (cell + 0.5) / cellCount;
			vec4 sc = texture2D(tScene, sceneUV);

			float lum = dot(sc.rgb, vec3(0.299, 0.587, 0.114));

			// Empty cell — no creature here
			if (lum < 0.01) discard;

			// Stretch contrast: remap the actual luminance range to full 0–1
			// The creature's lit areas typically sit in 0.01–0.4 range
			lum = smoothstep(0.01, 0.32, lum);

			// Map to ramp index
			float fi = lum * (uRampLen - 1.0);
			int idx = int(clamp(fi, 0.0, uRampLen - 1.0));

			// Space character → discard
			if (idx == 0) discard;

			// UV within cell — flip Y for font atlas (canvas top-down vs GL bottom-up)
			vec2 cellUV = fract(gl_FragCoord.xy / cellSize);
			cellUV.y = 1.0 - cellUV.y;

			// Sample font atlas
			float atlasX = (float(idx) + cellUV.x) / uRampLen;
			float ch = texture2D(tFont, vec2(atlasX, cellUV.y)).r;

			if (ch < 0.08) discard;

			// Color: mood tint + luminance intensity
			vec3 color = uColor * ch * (0.6 + lum * 0.6);

			// Thinking glow
			color += uColor * ch * uGlow * 0.35;

			gl_FragColor = vec4(color, ch);
		}
	`;

	// ── Reactive refs for the render loop ──────────────────────────────

	let moodRef = mood;
	let thinkingRef = thinking;
	let voiceRef = voiceAmplitude;

	$effect(() => { moodRef = mood; });
	$effect(() => { thinkingRef = thinking; });
	$effect(() => { voiceRef = voiceAmplitude; });

	// ── Mount ──────────────────────────────────────────────────────────

	onMount(() => {
		if (!container) return;

		const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: false });
		renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
		renderer.setClearColor(0x000000, 0);
		container.appendChild(renderer.domElement);

		// ── Creature scene ──

		const creatureScene = new THREE.Scene();
		const cam = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
		cam.position.set(0, 0, 3.0);

		creatureScene.add(new THREE.AmbientLight(0xffffff, 0.05));

		const keyLight = new THREE.DirectionalLight(0xffffff, 2.5);
		keyLight.position.set(3, 2, 4);
		creatureScene.add(keyLight);

		const rimLight = new THREE.PointLight(0xffffff, 0.6);
		rimLight.position.set(-3, 1, -2);
		creatureScene.add(rimLight);

		const fillLight = new THREE.PointLight(0xffffff, 0.15);
		fillLight.position.set(0, -3, 1);
		creatureScene.add(fillLight);

		// Creature mesh — subdivision 6 for smooth silhouette
		const creatureGeo = new THREE.IcosahedronGeometry(1, 6);
		const basePositions = new Float32Array(creatureGeo.attributes.position.array);
		const creatureMat = new THREE.MeshStandardMaterial({
			roughness: 0.55, metalness: 0.15, transparent: true, opacity: 0.92,
		});
		const creature = new THREE.Mesh(creatureGeo, creatureMat);
		creatureScene.add(creature);

		// Inner glow core
		const innerMat = new THREE.MeshStandardMaterial({
			color: 0xffffff, emissive: 0xffffff, emissiveIntensity: 0.25,
			transparent: true, opacity: 0.08,
		});
		creatureScene.add(new THREE.Mesh(new THREE.IcosahedronGeometry(0.45, 3), innerMat));

		// ── Render target ──

		const rt = new THREE.WebGLRenderTarget(COLS * 4, ROWS * 4);

		// ── ASCII pass ──

		const fontAtlas = createFontAtlas();
		const asciiUniforms = {
			tScene: { value: rt.texture },
			tFont: { value: fontAtlas },
			uCols: { value: COLS },
			uRows: { value: ROWS },
			uRampLen: { value: RAMP.length },
			uColor: { value: new THREE.Color(moodColors[matchMood(mood)]) },
			uGlow: { value: 0 },
			uResolution: { value: new THREE.Vector2() },
		};

		const asciiScene = new THREE.Scene();
		const asciiCam = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
		const asciiMat = new THREE.ShaderMaterial({
			uniforms: asciiUniforms,
			vertexShader: asciiVert,
			fragmentShader: asciiFrag,
			transparent: true,
		});
		asciiScene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), asciiMat));

		// ── Resize ──

		function resize() {
			if (!container) return;
			const w = container.clientWidth;
			const h = container.clientHeight;
			renderer.setSize(w, h);
			asciiUniforms.uResolution.value.set(w * renderer.getPixelRatio(), h * renderer.getPixelRatio());
		}
		resize();
		const ro = new ResizeObserver(resize);
		ro.observe(container);

		// ── Render loop ──

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

			// Update creature material
			creatureMat.color.copy(col);
			creatureMat.emissive!.copy(col);
			creatureMat.emissiveIntensity = thinkingRef ? 0.08 : 0.03;
			keyLight.color.copy(col);
			keyLight.intensity = thinkingRef ? 3.5 : 2.5;
			rimLight.intensity = thinkingRef ? 1.0 : 0.6;
			fillLight.color.copy(col);
			innerMat.emissiveIntensity = thinkingRef ? 0.6 : 0.25;
			innerMat.opacity = thinkingRef ? 0.2 : 0.08;

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

			// ASCII uniforms
			asciiUniforms.uColor.value.copy(col);
			const targetGlow = thinkingRef ? 1.0 : 0.0;
			glowSmooth += (targetGlow - glowSmooth) * Math.min(delta * 3.0, 0.15);
			asciiUniforms.uGlow.value = glowSmooth;

			// Pass 1: creature → render target
			renderer.setRenderTarget(rt);
			renderer.setClearColor(0x000000, 1);
			renderer.clear();
			renderer.render(creatureScene, cam);

			// Pass 2: ASCII → screen
			renderer.setRenderTarget(null);
			renderer.setClearColor(0x000000, 0);
			renderer.clear();
			renderer.render(asciiScene, asciiCam);
		}
		requestAnimationFrame(animate);

		return () => {
			running = false;
			ro.disconnect();
			creatureGeo.dispose();
			creatureMat.dispose();
			innerMat.dispose();
			asciiMat.dispose();
			fontAtlas.dispose();
			rt.dispose();
			renderer.dispose();
			renderer.domElement.remove();
		};
	});
</script>

<div class="ascii-shader" bind:this={container}></div>

<style>
	.ascii-shader {
		width: 100%;
		height: 100%;
	}

	.ascii-shader :global(canvas) {
		display: block;
		width: 100% !important;
		height: 100% !important;
	}
</style>
