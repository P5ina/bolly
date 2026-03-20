<script lang="ts">
	import { T, useTask, useThrelte } from '@threlte/core';
	import {
		MeshBasicNodeMaterial,
		SphereGeometry,
		BufferGeometry,
		BufferAttribute,
		PointsMaterial,
		AdditiveBlending,
		BackSide,
		PMREMGenerator,
	} from 'three/webgpu';
	import type { WebGPURenderer } from 'three/webgpu';
	import {
		Fn, float, vec3, vec4,
		uv, time, screenUV,
		sin, pow,
	} from 'three/tsl';

	const { scene, renderer, camera, invalidate } = useThrelte();

	// ── Skybox for envmap reflections ──
	const skyGeo = new SphereGeometry(30, 64, 32);
	const skyMat = new MeshBasicNodeMaterial({ side: BackSide });
	const skyShader = Fn(() => {
		const st = uv();
		const t = time.mul(0.012);
		const E = float(2.718);
		const c = vec3(0.003, 0.005, 0.018).toVar();
		c.addAssign(vec3(0.002, 0.003, 0.010).mul(st.y));
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

	// Flat background
	const flatBg = Fn(() => {
		const st = screenUV;
		const c = vec3(0.002, 0.003, 0.012).toVar();
		c.addAssign(vec3(0.001, 0.002, 0.006).mul(st.y));
		return c;
	});
	scene.backgroundNode = flatBg();

	// ── Stars ──
	const STAR_COUNT = 1200;
	const starPositions = new Float32Array(STAR_COUNT * 3);
	const starSizes = new Float32Array(STAR_COUNT);
	const starColors = new Float32Array(STAR_COUNT * 3);

	for (let i = 0; i < STAR_COUNT; i++) {
		const theta = Math.random() * Math.PI * 2;
		const phi = Math.acos(2 * Math.random() - 1);
		const r = 8 + Math.random() * 17;
		starPositions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
		starPositions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
		starPositions[i * 3 + 2] = r * Math.cos(phi);
		const rnd = Math.random();
		starSizes[i] = rnd < 0.95 ? 1.0 + Math.random() * 2.0 : 3.0 + Math.random() * 4.0;
		const warmth = Math.random();
		starColors[i * 3] = 0.6 + warmth * 0.4;
		starColors[i * 3 + 1] = 0.7 + warmth * 0.2;
		starColors[i * 3 + 2] = 1.0 - warmth * 0.3;
	}

	const starGeo = new BufferGeometry();
	starGeo.setAttribute('position', new BufferAttribute(starPositions, 3));
	starGeo.setAttribute('size', new BufferAttribute(starSizes, 1));
	starGeo.setAttribute('color', new BufferAttribute(starColors, 3));

	const starMat = new PointsMaterial({
		size: 0.06,
		sizeAttenuation: true,
		vertexColors: true,
		transparent: true,
		opacity: 0.85,
		depthWrite: false,
		blending: AdditiveBlending,
	});

	// ── Envmap ──
	let envDone = false;
	const pmrem = new PMREMGenerator(renderer as unknown as InstanceType<typeof WebGPURenderer>);

	// ── Mouse parallax ──
	let mouseX = 0;
	let mouseY = 0;
	let targetX = 0;
	let targetY = 0;

	if (typeof window !== 'undefined') {
		window.addEventListener('mousemove', (e: MouseEvent) => {
			targetX = (e.clientX / window.innerWidth - 0.5) * 2;
			targetY = (e.clientY / window.innerHeight - 0.5) * 2;
		}, { passive: true });
	}

	let starsRef: any;
	let skyboxRef: any;

	useTask(() => {
		const t = performance.now() / 1000;

		if (!envDone && t > 2 && skyboxRef) {
			skyboxRef.visible = true;
			const envTarget = pmrem.fromScene(scene, 0.04);
			scene.environment = envTarget.texture;
			skyboxRef.visible = false;
			envDone = true;
		}

		mouseX += (targetX - mouseX) * 0.03;
		mouseY += (targetY - mouseY) * 0.03;

		if (starsRef) {
			starsRef.rotation.y = t * 0.008 + mouseX * 0.05;
			starsRef.rotation.x = mouseY * 0.03;
		}

		camera.current.position.x = mouseX * 0.15;
		camera.current.position.y = -mouseY * 0.1;
		camera.current.lookAt(0, 0, 0);

		invalidate();
	});
</script>

<T.PerspectiveCamera makeDefault position={[0, 0, 5]} fov={50} />

<T.Mesh
	bind:ref={skyboxRef}
	geometry={skyGeo}
	material={skyMat}
	visible={false}
/>

<T.Points
	bind:ref={starsRef}
	geometry={starGeo}
	material={starMat}
/>

<T.AmbientLight color={0x334477} intensity={0.3} />
<T.DirectionalLight color={0xffffff} intensity={2.0} position={[3, 2, 4]} />
<T.PointLight color={0x8899cc} intensity={1.0} position={[-3, 1.5, -2]} />
<T.PointLight color={0x6677aa} intensity={0.4} position={[0, -3, 1]} />
