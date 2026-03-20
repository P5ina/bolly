<script lang="ts">
	import { onMount } from 'svelte';
	import { T, useTask, useThrelte } from '@threlte/core';
	import { HTML, Text } from '@threlte/extras';
	import * as THREE from 'three';

	const { scene, camera, renderer, size } = useThrelte();

	// ── HTML → Texture renderer (ported from three-html-render) ──
	class HtmlRenderer {
		private canvas = document.createElement('canvas');
		private ctx = this.canvas.getContext('2d')!;
		private textures = new Map<HTMLElement, THREE.CanvasTexture>();
		private pageStyles = '';

		setPageStyles(styles: string) { this.pageStyles = styles; }

		private svgToDataUrl(svg: string): string {
			return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
		}

		async update(node: HTMLElement): Promise<THREE.CanvasTexture> {
			const w = node.clientWidth;
			const h = node.clientHeight;

			const html = node.innerHTML;

			const svg = `<svg viewBox="0 0 ${w} ${h}" xmlns="http://www.w3.org/2000/svg">
	<style>${this.pageStyles}</style>
	<foreignObject x="0" y="0" width="100%" height="100%">
		<div xmlns="http://www.w3.org/1999/xhtml" style="height:100%;width:100%;">
			${html}
		</div>
	</foreignObject>
</svg>`;

			// Data URL — does NOT taint canvas (unlike Blob URL)
			const dataUrl = this.svgToDataUrl(svg);

			const img = await new Promise<HTMLImageElement>((resolve, reject) => {
				const i = new Image();
				i.onload = () => resolve(i);
				i.onerror = (e) => reject(e);
				i.crossOrigin = 'anonymous';
				i.decoding = 'sync';
				i.src = dataUrl;
			});

			let tex = this.textures.get(node);
			if (!tex || this.canvas.width !== w || this.canvas.height !== h) {
				tex?.dispose();
				this.canvas.width = w;
				this.canvas.height = h;
				tex = new THREE.CanvasTexture(this.canvas);
				tex.colorSpace = THREE.SRGBColorSpace;
				this.textures.set(node, tex);
			}

			this.ctx.clearRect(0, 0, w, h);
			this.ctx.drawImage(img, 0, 0, w, h);
			tex.needsUpdate = true;
			return tex;
		}
	}

	const htmlRenderer = new HtmlRenderer();

	// Backing planes — HTML rendered as textures for refraction FBO
	const backingPlanes: { mesh: THREE.Mesh; el: HTMLElement; position: THREE.Vector3 }[] = [];

	// Register an HTML element for texture backing
	function registerHtmlBacking(el: HTMLElement, x: number, y: number, z: number, scaleVal: number) {
		const mesh = new THREE.Mesh(
			new THREE.PlaneGeometry(1, 1),
			new THREE.MeshBasicMaterial({ transparent: true, side: THREE.DoubleSide })
		);
		mesh.position.set(x, y, z);
		mesh.visible = false; // only shown during FBO pass
		scene.add(mesh);
		backingPlanes.push({ mesh, el, position: new THREE.Vector3(x, y, z) });

		// Initial capture after a delay
		setTimeout(async () => {
			try {
				const tex = await htmlRenderer.update(el);
				(mesh.material as THREE.MeshBasicMaterial).map = tex;
				(mesh.material as THREE.MeshBasicMaterial).needsUpdate = true;

				// Scale plane to match HTML size in 3D
				const pxToUnit = (CAM_Z * 2 * Math.tan((FOV / 2) * Math.PI / 180)) / window.innerHeight;
				mesh.scale.set(el.clientWidth * pxToUnit * scaleVal, el.clientHeight * pxToUnit * scaleVal, 1);
			} catch (e) {
				console.warn('HTML backing capture failed:', e);
			}
		}, 2000);
	}

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
				vec2 offset = refr.xy * 0.15;

				// Chromatic aberration
				float r = texture2D(uSceneTex, vScreenUV + offset * (1.0 + uChroma)).r;
				float g = texture2D(uSceneTex, vScreenUV + offset).g;
				float b = texture2D(uSceneTex, vScreenUV + offset * (1.0 - uChroma)).b;
				vec3 refracted = vec3(r, g, b);

				// Fresnel
				float fresnel = pow(1.0 + dot(V, N), uFresnelPow);

				// Specular
				vec3 refl = reflect(V, N);
				float spec = pow(max(dot(refl, normalize(vec3(3, 2, 4))), 0.0), 80.0) * 0.6;
				float spec2 = pow(max(dot(refl, normalize(vec3(-2, 1, -1))), 0.0), 40.0) * 0.2;

				// Mix
				vec3 col = mix(refracted, vec3(0.1, 0.13, 0.22), fresnel * 0.35);
				col += (spec + spec2);
				col += fresnel * vec3(0.05, 0.08, 0.14) * 0.5;

				gl_FragColor = vec4(col, 1.0);
			}
		`,
		uniforms: {
			uSceneTex: { value: null },
			uIOR: { value: 1.45 },
			uChroma: { value: 0.12 },
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
			// Gather all CSS styles for the foreignObject
			let styles = '';
			for (const sheet of document.styleSheets) {
				try { for (const rule of sheet.cssRules) styles += rule.cssText + '\n'; } catch {}
			}
			htmlRenderer.setPageStyles(styles);

			// Find all HTML wrappers created by <HTML transform>
			const canvasParent = renderer.domElement.parentElement;
			if (!canvasParent) return;

			// Threlte HTML components are siblings of the canvas
			const allDivs = canvasParent.querySelectorAll<HTMLElement>('div');
			const wrappers: HTMLElement[] = [];
			allDivs.forEach(d => {
				if (d.style.transform && d.style.transform.includes('matrix3d')) wrappers.push(d);
			});
			console.log(`[GlassScene] Found ${wrappers.length} matrix3d wrappers`);

			let count = 0;

			for (const wrapper of wrappers) {
				const content = wrapper.children[0] as HTMLElement;
				if (!content || content.clientWidth === 0) {
					console.log('[GlassScene] Skipping wrapper — no content or zero width');
					continue;
				}

				// Parse position from matrix3d
				const m = wrapper.style.transform.match(/matrix3d\(([^)]+)\)/);
				if (!m) { console.log('[GlassScene] No matrix3d match'); continue; }
				const vals = m[1].split(',').map(Number);
				const x = vals[12], y = vals[13], z = vals[14];
				console.log(`[GlassScene] Wrapper at (${x.toFixed(1)}, ${y.toFixed(1)}, ${z.toFixed(1)}) size: ${content.clientWidth}x${content.clientHeight}`);

				// Parse scale from the wrapper
				const scaleMatch = wrapper.style.transform.match(/scale\(([^)]+)\)/);
				const scaleVal = scaleMatch ? parseFloat(scaleMatch[1]) : 1;

				try {
					const tex = await htmlRenderer.update(content);
					const pxToUnit = (CAM_Z * 2 * Math.tan((FOV / 2) * Math.PI / 180)) / window.innerHeight;
					const pw = content.clientWidth * pxToUnit * scaleVal;
					const ph = content.clientHeight * pxToUnit * scaleVal;

					const geo = new THREE.PlaneGeometry(pw, ph);
					const mat = new THREE.MeshBasicMaterial({ map: tex, transparent: true, side: THREE.DoubleSide });
					const mesh = new THREE.Mesh(geo, mat);
					mesh.position.set(x, y, z);
					mesh.visible = false;
					scene.add(mesh);
					backingPlanes.push({ mesh, el: content, position: new THREE.Vector3(x, y, z) });
					count++;
				} catch (e) {
					console.warn('Backing capture failed:', e);
				}
			}

			console.log(`[GlassScene] Captured ${count} HTML backing planes`);
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
			mainSphere.position.set(6 * Math.cos(t * 0.3) + mouseX * 0.5, cy + Math.sin(t * 0.4) * 2, 3 * Math.sin(t * 0.3));
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
		<div style="width:380px;height:160px;padding:2rem;border-radius:0;background:rgba(255,255,255,0.03);border:1px solid rgba(255,255,255,0.06);backdrop-filter:blur(12px);display:flex;flex-direction:column;justify-content:center;">
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
	<div style="width:500px;height:380px;background:rgba(255,255,255,0.03);border:1px solid rgba(255,255,255,0.06);backdrop-filter:blur(12px);overflow:hidden;display:flex;align-items:center;justify-content:center;">
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
		<div style="width:300px;padding:1.5rem;border-radius:0;background:rgba(255,255,255,0.03);border:1px solid rgba(255,255,255,0.06);backdrop-filter:blur(12px);">
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
		<div style="width:300px;padding:1.75rem;border-radius:0;background:rgba(255,255,255,{plan.featured ? '0.05' : '0.03'});border:1px solid {plan.featured ? 'rgba(196,162,101,0.15)' : 'rgba(255,255,255,0.06)'};backdrop-filter:blur(12px);position:relative;">
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
