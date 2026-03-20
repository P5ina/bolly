<script lang="ts">
	import { T, useTask, useThrelte } from '@threlte/core';
	import { HTML } from '@threlte/extras';
	import * as THREE from 'three';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();

	const { scene, camera } = useThrelte();

	scene.background = new THREE.Color(0x000206);

	const CAM_Z = 14;
	const FOV = 50;

	// Plane sized to fill ~80% of camera view
	const screenH = 2 * CAM_Z * Math.tan((FOV / 2) * Math.PI / 180) * 0.75;
	const aspect = typeof window !== 'undefined' ? window.innerWidth / window.innerHeight : 16 / 9;
	const screenW = screenH * aspect;

	// Opaque material — must write to depth buffer for occlude to work
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

	let mouseX = 0, mouseY = 0, targetMX = 0, targetMY = 0;

	if (typeof window !== 'undefined') {
		window.addEventListener('mousemove', (e) => {
			targetMX = (e.clientX / window.innerWidth - 0.5) * 2;
			targetMY = (e.clientY / window.innerHeight - 0.5) * 2;
		}, { passive: true });
	}

	useTask(() => {
		const t = performance.now() / 1000;

		mouseX += (targetMX - mouseX) * 0.04;
		mouseY += (targetMY - mouseY) * 0.04;

			// Spheres orbit around the plane
		if (mainSphere) {
			mainSphere.position.set(
				6 * Math.cos(t * 0.3) + mouseX * 0.5,
				Math.sin(t * 0.4) * 2 + mouseY * -0.3,
				3 * Math.sin(t * 0.3)
			);
		}

		if (smallSphere) {
			smallSphere.position.set(
				-5 * Math.cos(t * 0.4 + 1) + mouseX * 0.3,
				-1.5 + Math.sin(t * 0.5 + 1) * 1.5,
				2.5 * Math.sin(t * 0.4 + 1)
			);
		}

		if (tinySphere) {
			tinySphere.position.set(
				4.5 * Math.cos(t * 0.5 + 2) + mouseX * 0.2,
				2 + Math.sin(t * 0.6 + 2) * 1,
				2 * Math.sin(t * 0.5 + 2)
			);
		}

		camera.current.position.set(mouseX * 0.3, -mouseY * 0.15, CAM_Z);
		camera.current.lookAt(0, 0, 0);
	});
</script>

<T.PerspectiveCamera makeDefault position={[0, 0, CAM_Z]} fov={FOV} near={0.1} far={100} />

<T.AmbientLight color={0x334466} intensity={0.8} />
<T.DirectionalLight color={0xffffff} intensity={2.0} position={[3, 2, 4]} />
<T.PointLight color={0x8899cc} intensity={1.0} position={[-3, 1.5, -2]} />

<!-- Visible plane at z=0 — where HTML will go -->
<T.Mesh position={[0, 0, 0]}>
	<T.PlaneGeometry args={[screenW, screenH]} />
	<T.MeshStandardMaterial color="#111122" roughness={0.9} metalness={0.0} transparent opacity={0.8} />
</T.Mesh>

<!-- Spheres -->
<T.Mesh bind:ref={mainSphere} geometry={mainGeo} material={sphereMat} />
<T.Mesh bind:ref={smallSphere} geometry={smallGeo} material={sphereMat} />
<T.Mesh bind:ref={tinySphere} geometry={tinyGeo} material={sphereMat} />
