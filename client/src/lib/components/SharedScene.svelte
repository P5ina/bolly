<script lang="ts">
	import { onMount } from "svelte";
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
		renderer.domElement.style.pointerEvents = "auto";

		const scene = new THREE.Scene();
		scene.background = new THREE.Color(0x010210);

		const cam = new THREE.PerspectiveCamera(50, 1, 0.1, 100);
		cam.position.set(0, 0, 5);
		cam.lookAt(0, 0, 0);

		// ── Skybox ──
		const skyGeo = new THREE.SphereGeometry(30, 64, 32);
		const skyMat = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide });
		const skyShader = Fn(() => {
			const st = uv();
			const t = time.mul(0.02);
			const abyss = vec3(0.005, 0.01, 0.04);
			const deep = vec3(0.01, 0.02, 0.07);
			const purp = vec3(0.03, 0.015, 0.08);
			const c = mix(abyss, deep, smoothstep(float(0), float(0.5), st.y)).toVar();
			c.assign(mix(c, purp, smoothstep(float(0.5), float(1), st.y)));
			const TAU = float(6.2832);
			const sx = st.x.mul(TAU);
			const addCaustic = (base: number, f1: number, s1: number, f2: number, s2: number, coreStr: number, glowStr: number) => {
				const curve = float(base).add(sin(sx.mul(f1).add(t.mul(s1))).mul(0.18)).add(cos(sx.mul(f2).sub(t.mul(s2)).add(1.5)).mul(0.10));
				const d = abs(st.y.sub(curve));
				c.addAssign(vec3(0.55, 0.60, 0.85).mul(pow(float(2.718), d.mul(d).mul(-2000)).mul(coreStr)));
				c.addAssign(vec3(0.20, 0.25, 0.50).mul(pow(float(2.718), d.mul(d).mul(-40)).mul(glowStr)));
			};
			addCaustic(0.50, 1, 0.6, 1, 0.9, 0.7, 0.12);
			addCaustic(0.35, 1, 0.4, 2, 0.7, 0.5, 0.08);
			addCaustic(0.68, 1, 0.8, 1, 0.5, 0.35, 0.06);
			return vec4(c, float(1));
		});
		skyMat.colorNode = skyShader();
		const skybox = new THREE.Mesh(skyGeo, skyMat);
		skybox.visible = false;
		scene.add(skybox);

		// ── Flat background ──
		const flatBg = Fn(() => {
			const st = screenUV;
			const t = time.mul(0.02);
			const abyss = vec3(0.005, 0.01, 0.04);
			const deep = vec3(0.01, 0.02, 0.07);
			const purp = vec3(0.03, 0.015, 0.08);
			const c = mix(abyss, deep, smoothstep(float(0), float(0.5), st.y)).toVar();
			c.assign(mix(c, purp, smoothstep(float(0.5), float(1), st.y)));
			const addLine = (base: number, freq1: number, sp1: number, freq2: number, sp2: number, cStr: number, gStr: number) => {
				const curve = float(base).add(sin(st.x.mul(freq1).add(t.mul(sp1))).mul(0.18)).add(cos(st.x.mul(freq2).sub(t.mul(sp2)).add(1.5)).mul(0.10));
				const d = abs(st.y.sub(curve));
				c.addAssign(vec3(0.55, 0.60, 0.85).mul(pow(float(2.718), d.mul(d).mul(-2000)).mul(cStr)));
				c.addAssign(vec3(0.20, 0.25, 0.50).mul(pow(float(2.718), d.mul(d).mul(-40)).mul(gStr)));
			};
			addLine(0.50, 2.5, 0.6, 4.5, 0.9, 0.7, 0.12);
			addLine(0.32, 3.0, 0.4, 5.5, 0.7, 0.5, 0.08);
			addLine(0.70, 2.0, 0.8, 3.5, 0.5, 0.35, 0.06);
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

		// ── Glass material (shared) ──
		const uSpeed = uniform(0.8);
		const uIntensity = uniform(0.08);
		const uBreathe = uniform(1.0);
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
		glassMat.dispersion = 0.15;
		glassMat.attenuationColor = new THREE.Color(0xffffff);
		glassMat.attenuationDistance = Infinity;
		glassMat.clearcoat = 0.1;
		glassMat.specularIntensity = 1.0;
		glassMat.specularColor = new THREE.Color(0xffffff);
		glassMat.envMapIntensity = 25;
		glassMat.transparent = true;
		glassMat.side = THREE.FrontSide;

		const sphereGeo = new THREE.IcosahedronGeometry(1, 6);

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
			t += delta;

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

				const isSpeaking = smoothAmp > 0.01;
				const thk = store.thinking;
				const tgtSpeed = thk ? 3.0 : isSpeaking ? energy.speed + smoothAmp * 1.5 : energy.speed;
				const tgtInt = thk ? 0.25 : isSpeaking ? energy.intensity + smoothAmp * 0.15 : energy.intensity;

				const uLerp = Math.min(delta * 3, 1);
				smoothSpeed += (tgtSpeed - smoothSpeed) * uLerp;
				smoothIntensity += (tgtInt - smoothIntensity) * uLerp;

				uSpeed.value = smoothSpeed;
				uIntensity.value = smoothIntensity;
				uBreathe.value = 1.0
					+ Math.sin(t * (thk ? 2.5 : energy.breatheRate)) * (thk ? 0.06 : energy.breatheDepth)
					+ (isSpeaking ? smoothAmp * 0.06 : 0);

				keyLight.color.set(moodColors[resolved]);
				keyLight.intensity = thk ? 2.5 : 2.0;
				fillLight.color.set(moodColors[resolved]);
				glassMat.dispersion = thk ? 0.6 : 0.4;

				// Override rotation for selected orb in chat
				const selOrb = sel ? orbMap.get(sel) : null;
				if (selOrb) {
					selOrb.mesh.rotation.y += delta * (thk ? 0.4 : energy.rotSpeed);
					selOrb.mesh.rotation.x = Math.sin(t * 0.3) * 0.1;
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

			renderer.render(scene, cam);
		}
		requestAnimationFrame(animate);

		return () => {
			running = false;
			ro.disconnect();
			renderer.domElement.removeEventListener("pointermove", onPointerMove);
			renderer.domElement.removeEventListener("pointerdown", onPointerDown);
			renderer.domElement.removeEventListener("pointerup", onPointerUp);
			sphereGeo.dispose();
			glassMat.dispose();
			skyGeo.dispose();
			skyMat.dispose();
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
</style>
