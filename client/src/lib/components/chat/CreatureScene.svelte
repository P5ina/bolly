<script lang="ts">
	import { T, useTask } from "@threlte/core";
	import { IcosahedronGeometry } from "three";
	import type { Mesh } from "three";

	let { thinking = false, mood = "calm" }: { thinking?: boolean; mood?: string } = $props();

	let meshRef = $state<Mesh | undefined>();
	let time = $state(0);

	// Mood-based color mapping
	const moodColors: Record<string, string> = {
		calm: "#8ab4f8",
		curious: "#a8d8ea",
		excited: "#f8c471",
		warm: "#f0b27a",
		reflective: "#bb8fce",
		contemplative: "#a993c7",
		worried: "#85929e",
		playful: "#82e0aa",
	};

	const baseColor = $derived(moodColors[mood] ?? moodColors.calm);

	// Cache base geometry positions
	const baseGeo = new IcosahedronGeometry(1, 4);
	const basePositions = baseGeo.getAttribute("position").array.slice();
	baseGeo.dispose();

	// Animate the blob
	useTask((delta) => {
		time += delta;

		if (!meshRef) return;

		const geo = meshRef.geometry;
		const pos = geo.getAttribute("position");

		const speed = thinking ? 3.0 : 0.8;
		const intensity = thinking ? 0.25 : 0.12;
		const breathe = 1.0 + Math.sin(time * (thinking ? 2.5 : 1.2)) * 0.04;

		for (let i = 0; i < pos.count; i++) {
			const bx = basePositions[i * 3];
			const by = basePositions[i * 3 + 1];
			const bz = basePositions[i * 3 + 2];

			// Organic noise displacement
			const noise =
				Math.sin(bx * 2.1 + time * speed * 0.7) *
				Math.cos(by * 1.8 + time * speed * 0.5) *
				Math.sin(bz * 2.5 + time * speed * 0.9);

			const scale = breathe + noise * intensity;

			pos.setXYZ(i, bx * scale, by * scale, bz * scale);
		}

		pos.needsUpdate = true;
		geo.computeVertexNormals();

		// Gentle rotation
		meshRef.rotation.y += delta * (thinking ? 0.4 : 0.15);
		meshRef.rotation.x = Math.sin(time * 0.3) * 0.1;
	});
</script>

<!-- Camera -->
<T.PerspectiveCamera makeDefault position={[0, 0, 3.2]} fov={45} />

<!-- Lighting -->
<T.AmbientLight intensity={0.3} color="#b8c9e8" />
<T.PointLight
	position={[2, 2, 3]}
	intensity={thinking ? 2.5 : 1.5}
	color={baseColor}
	castShadow={false}
/>
<T.PointLight position={[-2, -1, 2]} intensity={0.6} color="#4a6fa5" />
<T.PointLight position={[0, 3, 0]} intensity={thinking ? 1.2 : 0.4} color="#e8d5b7" />

<!-- The creature -->
<T.Mesh bind:ref={meshRef}>
	<T.IcosahedronGeometry args={[1, 4]} />
	<T.MeshStandardMaterial
		color={baseColor}
		emissive={baseColor}
		emissiveIntensity={thinking ? 0.35 : 0.15}
		roughness={0.7}
		metalness={0.1}
		wireframe={false}
		transparent
		opacity={0.85}
	/>
</T.Mesh>

<!-- Inner glow core -->
<T.Mesh>
	<T.IcosahedronGeometry args={[0.5, 3]} />
	<T.MeshStandardMaterial
		color="#ffffff"
		emissive="#ffffff"
		emissiveIntensity={thinking ? 0.8 : 0.3}
		transparent
		opacity={thinking ? 0.25 : 0.12}
	/>
</T.Mesh>
