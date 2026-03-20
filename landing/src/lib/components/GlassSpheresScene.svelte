<script lang="ts">
	import { onMount } from 'svelte';
	import { T, useTask, useThrelte } from '@threlte/core';
	import { HTML, Text } from '@threlte/extras';
	import * as THREE from 'three';

	const { scene, camera, renderer, size } = useThrelte();

	// ── HTML → Texture (exact copy of three-html-render approach) ──
	class HtmlRenderer {
		async update(node: HTMLElement): Promise<THREE.CanvasTexture> {
			const w = node.clientWidth;
			const h = node.clientHeight;

			const serialized = new XMLSerializer().serializeToString(node);
			const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}"><foreignObject width="100%" height="100%">${serialized}</foreignObject></svg>`;
			const dataUrl = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);

			const image = await new Promise<HTMLImageElement>((resolve, reject) => {
				const img = new Image();
				img.onload = () => resolve(img);
				img.onerror = reject;
				img.src = dataUrl;
			});

			// Each texture gets its OWN canvas (no shared reference)
			const canvas = document.createElement('canvas');
			canvas.width = w;
			canvas.height = h;
			const ctx = canvas.getContext('2d')!;
			// Draw dark background first (HTML has transparent backgrounds)
			ctx.fillStyle = '#000206';
			ctx.fillRect(0, 0, w, h);
			ctx.drawImage(image, 0, 0, w, h);

			const tex = new THREE.CanvasTexture(canvas);
			tex.needsUpdate = true;
			return tex;
		}
	}

	const htmlRenderer = new HtmlRenderer();
	const backingPlanes: { mesh: THREE.Mesh; el: HTMLElement }[] = [];

	scene.background = new THREE.Color(0x000206);

	const CAM_Z = 14;
	const FOV = 50;
	const TOTAL_HEIGHT = 85;

	// ── Render target for refraction ──
	let fbo: THREE.WebGLRenderTarget | null = null;

	// ── Fonts ──
	const fraunces = 'https://fonts.gstatic.com/s/fraunces/v38/6NVf8FyLNQOQZAnv9ZwNjucMHVn85Ni7emAe9lKqZTnbB-gzTK0K1ChJdt9vIVYX9G37lod9sPEKsxx664UJf1hLTf7W.ttf';
	const bricolage = 'https://fonts.gstatic.com/s/bricolagegrotesque/v9/3y9U6as8bTXq_nANBjzKo3IeZx8z6up5BeSl5jBNz_19PpbJMXuECpwUxJBOm_OJWiaaD30YfKfjZZoLvRviyM0.ttf';

	// ── Colors ──
	const warm = '#c4a265';
	const text = '#e6dcc8';
	const dim = '#8a8070';
	const ghost = '#55504a';

	// ── Starfield ──
	const STAR_COUNT = 1500;
	const starPos = new Float32Array(STAR_COUNT * 3);
	const starCol = new Float32Array(STAR_COUNT * 3);
	for (let i = 0; i < STAR_COUNT; i++) {
		const theta = Math.random() * Math.PI * 2;
		const phi = Math.acos(2 * Math.random() - 1);
		const r = 15 + Math.random() * 45;
		starPos[i * 3] = r * Math.sin(phi) * Math.cos(theta);
		starPos[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
		starPos[i * 3 + 2] = r * Math.cos(phi);
		const w = Math.random();
		starCol[i * 3] = 0.6 + w * 0.4;
		starCol[i * 3 + 1] = 0.7 + w * 0.2;
		starCol[i * 3 + 2] = 1.0 - w * 0.3;
	}
	const starGeo = new THREE.BufferGeometry();
	starGeo.setAttribute('position', new THREE.BufferAttribute(starPos, 3));
	starGeo.setAttribute('color', new THREE.BufferAttribute(starCol, 3));
	const starMat = new THREE.PointsMaterial({
		size: 0.08, sizeAttenuation: true, vertexColors: true,
		transparent: true, opacity: 0.7, depthWrite: false,
		blending: THREE.AdditiveBlending,
	});

	// ── Custom refraction shader ──
	const refractionMat = new THREE.ShaderMaterial({
		vertexShader: /* glsl */ `
			varying vec3 vWorldNormal;
			varying vec3 vViewDir;
			varying vec2 vScreenUV;
			void main() {
				vec4 worldPos = modelMatrix * vec4(position, 1.0);
				vWorldNormal = normalize((modelMatrix * vec4(normal, 0.0)).xyz);
				vViewDir = normalize(worldPos.xyz - cameraPosition);
				vec4 clip = projectionMatrix * viewMatrix * worldPos;
				vScreenUV = clip.xy / clip.w * 0.5 + 0.5;
				gl_Position = clip;
			}
		`,
		fragmentShader: /* glsl */ `
			uniform sampler2D uSceneTex;
			uniform float uIOR;
			uniform float uChroma;
			uniform float uFresnelPow;

			varying vec3 vWorldNormal;
			varying vec3 vViewDir;
			varying vec2 vScreenUV;

			void main() {
				vec3 N = normalize(vWorldNormal);
				vec3 V = normalize(vViewDir);

				// Snell refraction
				vec3 refr = refract(V, N, 1.0 / uIOR);
				vec2 offset = refr.xy * 0.03;

				// Chromatic aberration
				float r = texture2D(uSceneTex, vScreenUV + offset * (1.0 + uChroma)).r;
				float g = texture2D(uSceneTex, vScreenUV + offset).g;
				float b = texture2D(uSceneTex, vScreenUV + offset * (1.0 - uChroma)).b;
				vec3 refracted = vec3(r, g, b);

				// Fresnel — subtle edge
				float fresnel = pow(1.0 + dot(V, N), uFresnelPow);

				// Specular — single sharp highlight
				vec3 refl = reflect(V, N);
				float spec = pow(max(dot(refl, normalize(vec3(3, 2, 4))), 0.0), 120.0) * 0.25;

				// Mix — mostly refracted, minimal Fresnel
				vec3 col = mix(refracted, vec3(0.04, 0.06, 0.10), fresnel * 0.12);
				col += spec;
				col += fresnel * vec3(0.02, 0.03, 0.06) * 0.15;

				gl_FragColor = vec4(col, 1.0);
			}
		`,
		uniforms: {
			uSceneTex: { value: null },
			uIOR: { value: 1.05 },
			uChroma: { value: 0.02 },
			uFresnelPow: { value: 3.0 },
		},
	});
	const mainGeo = new THREE.IcosahedronGeometry(1, 12);
	const smallGeo = new THREE.IcosahedronGeometry(0.5, 8);
	const tinyGeo = new THREE.IcosahedronGeometry(0.3, 6);

	let mainSphere: THREE.Mesh | undefined;
	let smallSphere: THREE.Mesh | undefined;
	let tinySphere: THREE.Mesh | undefined;
	let starsRef: THREE.Points | undefined;

	// ── Features data ──
	const features = [
		{ title: 'helps you focus', desc: 'Track your tasks, break down complex goals, and stay on course.' },
		{ title: 'studies with you', desc: 'Explain concepts, quiz you, discuss what you\'re reading.' },
		{ title: 'feels your mood', desc: 'Notices when you\'re stressed, tired, or excited.' },
		{ title: 'thinks with you', desc: 'Talk through ideas, decisions, creative blocks.' },
		{ title: 'checks in on you', desc: 'Wakes up on its own. Reflects, journals, reaches out.' },
		{ title: 'completely private', desc: 'Fully encrypted and private. No one else can access.' },
	];

	// ── Steps data ──
	const steps = [
		{ num: '01', title: 'sign up', desc: 'Pick a plan. Your environment spins up in seconds.' },
		{ num: '02', title: 'shape who they are', desc: 'Choose a personality or write your own.' },
		{ num: '03', title: 'just talk', desc: 'They remember everything. Their mood shifts. They grow.' },
	];

	// ── Plans data ──
	const plans = [
		{ name: 'starter', price: '$12', desc: 'See if it clicks', features: ['1M tokens/mo', '10 GB', 'Mood tracking'] },
		{ name: 'companion', price: '$29', desc: 'For everyday life', features: ['3M tokens/mo', '20 GB', 'Web browsing', 'Email'], featured: true },
		{ name: 'real friend', price: '$59', desc: 'No limits', features: ['10M tokens/mo', '50 GB', 'Web browsing', 'Early access'] },
	];

	// ── Scroll & mouse ──

	// ── Auto-capture all HTML elements after mount ──
	onMount(() => {
		setTimeout(async () => {
			// ── Capture HTML elements and place backing planes at known 3D positions ──
			const canvasParent = renderer.domElement.parentElement;
			if (!canvasParent) return;

			// ── Capture by data-backing attribute ──
			const pxToUnit = 0.024;

			const backingDefs: { id: string; x: number; y: number; s: number }[] = [];
			for (let i = 0; i < 6; i++) {
				const col = i % 3, row = Math.floor(i / 3);
				backingDefs.push({ id: `feature-${i}`, x: -6 + col * 6, y: Y.features - row * 4.5, s: 0.6 });
			}
			backingDefs.push({ id: 'demo', x: -4, y: Y.demo + 1, s: 0.7 });
			for (let i = 0; i < 3; i++) backingDefs.push({ id: `step-${i}`, x: -5 + i * 5, y: Y.how - 0.5, s: 0.5 });
			for (let i = 0; i < 3; i++) backingDefs.push({ id: `price-${i}`, x: -5 + i * 5, y: Y.pricing - 0.5, s: 0.5 });

			let count = 0;
			for (const def of backingDefs) {
				const el = document.querySelector<HTMLElement>(`[data-backing="${def.id}"]`);
				if (!el) { console.warn(`[GlassScene] ✗ ${def.id}: not found`); continue; }

				try {
					const tex = await htmlRenderer.update(el);
					const pw = el.clientWidth * pxToUnit * def.s;
					const ph = el.clientHeight * pxToUnit * def.s;

					const mesh = new THREE.Mesh(
						new THREE.PlaneGeometry(pw, ph),
						new THREE.MeshBasicMaterial({ map: tex, side: THREE.DoubleSide })
					);
					mesh.position.set(def.x, def.y, 0.5);
					mesh.visible = false; // only visible during FBO pass
					scene.add(mesh);
					backingPlanes.push({ mesh, el });
					count++;
					console.log(`[GlassScene] ✓ ${def.id}: ${pw.toFixed(1)}x${ph.toFixed(1)} at (${def.x}, ${def.y})`);
				} catch (e) {
					console.warn(`[GlassScene] ✗ ${def.id}:`, e);
				}
			}
			console.log(`[GlassScene] Captured ${count}/${backingDefs.length}. Scene children:`, scene.children.length);


		}, 2500);
	});

	let scrollY = 0;
	let smoothCamY = 0;
	let mouseX = 0, mouseY = 0, targetMX = 0, targetMY = 0;

	if (typeof window !== 'undefined') {
		window.addEventListener('scroll', () => { scrollY = window.scrollY; }, { passive: true });
		window.addEventListener('mousemove', (e) => {
			targetMX = (e.clientX / window.innerWidth - 0.5) * 2;
			targetMY = (e.clientY / window.innerHeight - 0.5) * 2;
		}, { passive: true });
	}

	useTask(() => {
		const t = performance.now() / 1000;

		// Resize FBO
		const w = Math.round(size.current.width * Math.min(devicePixelRatio, 2));
		const h = Math.round(size.current.height * Math.min(devicePixelRatio, 2));
		if (w > 0 && h > 0) {
			if (!fbo || fbo.width !== w || fbo.height !== h) {
				fbo?.dispose();
				fbo = new THREE.WebGLRenderTarget(w, h);
				refractionMat.uniforms.uSceneTex.value = fbo.texture;
			}
		}

		const maxScroll = typeof document !== 'undefined'
			? Math.max(1, document.body.scrollHeight - window.innerHeight) : 1;
		const scrollProgress = scrollY / maxScroll;
		const targetCamY = -scrollProgress * TOTAL_HEIGHT;
		smoothCamY += (targetCamY - smoothCamY) * 0.1;

		mouseX += (targetMX - mouseX) * 0.04;
		mouseY += (targetMY - mouseY) * 0.04;

		camera.current.position.set(mouseX * 0.3, smoothCamY - mouseY * 0.15, CAM_Z);
		camera.current.lookAt(0, smoothCamY, 0);

		const cy = smoothCamY;
		if (mainSphere) {
			// Follow cursor for testing
			const visH = 2 * CAM_Z * Math.tan((FOV / 2) * Math.PI / 180);
			const visW = visH * (window.innerWidth / window.innerHeight);
			mainSphere.position.set(mouseX * visW * 0.5, cy - mouseY * visH * 0.5, 2);
		}
		if (smallSphere) {
			smallSphere.position.set(-5 * Math.cos(t * 0.4 + 1) + mouseX * 0.3, cy - 1.5 + Math.sin(t * 0.5 + 1) * 1.5, 2.5 * Math.sin(t * 0.4 + 1));
		}
		if (tinySphere) {
			tinySphere.position.set(4.5 * Math.cos(t * 0.5 + 2) + mouseX * 0.2, cy + 2 + Math.sin(t * 0.6 + 2) * 1, 2 * Math.sin(t * 0.5 + 2));
		}
		if (starsRef) starsRef.rotation.y = t * 0.005 + mouseX * 0.02;

		// ── Two-pass render ──
		if (fbo && mainSphere && smallSphere && tinySphere) {
			// Pass 1: hide spheres, show backing planes → render to FBO
			mainSphere.visible = false;
			smallSphere.visible = false;
			tinySphere.visible = false;
			for (const bp of backingPlanes) bp.mesh.visible = true;

			renderer.setRenderTarget(fbo);
			renderer.clear();
			renderer.render(scene, camera.current);

			// Pass 2: show spheres, hide backing planes → render to screen
			mainSphere.visible = true;
			smallSphere.visible = true;
			tinySphere.visible = true;
			for (const bp of backingPlanes) bp.mesh.visible = false;

			renderer.setRenderTarget(null);
			renderer.clear();
			renderer.render(scene, camera.current);
		}
	});

	// ── Section Y positions ──
	const Y = {
		hero: 0,
		features: -16,
		demo: -32,
		how: -48,
		pricing: -64,
		cta: -78,
		footer: -88,
	};
</script>

<T.PerspectiveCamera makeDefault position={[0, 0, CAM_Z]} fov={FOV} near={0.1} far={100} />

<T.AmbientLight color={0x334466} intensity={0.8} />
<T.DirectionalLight color={0xffffff} intensity={2.0} position={[3, 2, 4]} />
<T.PointLight color={0x8899cc} intensity={1.0} position={[-3, 1.5, -2]} />
<T.PointLight color={0xffcc88} intensity={0.5} position={[2, -1, 3]} />

<T.Points bind:ref={starsRef} geometry={starGeo} material={starMat} />

<!-- ═══════════════════════════════════════
     HERO
     ═══════════════════════════════════════ -->

<HTML transform pointerEvents="none" position={[0, Y.hero + 4, 0]} scale={0.7}>
	<div style="display:inline-flex;align-items:center;gap:0.5rem;padding:0.375rem 1rem;border-radius:2rem;background:rgba(255,255,255,0.04);backdrop-filter:blur(20px);border:1px solid rgba(255,255,255,0.08);font-family:'Bricolage Grotesque',sans-serif;font-size:0.85rem;color:rgba(230,220,200,0.5);letter-spacing:0.05em;">
		<span style="width:5px;height:5px;border-radius:50%;background:#c4a265;box-shadow:0 0 8px rgba(196,162,101,0.4);"></span>
		now in beta
	</div>
</HTML>

<Text text="a friend that helps you" font={fraunces} fontSize={1.5} color={text} anchorX="center" anchorY="middle" position={[0, Y.hero + 2, 0]} textAlign="center" />
<Text text="think, work & feel" font={fraunces} fontSize={1.5} color={warm} anchorX="center" anchorY="middle" position={[0, Y.hero + 0.2, 0]} textAlign="center" />
<Text text="Not a chatbot. A presence that remembers your goals, notices your mood, helps you study, and checks in when you've been quiet too long." font={bricolage} fontSize={0.32} color={dim} anchorX="center" anchorY="top" position={[0, Y.hero - 1, 0]} maxWidth={11} textAlign="center" lineHeight={1.5} />

<HTML transform pointerEvents="auto" position={[0, Y.hero - 3.5, 0]} scale={0.7}>
	<a href="#pricing" style="display:inline-flex;align-items:center;gap:0.5rem;padding:0.75rem 1.75rem;border-radius:2rem;background:rgba(255,255,255,0.04);backdrop-filter:blur(20px);border:1px solid rgba(255,255,255,0.08);color:#c4a265;font-family:'Bricolage Grotesque',sans-serif;font-size:0.875rem;font-weight:500;text-decoration:none;">Meet yours →</a>
</HTML>

<!-- ═══════════════════════════════════════
     FEATURES
     ═══════════════════════════════════════ -->

<Text text="What it does" font={bricolage} fontSize={0.22} color={warm} anchorX="left" anchorY="middle" position={[-7, Y.features + 5, 0]} letterSpacing={0.15} />
<Text text="not another chatbot" font={fraunces} fontSize={1.0} color={text} anchorX="left" anchorY="middle" position={[-7, Y.features + 3.5, 0]} />
<Text text="Bolly is a friend that remembers, feels, and grows — one that actually helps you get through your day." font={bricolage} fontSize={0.25} color={dim} anchorX="left" anchorY="top" position={[-7, Y.features + 2.5, 0]} maxWidth={8} lineHeight={1.5} />

{#each features as f, i}
	{@const col = i % 3}
	{@const row = Math.floor(i / 3)}
	<HTML transform occlude="blending" pointerEvents="none" position={[-6 + col * 6, Y.features - row * 4.5, 0]} scale={0.6}>
		<div data-backing="feature-{i}" style="width:380px;height:160px;padding:2rem;border-radius:0;background:rgba(255,255,255,0.03);border:1px solid rgba(255,255,255,0.06);backdrop-filter:blur(12px);display:flex;flex-direction:column;justify-content:center;">
			<h3 style="font-family:'Fraunces',serif;font-style:italic;font-size:1.25rem;color:#e6dcc8;margin:0 0 0.75rem;">{f.title}</h3>
			<p style="font-family:'Bricolage Grotesque',sans-serif;font-size:0.9rem;color:#8a8070;line-height:1.6;margin:0;">{f.desc}</p>
		</div>
	</HTML>
{/each}

<!-- ═══════════════════════════════════════
     DEMO
     ═══════════════════════════════════════ -->

<Text text="How it feels" font={bricolage} fontSize={0.22} color={warm} anchorX="left" anchorY="middle" position={[2, Y.demo + 5, 0]} letterSpacing={0.15} />
<Text text="someone in your corner." font={fraunces} fontSize={0.85} color={text} anchorX="left" anchorY="middle" position={[2, Y.demo + 3.5, 0]} />
<Text text="not a search engine." font={fraunces} fontSize={0.85} color={dim} anchorX="left" anchorY="middle" position={[2, Y.demo + 2.3, 0]} />
<Text text="It doesn't just answer questions — it notices when you're overwhelmed and breaks things down. It remembers what you've been studying and what trips you up." font={bricolage} fontSize={0.24} color={dim} anchorX="left" anchorY="top" position={[2, Y.demo + 1.2, 0]} maxWidth={7} lineHeight={1.5} />

<!-- Demo video/chat placeholder — replace src with your video -->
<HTML transform occlude="blending" pointerEvents="none" position={[-4, Y.demo + 1, 0]} scale={0.7}>
	<div data-backing="demo" style="width:500px;height:380px;background:rgba(255,255,255,0.03);border:1px solid rgba(255,255,255,0.06);backdrop-filter:blur(12px);overflow:hidden;display:flex;align-items:center;justify-content:center;">
		<!-- Replace this div with <video> when ready -->
		<div style="width:100%;height:100%;display:flex;flex-direction:column;">
			<div style="display:flex;align-items:center;gap:0.5rem;padding:0.75rem 1rem;border-bottom:1px solid rgba(255,255,255,0.05);">
				<div style="width:28px;height:28px;border-radius:50%;background:rgba(196,162,101,0.1);border:1px solid rgba(196,162,101,0.15);display:flex;align-items:center;justify-content:center;font-family:'Fraunces',serif;font-style:italic;font-size:0.8rem;color:rgba(196,162,101,0.6);">b</div>
				<div><span style="font-size:0.85rem;color:#e6dcc8;">bolly</span><br/><span style="font-size:0.75rem;color:rgba(196,162,101,0.4);font-style:italic;">feeling curious</span></div>
			</div>
			<div style="padding:1.25rem;display:flex;flex-direction:column;gap:0.6rem;font-family:'Bricolage Grotesque',sans-serif;font-size:0.85rem;flex:1;">
				<div style="align-self:flex-end;background:rgba(196,162,101,0.08);border:1px solid rgba(196,162,101,0.12);padding:0.6rem 0.85rem;color:#e6dcc8;max-width:80%;">i have an exam on thursday and i haven't started studying. kind of freaking out</div>
				<div style="align-self:flex-start;background:rgba(255,255,255,0.03);border:1px solid rgba(255,255,255,0.06);padding:0.6rem 0.85rem;color:#8a8070;max-width:80%;">okay let's not panic. what's the subject and what topics does it cover?</div>
				<div style="align-self:flex-end;background:rgba(196,162,101,0.08);border:1px solid rgba(196,162,101,0.12);padding:0.6rem 0.85rem;color:#e6dcc8;max-width:80%;">organic chemistry. reactions, mechanisms, stereochemistry</div>
				<div style="align-self:flex-start;background:rgba(255,255,255,0.03);border:1px solid rgba(255,255,255,0.06);padding:0.6rem 0.85rem;color:#8a8070;max-width:80%;">three days is enough if we're smart about it. want to start with the easier topics or the hardest?</div>
			</div>
		</div>
	</div>
</HTML>

<!-- ═══════════════════════════════════════
     HOW IT WORKS
     ═══════════════════════════════════════ -->

<Text text="How it works" font={bricolage} fontSize={0.22} color={warm} anchorX="center" anchorY="middle" position={[0, Y.how + 5, 0]} letterSpacing={0.15} />
<Text text="three minutes to someone" font={fraunces} fontSize={0.95} color={text} anchorX="center" anchorY="middle" position={[0, Y.how + 3.5, 0]} textAlign="center" />
<Text text="who gets you" font={fraunces} fontSize={0.95} color={text} anchorX="center" anchorY="middle" position={[0, Y.how + 2.3, 0]} textAlign="center" />

{#each steps as step, i}
	<HTML transform occlude="blending" pointerEvents="none" position={[-5 + i * 5, Y.how - 0.5, 0]} scale={0.5}>
		<div data-backing="step-{i}" style="width:300px;padding:1.5rem;border-radius:0;background:rgba(255,255,255,0.03);border:1px solid rgba(255,255,255,0.06);backdrop-filter:blur(12px);">
			<div style="font-family:'Fraunces',serif;font-style:italic;font-size:2.5rem;color:rgba(255,255,255,0.05);line-height:1;margin-bottom:0.75rem;">{step.num}</div>
			<h3 style="font-family:'Fraunces',serif;font-style:italic;font-size:1.15rem;color:#e6dcc8;margin:0 0 0.5rem;">{step.title}</h3>
			<p style="font-family:'Bricolage Grotesque',sans-serif;font-size:0.8rem;color:#8a8070;line-height:1.5;margin:0;">{step.desc}</p>
		</div>
	</HTML>
{/each}

<!-- ═══════════════════════════════════════
     PRICING
     ═══════════════════════════════════════ -->

<Text text="Pricing" font={bricolage} fontSize={0.22} color={warm} anchorX="center" anchorY="middle" position={[0, Y.pricing + 5, 0]} letterSpacing={0.15} />
<Text text="simple, transparent" font={fraunces} fontSize={1.0} color={text} anchorX="center" anchorY="middle" position={[0, Y.pricing + 3.5, 0]} textAlign="center" />

{#each plans as plan, i}
	<HTML transform occlude="blending" pointerEvents="auto" position={[-5 + i * 5, Y.pricing - 0.5, 0]} scale={0.5}>
		<div data-backing="price-{i}" style="width:300px;padding:1.75rem;border-radius:0;background:rgba(255,255,255,{plan.featured ? '0.05' : '0.03'});border:1px solid {plan.featured ? 'rgba(196,162,101,0.15)' : 'rgba(255,255,255,0.06)'};backdrop-filter:blur(12px);position:relative;">
			{#if plan.featured}
				<div style="position:absolute;top:0.4rem;left:50%;transform:translateX(-50%);font-size:0.7rem;letter-spacing:0.1em;text-transform:uppercase;padding:0.2rem 0.7rem;border-radius:0;background:rgba(196,162,101,0.1);border:1px solid rgba(196,162,101,0.15);color:#c4a265;font-family:'Bricolage Grotesque',sans-serif;">popular</div>
			{/if}
			<div style="font-family:'Fraunces',serif;font-style:italic;font-size:1.2rem;color:#e6dcc8;">{plan.name}</div>
			<div style="font-size:0.7rem;color:#55504a;margin-bottom:1rem;font-family:'Bricolage Grotesque',sans-serif;">{plan.desc}</div>
			<div style="display:flex;align-items:baseline;gap:0.2rem;margin-bottom:0.25rem;">
				<span style="font-size:1rem;color:#8a8070;">$</span>
				<span style="font-family:'Fraunces',serif;font-style:italic;font-size:2.8rem;color:#e6dcc8;line-height:1;">{plan.price.replace('$','')}</span>
			</div>
			<div style="font-size:0.75rem;color:#55504a;margin-bottom:1.25rem;font-family:'Bricolage Grotesque',sans-serif;">per month</div>
			{#each plan.features as feat}
				<div style="font-size:0.75rem;color:#8a8070;margin-bottom:0.3rem;display:flex;align-items:center;gap:0.4rem;font-family:'Bricolage Grotesque',sans-serif;">
					<span style="width:3px;height:3px;border-radius:50%;background:rgba(255,255,255,0.15);"></span>{feat}
				</div>
			{/each}
			<a href="/signup" style="display:block;text-align:center;margin-top:1.25rem;padding:0.65rem;border-radius:0;font-size:0.75rem;font-family:'Bricolage Grotesque',sans-serif;text-decoration:none;background:{plan.featured ? 'rgba(196,162,101,0.1)' : 'rgba(255,255,255,0.03)'};border:1px solid {plan.featured ? 'rgba(196,162,101,0.18)' : 'rgba(255,255,255,0.06)'};color:{plan.featured ? '#c4a265' : '#8a8070'};">Get started</a>
		</div>
	</HTML>
{/each}

<!-- ═══════════════════════════════════════
     CTA
     ═══════════════════════════════════════ -->

<Text text="someone is ready to" font={fraunces} fontSize={1.1} color={text} anchorX="center" anchorY="middle" position={[0, Y.cta + 2.5, 0]} textAlign="center" />
<Text text="be in your corner" font={fraunces} fontSize={1.1} color={warm} anchorX="center" anchorY="middle" position={[0, Y.cta + 1, 0]} textAlign="center" />
<Text text="They'll remember what matters to you, notice how you're feeling, and be there at 3am when no one else is." font={bricolage} fontSize={0.26} color={dim} anchorX="center" anchorY="top" position={[0, Y.cta - 0.2, 0]} maxWidth={10} textAlign="center" lineHeight={1.5} />

<HTML transform pointerEvents="auto" position={[0, Y.cta - 2, 0]} scale={0.7}>
	<a href="#pricing" style="display:inline-flex;align-items:center;gap:0.5rem;padding:0.75rem 1.75rem;border-radius:2rem;background:rgba(255,255,255,0.04);backdrop-filter:blur(20px);border:1px solid rgba(255,255,255,0.08);color:#c4a265;font-family:'Bricolage Grotesque',sans-serif;font-size:0.875rem;font-weight:500;text-decoration:none;">Meet yours →</a>
</HTML>

<!-- ═══════════════════════════════════════
     FOOTER
     ═══════════════════════════════════════ -->

<Text text="© 2026 Bolly · Triangle Interactive" font={bricolage} fontSize={0.18} color={ghost} anchorX="center" anchorY="middle" position={[0, Y.footer, 0]} textAlign="center" />

<HTML transform pointerEvents="auto" position={[0, Y.footer - 1, 0]} scale={0.5}>
	<div style="display:flex;gap:1.5rem;font-family:'Bricolage Grotesque',sans-serif;font-size:0.75rem;">
		<a href="/privacy" style="color:#55504a;text-decoration:none;">Privacy</a>
		<a href="/terms" style="color:#55504a;text-decoration:none;">Terms</a>
		<a href="/docs" style="color:#55504a;text-decoration:none;">Docs</a>
	</div>
</HTML>

<!-- ═══ SPHERES ═══ -->
<T.Mesh bind:ref={mainSphere} geometry={mainGeo} material={refractionMat} />
<T.Mesh bind:ref={smallSphere} geometry={smallGeo} material={refractionMat} />
<T.Mesh bind:ref={tinySphere} geometry={tinyGeo} material={refractionMat} />
