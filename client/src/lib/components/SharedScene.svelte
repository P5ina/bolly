<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { getSceneStore } from "$lib/stores/scene.svelte.js";

	const store = getSceneStore();

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

	function easeOutCubic(x: number) { return 1 - Math.pow(1 - x, 3); }
	function easeInOutQuart(x: number) {
		return x < 0.5 ? 8 * x * x * x * x : 1 - Math.pow(-2 * x + 2, 4) / 2;
	}

	// Final chat position
	const FINAL_X = 1.8, FINAL_Y = -0.1, FINAL_Z = 0, FINAL_SCALE = 1.2;
	const HOME_SCALE = 0.5;

	let cleanup: (() => void) | null = null;
	onDestroy(() => cleanup?.());

	onMount(async () => {
		if (!container) return;
		// Skip 3D scene on mobile — cards are shown instead
		if (window.innerWidth < 640) return;

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
		renderer.domElement.style.pointerEvents = "auto";

		const scene = new THREE.Scene();
		scene.background = new THREE.Color(0x000206);

		const cam = new THREE.PerspectiveCamera(50, 1, 0.1, 100);
		cam.position.set(0, 0, 5);
		cam.lookAt(0, 0, 0);

		// ── Skybox — dark gradient with subtle color for envmap reflections ──
		const skyGeo = new THREE.SphereGeometry(30, 32, 16);
		const skyMat = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide });
		const skyShader = Fn(() => {
			const st = uv();
			const t = time.mod(6283.0).mul(0.012);
			const E = float(2.718);
			// Dark base
			const c = vec3(0.003, 0.005, 0.018).toVar();
			c.addAssign(vec3(0.002, 0.003, 0.010).mul(st.y));
			// Soft aurora bands for envmap color
			const TAU = float(6.2832);
			const sx = st.x.mul(TAU);
			const band = (baseY: number, drift: number, phase: number, w: number, col: [number, number, number], bright: number) => {
				const wy = float(baseY).add(sin(sx.mul(0.8).add(t.mul(drift)).add(phase)).mul(0.06));
				const d = st.y.sub(wy);
				c.addAssign(vec3(col[0], col[1], col[2]).mul(pow(E, d.mul(d).negate().div(float(w * w * 2)))).mul(bright));
			};
			band(0.6, 0.4, 0, 0.14, [0.04, 0.12, 0.18], 0.12);
			band(0.4, 0.3, 2, 0.16, [0.06, 0.05, 0.15], 0.08);
			band(0.25, 0.5, 4, 0.10, [0.10, 0.03, 0.12], 0.06);
			return vec4(c, float(1));
		});
		skyMat.colorNode = skyShader();
		const skybox = new THREE.Mesh(skyGeo, skyMat);
		skybox.visible = false;
		scene.add(skybox);

		// ── Background — dark base with disco mode uniforms ──
		const uDiscoBeat = uniform(0.0);   // 0-1 music amplitude
		const uDiscoHue = uniform(0.0);    // 0-1 hue rotation
		const uDiscoActive = uniform(0.0); // 0 or 1

		const flatBg = Fn(() => {
			const st = screenUV;
			// Dark base
			const c = vec3(0.002, 0.003, 0.012).toVar();
			c.addAssign(vec3(0.001, 0.002, 0.006).mul(st.y));

			// Disco: color-cycling radial glow from center
			const center = st.sub(vec3(0.5, 0.45, 0));
			const dist = center.length();
			const glow = pow(
				float(1.0).sub(smoothstep(0.0, 0.8, dist)),
				float(2.0)
			);

			// HSV-like hue to RGB
			const h = uDiscoHue;
			const r = abs(h.mul(6.0).sub(3.0)).sub(1.0).clamp(0.0, 1.0);
			const g = float(2.0).sub(abs(h.mul(6.0).sub(2.0))).clamp(0.0, 1.0);
			const b = float(2.0).sub(abs(h.mul(6.0).sub(4.0))).clamp(0.0, 1.0);
			const discoColor = vec3(r, g, b);

			// Mix in disco glow scaled by beat intensity
			const intensity = uDiscoBeat.mul(glow).mul(uDiscoActive).mul(0.08);
			c.addAssign(discoColor.mul(intensity));

			return c;
		});
		scene.backgroundNode = flatBg();

		// ── Star particles — real 3D points for parallax + glass refraction ──
		const STAR_COUNT = 800;
		const starPositions = new Float32Array(STAR_COUNT * 3);
		const starSizes = new Float32Array(STAR_COUNT);
		const starColors = new Float32Array(STAR_COUNT * 3);

		for (let i = 0; i < STAR_COUNT; i++) {
			// Distribute in a sphere shell (radius 8..25)
			const theta = Math.random() * Math.PI * 2;
			const phi = Math.acos(2 * Math.random() - 1);
			const r = 8 + Math.random() * 17;
			starPositions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
			starPositions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
			starPositions[i * 3 + 2] = r * Math.cos(phi);
			// Size variation — mostly small, few bright
			const rnd = Math.random();
			starSizes[i] = rnd < 0.95 ? 1.0 + Math.random() * 2.0 : 3.0 + Math.random() * 4.0;
			// Color — cool blue-white to warm
			const warmth = Math.random();
			starColors[i * 3] = 0.6 + warmth * 0.4;
			starColors[i * 3 + 1] = 0.7 + warmth * 0.2;
			starColors[i * 3 + 2] = 1.0 - warmth * 0.3;
		}

		const starGeo = new THREE.BufferGeometry();
		starGeo.setAttribute("position", new THREE.BufferAttribute(starPositions, 3));
		starGeo.setAttribute("size", new THREE.BufferAttribute(starSizes, 1));
		starGeo.setAttribute("color", new THREE.BufferAttribute(starColors, 3));

		const starMat = new THREE.PointsMaterial({
			size: 0.06,
			sizeAttenuation: true,
			vertexColors: true,
			transparent: true,
			opacity: 0.85,
			depthWrite: false,
			blending: THREE.AdditiveBlending,
		});

		const stars = new THREE.Points(starGeo, starMat);
		scene.add(stars);

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

		// ── Visualizer columns (party mode) ──
		// 5 columns placed directly behind the blob so refraction shows them
		const VIZ_BARS = 5;
		const VIZ_WIDTH = 0.7;   // small — glass refraction magnifies them
		const VIZ_Z_BEHIND = -2; // behind the blob (blob is at z=0)
		const barGeo = new THREE.BoxGeometry(1, 1, 1);
		const vizGroup = new THREE.Group();
		// Place columns behind the blob along the camera→blob vector.
		// Camera is at (0,0,5), blob at (FINAL_X, FINAL_Y, 0).
		// Direction: normalize(blob - cam), then offset blob by ~2.5 units along it.
		{
			const dx = FINAL_X - 0, dy = FINAL_Y - 0, dz = FINAL_Z - 5;
			const len = Math.sqrt(dx * dx + dy * dy + dz * dz);
			const VIZ_DEPTH = 1.8; // closer to blob so they hide behind it
			vizGroup.position.set(
				FINAL_X + (dx / len) * VIZ_DEPTH,
				FINAL_Y + (dy / len) * VIZ_DEPTH,
				FINAL_Z + (dz / len) * VIZ_DEPTH,
			);
		}
		// Orient the group to face the camera
		vizGroup.lookAt(0, 0, 5);
		vizGroup.visible = false;
		scene.add(vizGroup);

		const vizBars: InstanceType<typeof THREE.Mesh>[] = [];
		const vizMats: InstanceType<typeof THREE.MeshStandardMaterial>[] = [];
		const barSpacing = VIZ_WIDTH / VIZ_BARS;
		const barWidth = barSpacing * 0.65;

		for (let i = 0; i < VIZ_BARS; i++) {
			const mat = new THREE.MeshStandardMaterial({
				color: 0x000000,     // no diffuse — purely emissive
				emissive: 0x000000,  // set per-frame in animate loop
				roughness: 0.1,
				metalness: 0.0,
				transparent: false,
			});
			const mesh = new THREE.Mesh(barGeo, mat);
			// Columns spread along local X, in the group's plane facing camera
			const x = -VIZ_WIDTH / 2 + i * barSpacing + barSpacing / 2;
			mesh.position.set(x, 0, 0);
			mesh.scale.set(barWidth, 0.01, 0.08);
			vizGroup.add(mesh);
			vizBars.push(mesh);
			vizMats.push(mat);
		}

		const vizSmooth = new Float32Array(VIZ_BARS);

		// ── Glass material (shared) ──
		const uSpeed = uniform(0.8);
		const uIntensity = uniform(0.08);
		const uBreathe = uniform(1.0);
		const displacedPos = Fn(() => {
			const pos = positionLocal.toVar();
			// Wrap time to prevent float32 precision loss in sin/cos on GPU
			// after long idle periods. 6283 ≈ 1000*2π, so wrapping is seamless.
			const t = time.mod(6283.0);
			const noise = sin(pos.x.mul(2.1).add(t.mul(uSpeed).mul(0.7)))
				.mul(cos(pos.y.mul(1.8).add(t.mul(uSpeed).mul(0.5))))
				.mul(sin(pos.z.mul(2.5).add(t.mul(uSpeed).mul(0.9))));
			return pos.mul(uBreathe.add(noise.mul(uIntensity)));
		});

		const glassMat = new THREE.MeshPhysicalNodeMaterial();
		glassMat.positionNode = displacedPos();
		glassMat.color = new THREE.Color(0xffffff);
		glassMat.transmission = 1.0;
		glassMat.ior = 1.45;
		glassMat.thickness = 1.5;
		glassMat.roughness = 0.0;
		glassMat.metalness = 0.0;
		glassMat.dispersion = 0.15;
		glassMat.attenuationColor = new THREE.Color(0xffffff);
		glassMat.attenuationDistance = Infinity;
		glassMat.clearcoat = 0.0;
		glassMat.specularIntensity = 0.2;
		glassMat.specularColor = new THREE.Color(0xffffff);
		glassMat.envMapIntensity = 1.0;
		glassMat.transparent = true;
		glassMat.side = THREE.FrontSide;

		const sphereGeo = new THREE.IcosahedronGeometry(1, 4);

		// ── Sphere pool ──
		interface OrbState {
			slug: string;
			mesh: InstanceType<typeof THREE.Mesh>;
			homeX: number;
			homeY: number;
			curX: number;
			curY: number;
			curScale: number;
		}
		const orbMap = new Map<string, OrbState>();
		const orbGroup = new THREE.Group();
		orbGroup.position.set(0, -0.15, 0);
		scene.add(orbGroup);

		function syncOrbs() {
			const list = store.instances;
			const slugs = new Set(list.map(i => i.slug));

			// Add selected slug if not in list (direct navigation)
			if (store.selectedSlug && !slugs.has(store.selectedSlug)) {
				slugs.add(store.selectedSlug);
			}

			// Remove old
			for (const [slug, orb] of orbMap) {
				if (!slugs.has(slug)) {
					orbGroup.remove(orb.mesh);
					orbMap.delete(slug);
				}
			}

			// Add new
			for (const slug of slugs) {
				if (!orbMap.has(slug)) {
					const mesh = new THREE.Mesh(sphereGeo, glassMat);
					mesh.scale.setScalar(0);
					mesh.userData.slug = slug;
					orbGroup.add(mesh);
					orbMap.set(slug, { slug, mesh, homeX: 0, homeY: 0, curX: 0, curY: 0, curScale: 0 });
				}
			}

			// Calculate home positions
			const count = list.length;
			const spacing = 1.4;
			const totalW = (count - 1) * spacing;
			const startX = -totalW / 2;
			list.forEach((inst, i) => {
				const orb = orbMap.get(inst.slug);
				if (orb) {
					orb.homeX = startX + i * spacing;
					orb.homeY = 0;
				}
			});
		}

		// ── Envmap ──
		const pmrem = new THREE.PMREMGenerator(renderer);
		let envDone = false;

		// ── Raycaster ──
		const raycaster = new THREE.Raycaster();
		const pointer = new THREE.Vector2();
		let ptrDown = false;

		function updatePtr(e: PointerEvent) {
			if (!container) return;
			const r = container.getBoundingClientRect();
			pointer.x = ((e.clientX - r.left) / r.width) * 2 - 1;
			pointer.y = -((e.clientY - r.top) / r.height) * 2 + 1;
		}

		function onPointerMove(e: PointerEvent) {
			if (store.mode !== "home") return;
			updatePtr(e);
			raycaster.setFromCamera(pointer, cam);
			const meshes = [...orbMap.values()].map(o => o.mesh);
			const hits = raycaster.intersectObjects(meshes, false);
			store.hoveredSlug = hits.length > 0 ? (hits[0].object.userData.slug ?? null) : null;
			renderer.domElement.style.cursor = store.hoveredSlug ? "pointer" : "default";
		}

		function onPointerDown(e: PointerEvent) {
			if (store.mode !== "home") return;
			updatePtr(e);
			ptrDown = true;
		}

		function onPointerUp(e: PointerEvent) {
			if (!ptrDown || store.mode !== "home") { ptrDown = false; return; }
			ptrDown = false;
			updatePtr(e);
			raycaster.setFromCamera(pointer, cam);
			const meshes = [...orbMap.values()].map(o => o.mesh);
			const hits = raycaster.intersectObjects(meshes, false);
			if (hits.length > 0) {
				const slug = hits[0].object.userData.slug;
				if (slug) store.selectInstance(slug);
			}
		}

		renderer.domElement.addEventListener("pointermove", onPointerMove);
		renderer.domElement.addEventListener("pointerdown", onPointerDown);
		renderer.domElement.addEventListener("pointerup", onPointerUp);

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
		let skipFrame = false;
		let smoothAmp = 0;
		let smoothMusicAmp = 0;
		let beatPulse = 0;         // 0→1 on beat hit, decays to 0
		let prevSpectrum: number[] = [];  // previous frame's frequency data
		const fluxHistory: number[] = [];  // rolling window of spectral flux values
		const FLUX_HISTORY_SIZE = 30;      // ~0.5s at 60fps (skip every other = ~1s)
		let lastBeatTime = 0;
		let smoothSpeed = 0.8;
		let smoothIntensity = 0.08;
		let lastMode: string = "";

		function animate() {
			if (!running) return;
			requestAnimationFrame(animate);
			skipFrame = !skipFrame;
			if (skipFrame) return;

			const now = performance.now();
			const delta = (now - prevTime) / 1000;
			prevTime = now;
			t = (t + delta) % 6283; // wrap to match shader time.mod(6283)

			store.tick();
			syncOrbs();

			const m = store.mode;
			const sel = store.selectedSlug;

			// Reset smooth shader values when leaving chat
			if (m !== lastMode) {
				if (lastMode === "chat" || m === "home") {
					smoothSpeed = 0.8;
					smoothIntensity = 0.08;
					smoothAmp = 0;
				}
				lastMode = m;
			}

			// ── Per-orb animation ──
			// In selecting/intro the easing functions already produce smooth curves,
			// so we write position/scale directly. Lerp is only for home↔chat.
			const useLerp = m === "home" || m === "chat" || m === "onboarding";
			const lerpF = Math.min(delta * 6, 1);

			for (const [slug, orb] of orbMap) {
				const isSelected = slug === sel;
				const isHovered = slug === store.hoveredSlug;
				let tx = orb.homeX;
				let ty = orb.homeY;
				let ts = HOME_SCALE;

				if (m === "home") {
					ts = isHovered ? 0.58 : HOME_SCALE;
				} else if (m === "onboarding") {
					if (isSelected) {
						tx = 0; ty = 0; ts = 0.9;
					} else {
						ts = 0;
					}
				} else if (m === "selecting") {
					const e = easeInOutQuart(store.selectProgress);
					if (isSelected) {
						tx = orb.homeX * (1 - e);
						ty = orb.homeY * (1 - e);
						ts = HOME_SCALE + e * (FINAL_SCALE - HOME_SCALE);
					} else {
						ts = HOME_SCALE * (1 - e);
					}
				} else if (m === "intro") {
					if (isSelected) {
						const p = store.introProgress;
						if (p < 0.25) {
							const e = easeOutCubic(p / 0.25);
							tx = 0;
							ty = 0;
							ts = FINAL_SCALE + e * 0.15;
						} else if (p < 0.58) {
							const e = easeInOutQuart((p - 0.25) / 0.33);
							tx = FINAL_X * e;
							ty = FINAL_Y * e;
							ts = FINAL_SCALE * 1.15 + (FINAL_SCALE - FINAL_SCALE * 1.15) * e;
						} else {
							tx = FINAL_X;
							ty = FINAL_Y;
							ts = FINAL_SCALE;
						}
					} else {
						ts = 0;
					}
				} else if (m === "chat") {
					if (isSelected) {
						tx = FINAL_X; ty = FINAL_Y; ts = FINAL_SCALE;
					} else {
						ts = 0;
					}
				}

				if (useLerp) {
					orb.curX += (tx - orb.curX) * lerpF;
					orb.curY += (ty - orb.curY) * lerpF;
					orb.curScale += (ts - orb.curScale) * lerpF;
				} else {
					// Direct — easing already smooth
					orb.curX = tx;
					orb.curY = ty;
					orb.curScale = ts;
				}

				orb.mesh.position.set(orb.curX, orb.curY, 0);
				orb.mesh.scale.setScalar(Math.max(orb.curScale, 0.001));
				orb.mesh.visible = orb.curScale > 0.01;

				// Gentle rotation
				const idx = [...orbMap.keys()].indexOf(slug);
				orb.mesh.rotation.y = t * 0.12 + idx * 1.5;
				orb.mesh.rotation.x = Math.sin(t * 0.25 + idx) * 0.06;
			}

			// ── Shader uniforms ──
			if (m === "onboarding") {
				uSpeed.value = 0.6;
				uIntensity.value = 0.08;
				uBreathe.value = 1.0 + Math.sin(t * 0.8) * 0.04;
				glassMat.dispersion = 0.15;
			} else if (m === "selecting") {
				// Gently ramp up from home defaults
				const e = easeInOutQuart(store.selectProgress);
				uSpeed.value = 0.8 + e * 0.3;
				uIntensity.value = 0.08 + e * 0.04;
				uBreathe.value = 1.0 + Math.sin(t * 1.0) * 0.03;
				glassMat.dispersion = 0.15 + e * 0.1;
			} else if (m === "intro") {
				const p = store.introProgress;
				// Calm intro — slight ramp then settle
				const ramp = Math.min(p * 4, 1); // 0→1 over first 25%
				const settle = p > 0.58 ? (p - 0.58) / 0.42 : 0;
				uSpeed.value = 1.0 + ramp * 0.2 - settle * 0.2;
				uIntensity.value = 0.12 + ramp * 0.03 - settle * 0.03;
				uBreathe.value = 1.0 + Math.sin(t * 1.2) * 0.04;
				glassMat.dispersion = 0.2 + ramp * 0.1 - settle * 0.1;
			} else if (m === "chat") {
				const resolved = matchMood(store.mood);
				const energy = moodEnergies[resolved] ?? defaultEnergy;

				const rawAmp = store.voiceAmplitude;
				const lerpUp = Math.min(delta * 6, 1);
				const lerpDown = Math.min(delta * 2, 1);
				smoothAmp += (rawAmp - smoothAmp) * (rawAmp > smoothAmp ? lerpUp : lerpDown);

				// Music visualizer amplitude — smoothed to prevent jitter
				const rawMusic = store.musicAmplitude;
				const musicLerpUp = Math.min(delta * 4, 1);   // moderate attack
				const musicLerpDown = Math.min(delta * 1.5, 1); // slow decay
				smoothMusicAmp += (rawMusic - smoothMusicAmp) * (rawMusic > smoothMusicAmp ? musicLerpUp : musicLerpDown);
				const mAmp = smoothMusicAmp;
				const isMusic = store.musicPlaying && mAmp > 0.01;

				const isSpeaking = smoothAmp > 0.01;
				const thk = store.thinking;

				// Music mode: subtle noise + strong BPM pulse
				const tgtSpeed = isMusic
					? 0.6
					: thk ? 3.0 : isSpeaking ? energy.speed + smoothAmp * 1.5 : energy.speed;
				const tgtInt = isMusic
					? 0.03 + mAmp * 0.02
					: thk ? 0.25 : isSpeaking ? energy.intensity + smoothAmp * 0.15 : energy.intensity;

				const uLerp = Math.min(delta * 3, 1);
				smoothSpeed += (tgtSpeed - smoothSpeed) * uLerp;
				smoothIntensity += (tgtInt - smoothIntensity) * uLerp;

				uSpeed.value = smoothSpeed;
				uIntensity.value = smoothIntensity;

				if (isMusic) {
					// Spectral flux onset detection — measures change in frequency
					// spectrum between frames. Much more reliable than raw bass energy.
					const freqData = store.getMusicFrequencyData();
					if (freqData) {
						// Initialize prev spectrum on first frame
						if (prevSpectrum.length !== freqData.length) {
							prevSpectrum = Array.from(freqData);
						}
						// Spectral flux: sum of positive differences (only increases)
						// Focus on low-mid frequencies (first 40% of bins)
						const rangeEnd = Math.floor(freqData.length * 0.4);
						let flux = 0;
						for (let j = 0; j < rangeEnd; j++) {
							const diff = freqData[j] - prevSpectrum[j];
							if (diff > 0) flux += diff;
							prevSpectrum[j] = freqData[j];
						}
						flux /= rangeEnd; // normalize

						// Adaptive threshold: mean + 1.5 × stddev of recent flux
						fluxHistory.push(flux);
						if (fluxHistory.length > FLUX_HISTORY_SIZE) fluxHistory.shift();

						if (fluxHistory.length >= 8) {
							const mean = fluxHistory.reduce((a, b) => a + b, 0) / fluxHistory.length;
							const variance = fluxHistory.reduce((a, b) => a + (b - mean) ** 2, 0) / fluxHistory.length;
							const stddev = Math.sqrt(variance);
							const threshold = mean + 1.5 * stddev;

							// Beat detected if flux exceeds adaptive threshold + cooldown
							const now = performance.now() / 1000;
							if (flux > threshold && flux > 3 && (now - lastBeatTime) > 0.15) {
								beatPulse = 1.0;
								lastBeatTime = now;
							}
						}
					}
					// Exponential decay for punchy feel
					beatPulse *= Math.pow(0.02, delta);
					// Scale: base 0.88, expand to 1.18 on beat
					uBreathe.value = 0.88 + beatPulse * 0.30;
				} else {
					uBreathe.value = 1.0
						+ Math.sin(t * (thk ? 2.5 : energy.breatheRate)) * (thk ? 0.06 : energy.breatheDepth)
						+ (isSpeaking ? smoothAmp * 0.06 : 0);
				}

				// Disco color cycling when music plays
				if (isMusic) {
					const hue = (t * 0.4) % 1;
					const boost = 0.5 + mAmp * 0.5;
					const r = Math.sin(hue * Math.PI * 2) * 0.5 + 0.5;
					const g = Math.sin((hue + 0.333) * Math.PI * 2) * 0.5 + 0.5;
					const b = Math.sin((hue + 0.666) * Math.PI * 2) * 0.5 + 0.5;
					keyLight.color.setRGB(r * boost, g * boost, b * boost);
					keyLight.intensity = 2.0 + mAmp * 1.0;
					fillLight.color.setRGB(b * boost, r * boost, g * boost);
					fillLight.intensity = 0.6 + mAmp * 0.8;
					rimLight.color.setRGB(g * boost, b * boost, r * boost);
					rimLight.intensity = 0.8 + mAmp * 1.0;
					glassMat.dispersion = 0.4 + mAmp * 0.3;

					// Drive background shader uniforms
					uDiscoActive.value = 1.0;
					uDiscoBeat.value += (mAmp - uDiscoBeat.value) * Math.min(delta * 10, 1);
					uDiscoHue.value = hue;
				} else {
					keyLight.color.set(moodColors[resolved]);
					keyLight.intensity = thk ? 2.5 : 2.0;
					fillLight.color.set(moodColors[resolved]);
					fillLight.intensity = 0.4;
					rimLight.color.set(0x8899cc);
					rimLight.intensity = 1.0;
					glassMat.dispersion = thk ? 0.6 : 0.4;

					// Fade out disco background
					uDiscoActive.value = Math.max(0, uDiscoActive.value - delta * 3);
					uDiscoBeat.value *= 0.95;
				}

				// Override rotation for selected orb in chat
				const selOrb = sel ? orbMap.get(sel) : null;
				if (selOrb) {
					const rotSpeed = isMusic ? 0.3 + mAmp * 0.5 : thk ? 0.4 : energy.rotSpeed;
					selOrb.mesh.rotation.y += delta * rotSpeed;
					selOrb.mesh.rotation.x = Math.sin(t * (isMusic ? 0.8 : 0.3)) * (isMusic ? 0.12 : 0.1);
				}
			} else {
				// Home / selecting — gentle defaults
				uSpeed.value = 0.8;
				uIntensity.value = 0.08;
				uBreathe.value = 1.0 + Math.sin(t * 1.0) * 0.03;
				glassMat.dispersion = 0.15;
			}

			// Envmap (once)
			if (!envDone && t > 0.15) {
				for (const orb of orbMap.values()) orb.mesh.visible = false;
				skybox.visible = true;
				const envRT = pmrem.fromScene(scene);
				scene.environment = envRT.texture;
				skybox.visible = false;
				for (const orb of orbMap.values()) orb.mesh.visible = orb.curScale > 0.01;
				envDone = true;
			}

			// ── Visualizer columns update ──
			const isDiscoNow = store.musicPlaying && store.musicAmplitude > 0.01;
			vizGroup.visible = isDiscoNow;

			if (isDiscoNow) {
				const freqData = store.getMusicFrequencyData();
				if (freqData) {
					// Frequency bands tuned so each column gets similar energy.
					// Skip sub-bass (0-2%), spread usable range more evenly.
					const total = freqData.length;
					const bandEdges = [0.02, 0.06, 0.14, 0.28, 0.50, 0.85]; // 5 bands
					const hue = (t * 0.4) % 1;
					for (let i = 0; i < VIZ_BARS; i++) {
						const lo = Math.floor(bandEdges[i] * total);
						const hi = Math.floor(bandEdges[i + 1] * total);
						// Use peak (max bin) instead of average for punchier response
						let peak = 0;
						for (let j = lo; j < hi; j++) {
							if (freqData[j] > peak) peak = freqData[j];
						}
						const raw = peak / 255;
						// Compress with sqrt so loud doesn't dominate
						const compressed = Math.sqrt(raw);
						vizSmooth[i] += (compressed - vizSmooth[i]) * (compressed > vizSmooth[i] ? 0.4 : 0.15);
						const h = vizSmooth[i];

						// Small height — glass sphere magnifies via refraction
						// Center columns at y=0 (sphere center)
						const barHeight = 0.02 + h * 0.4;
						vizBars[i].scale.y = barHeight;
						vizBars[i].position.y = 0;

						// Purely emissive — no diffuse color, no dark silhouettes.
						// Transmission pass sees bright glowing bars; refraction
						// bends them into the sphere as color.
						const barHue = (hue + i / VIZ_BARS) % 1;
						const r = Math.sin(barHue * Math.PI * 2) * 0.5 + 0.5;
						const g = Math.sin((barHue + 0.333) * Math.PI * 2) * 0.5 + 0.5;
						const b2 = Math.sin((barHue + 0.666) * Math.PI * 2) * 0.5 + 0.5;
						const brightness = 0.5 + h * 2.0;
						vizMats[i].emissive.setRGB(r * brightness, g * brightness, b2 * brightness);
					}
				}
			} else {
				for (let i = 0; i < VIZ_BARS; i++) {
					vizSmooth[i] *= 0.9;
				}
			}

			// Star field — faster rotation + brightness pulse in disco mode
			stars.rotation.y = t * (isDiscoNow ? 0.05 : 0.01);
			stars.rotation.x = t * (isDiscoNow ? 0.02 : 0.003);
			starMat.opacity = isDiscoNow ? 0.5 + store.musicAmplitude * 0.5 : 0.85;

			// Presentation mode: camera looks at the blob so it's centered
			if (store.presenting && m === "chat") {
				cam.lookAt(FINAL_X, FINAL_Y, FINAL_Z);
			} else {
				cam.lookAt(0, 0, 0);
			}

			renderer.render(scene, cam);
		}
		requestAnimationFrame(animate);

		cleanup = () => {
			running = false;
			ro.disconnect();
			renderer.domElement.removeEventListener("pointermove", onPointerMove);
			renderer.domElement.removeEventListener("pointerdown", onPointerDown);
			renderer.domElement.removeEventListener("pointerup", onPointerUp);
			sphereGeo.dispose();
			glassMat.dispose();
			skyGeo.dispose();
			skyMat.dispose();
			starGeo.dispose();
			starMat.dispose();
			barGeo.dispose();
			vizMats.forEach(m => m.dispose());
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
	}
	.scene-root :global(canvas) {
		display: block;
		width: 100% !important;
		height: 100% !important;
	}
	@media (max-width: 640px) {
		.scene-root { display: none; }
	}
</style>
