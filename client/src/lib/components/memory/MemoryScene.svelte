<script lang="ts">
	import { T, useTask, useThrelte } from "@threlte/core";
	import { Environment } from "@threlte/extras";
	import type { Mesh } from "three";
	import { MeshPhysicalMaterial, Color, PMREMGenerator, Scene, PointLight } from "three";

	interface SphereData {
		id: string;
		x: number;
		y: number;
		r: number;
		hex: string;
		hovered: boolean;
	}

	let { spheres = [], width = 600, height = 500, panX = 0, panY = 0, zoom = 1 }: {
		spheres?: SphereData[];
		width?: number;
		height?: number;
		panX?: number;
		panY?: number;
		zoom?: number;
	} = $props();

	let time = 0;
	let meshRefs: Record<string, Mesh> = {};

	// Camera params: orthographic mapped to pixel space
	const frustumScale = $derived(Math.max(width, height) / 2);

	useTask((delta) => {
		time += delta;
		for (let i = 0; i < spheres.length; i++) {
			const s = spheres[i];
			const mesh = meshRefs[s.id];
			if (!mesh) continue;
			mesh.rotation.y = time * 0.15 + i * 0.7;
			mesh.rotation.x = Math.sin(time * 0.2 + i * 0.5) * 0.1;
			// Gentle float
			mesh.position.y = -(s.y - height / 2) + Math.sin(time * 0.5 + i * 1.3) * 3;
		}
	});

	// Generate envmap on mount
	const { renderer } = useThrelte();
	let envTexture = $state<any>(null);

	$effect(() => {
		if (!renderer) return;
		const pmrem = new PMREMGenerator(renderer);
		const envScene = new Scene();
		envScene.background = new Color(0x0a1025);
		const light1 = new PointLight(0x4466aa, 5, 200);
		light1.position.set(50, 50, 50);
		envScene.add(light1);
		const light2 = new PointLight(0x6644aa, 3, 200);
		light2.position.set(-50, -30, -50);
		envScene.add(light2);
		const rt = pmrem.fromScene(envScene);
		envTexture = rt.texture;
		return () => { pmrem.dispose(); rt.dispose(); };
	});
</script>

<T.OrthographicCamera
	makeDefault
	position.x={(panX)}
	position.y={-(panY)}
	position.z={100}
	zoom={zoom}
	left={-width / 2}
	right={width / 2}
	top={height / 2}
	bottom={-height / 2}
	near={-500}
	far={500}
/>

<T.AmbientLight intensity={0.4} color="#334477" />
<T.DirectionalLight position={[200, -100, 300]} intensity={2.0} />
<T.PointLight position={[-200, 100, -200]} intensity={1.5} color="#8899cc" />

{#if envTexture}
	<T.Scene environment={envTexture} />
{/if}

{#each spheres as sphere (sphere.id)}
	<T.Mesh
		bind:ref={meshRefs[sphere.id]}
		position.x={sphere.x - width / 2}
		position.y={-(sphere.y - height / 2)}
		position.z={0}
		scale={sphere.r}
	>
		<T.IcosahedronGeometry args={[1, 4]} />
		<T.MeshPhysicalMaterial
			color="#ffffff"
			transmission={0.95}
			ior={1.3}
			thickness={0.8}
			roughness={0.05}
			metalness={0}
			clearcoat={0.3}
			clearcoatRoughness={0.05}
			specularIntensity={1.0}
			specularColor={new Color(0xffffff)}
			envMapIntensity={15}
			transparent
			envMap={envTexture}
		/>
	</T.Mesh>
{/each}
