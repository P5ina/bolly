<script lang="ts">
	import { T, useTask, useThrelte } from '@threlte/core';
	import { HTML, Text } from '@threlte/extras';
	import * as THREE from 'three';
	import type { Snippet } from 'svelte';

	interface Section {
		snippet: Snippet;
		y: number;
		width?: number;
	}

	let { sections }: { sections: Section[] } = $props();

	const { scene, camera, renderer } = useThrelte();

	scene.background = new THREE.Color(0x000206);

	const CAM_Z = 14;
	const FOV = 50;

	// ── Starfield background ──
	const STAR_COUNT = 800;
	const starPos = new Float32Array(STAR_COUNT * 3);
	const starCol = new Float32Array(STAR_COUNT * 3);
	for (let i = 0; i < STAR_COUNT; i++) {
		const theta = Math.random() * Math.PI * 2;
		const phi = Math.acos(2 * Math.random() - 1);
		const r = 20 + Math.random() * 30;
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

	// ── Sphere material ──
	const sphereMat = new THREE.MeshStandardMaterial({
		color: 0xccddee,
		roughness: 0.1,
		metalness: 0.3,
	});

	const mainGeo = new THREE.SphereGeometry(1, 32, 32);
	const smallGeo = new THREE.SphereGeometry(0.5, 24, 24);
	const tinyGeo = new THREE.SphereGeometry(0.3, 16, 16);

	let mainSphere: THREE.Mesh | undefined;
	let smallSphere: THREE.Mesh | undefined;
	let tinySphere: THREE.Mesh | undefined;
	let starsRef: THREE.Points | undefined;

	// ── Font URLs (Google Fonts) ──
	const fraunces = 'https://fonts.gstatic.com/s/fraunces/v38/6NVf8FyLNQOQZAnv9ZwNjucMHVn85Ni7emAe9lKqZTnbB-gzTK0K1ChJdt9vIVYX9G37lod9sPEKsxx664UJf1hLTf7W.ttf';
	const bricolage = 'https://fonts.gstatic.com/s/bricolagegrotesque/v9/3y9U6as8bTXq_nANBjzKo3IeZx8z6up5BeSl5jBNz_19PpbJMXuECpwUxJBOm_OJWiaaD30YfKfjZZoLvRviyM0.ttf';

	// ── Scroll & mouse ──
	let scrollY = 0;
	let smoothCamY = 0;
	let mouseX = 0, mouseY = 0, targetMX = 0, targetMY = 0;

	const lastSection = sections[sections.length - 1];
	const totalHeight = lastSection ? Math.abs(lastSection.y) + 5 : 20;

	if (typeof window !== 'undefined') {
		window.addEventListener('scroll', () => { scrollY = window.scrollY; }, { passive: true });
		window.addEventListener('mousemove', (e) => {
			targetMX = (e.clientX / window.innerWidth - 0.5) * 2;
			targetMY = (e.clientY / window.innerHeight - 0.5) * 2;
		}, { passive: true });
	}

	useTask(() => {
		const t = performance.now() / 1000;

		const maxScroll = typeof document !== 'undefined'
			? Math.max(1, document.body.scrollHeight - window.innerHeight) : 1;
		const scrollProgress = scrollY / maxScroll;
		const targetCamY = -scrollProgress * totalHeight;
		smoothCamY += (targetCamY - smoothCamY) * 0.1;

		mouseX += (targetMX - mouseX) * 0.04;
		mouseY += (targetMY - mouseY) * 0.04;

		camera.current.position.set(mouseX * 0.3, smoothCamY - mouseY * 0.15, CAM_Z);
		camera.current.lookAt(0, smoothCamY, 0);

		const cy = smoothCamY;

		if (mainSphere) {
			mainSphere.position.set(
				6 * Math.cos(t * 0.3) + mouseX * 0.5,
				cy + Math.sin(t * 0.4) * 2,
				3 * Math.sin(t * 0.3)
			);
		}
		if (smallSphere) {
			smallSphere.position.set(
				-5 * Math.cos(t * 0.4 + 1) + mouseX * 0.3,
				cy - 1.5 + Math.sin(t * 0.5 + 1) * 1.5,
				2.5 * Math.sin(t * 0.4 + 1)
			);
		}
		if (tinySphere) {
			tinySphere.position.set(
				4.5 * Math.cos(t * 0.5 + 2) + mouseX * 0.2,
				cy + 2 + Math.sin(t * 0.6 + 2) * 1,
				2 * Math.sin(t * 0.5 + 2)
			);
		}

		if (starsRef) {
			starsRef.rotation.y = t * 0.005 + mouseX * 0.02;
		}
	});
</script>

<T.PerspectiveCamera makeDefault position={[0, 0, CAM_Z]} fov={FOV} near={0.1} far={100} />

<!-- Lighting -->
<T.AmbientLight color={0x334466} intensity={0.8} />
<T.DirectionalLight color={0xffffff} intensity={2.0} position={[3, 2, 4]} />
<T.PointLight color={0x8899cc} intensity={1.0} position={[-3, 1.5, -2]} />
<T.PointLight color={0xffcc88} intensity={0.5} position={[2, -1, 3]} />

<!-- Starfield -->
<T.Points bind:ref={starsRef} geometry={starGeo} material={starMat} />

<!-- ═══ HERO (3D native) ═══ -->

<!-- Badge -->
<HTML transform pointerEvents="none" position={[0, 2.5, 0]} scale={0.5}>
	<div style="
		display: inline-flex; align-items: center; gap: 0.5rem;
		padding: 0.375rem 1rem; border-radius: 2rem;
		background: rgba(255,255,255,0.04); backdrop-filter: blur(20px);
		border: 1px solid rgba(255,255,255,0.08);
		font-family: 'Bricolage Grotesque', sans-serif;
		font-size: 0.85rem; color: rgba(230,220,200,0.5);
		letter-spacing: 0.05em;
	">
		<span style="width:5px;height:5px;border-radius:50%;background:#c4a265;box-shadow:0 0 8px rgba(196,162,101,0.4);"></span>
		now in beta
	</div>
</HTML>

<!-- Title line 1 -->
<Text
	text="a friend that helps you"
	font={fraunces}
	fontSize={0.9}
	color="#e6dcc8"
	anchorX="center"
	anchorY="middle"
	position={[0, 1.2, 0]}
	textAlign="center"
/>

<!-- Title line 2 (warm accent) -->
<Text
	text="think, work & feel"
	font={fraunces}
	fontSize={0.9}
	color="#c4a265"
	anchorX="center"
	anchorY="middle"
	position={[0, 0.2, 0]}
	textAlign="center"
/>

<!-- Subtitle -->
<Text
	text="Not a chatbot. A presence that remembers your goals, notices your mood, helps you study, and checks in when you've been quiet too long."
	font={bricolage}
	fontSize={0.22}
	color="#8a8070"
	anchorX="center"
	anchorY="top"
	position={[0, -0.5, 0]}
	maxWidth={8}
	textAlign="center"
	lineHeight={1.5}
/>

<!-- CTA button -->
<HTML transform pointerEvents="auto" position={[0, -2, 0]} scale={0.5}>
	<a href="#pricing" style="
		display: inline-flex; align-items: center; gap: 0.5rem;
		padding: 0.75rem 1.75rem; border-radius: 2rem;
		background: rgba(255,255,255,0.04); backdrop-filter: blur(20px);
		border: 1px solid rgba(255,255,255,0.08);
		color: #c4a265; font-family: 'Bricolage Grotesque', sans-serif;
		font-size: 0.875rem; font-weight: 500; text-decoration: none;
		transition: all 0.3s ease;
	">
		Meet yours →
	</a>
</HTML>

<!-- ═══ PAGE SECTIONS (HTML in 3D) ═══ -->
{#each sections as section}
	<HTML
		transform
		occlude="blending"
		pointerEvents="auto"
		position.y={section.y}
		distanceFactor={8}
	>
		<div style="width: {section.width ?? 1100}px; pointer-events: auto;">
			{@render section.snippet()}
		</div>
	</HTML>
{/each}

<!-- Spheres -->
<T.Mesh bind:ref={mainSphere} geometry={mainGeo} material={sphereMat} />
<T.Mesh bind:ref={smallSphere} geometry={smallGeo} material={sphereMat} />
<T.Mesh bind:ref={tinySphere} geometry={tinyGeo} material={sphereMat} />
