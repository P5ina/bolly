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
		happy: "#f7dc6f",
		joyful: "#f9e154",
		reflective: "#bb8fce",
		contemplative: "#a993c7",
		melancholy: "#7f8c9a",
		sad: "#6b7b8d",
		worried: "#85929e",
		anxious: "#95a0ab",
		playful: "#82e0aa",
		mischievous: "#58d68d",
		focused: "#76d7c4",
		tired: "#a0937d",
		peaceful: "#aed6f1",
		loving: "#f1948a",
		tender: "#f5b7b1",
		creative: "#d2b4de",
		energetic: "#fad7a0",
	};

	// Fuzzy mood matcher — handles "a bit sad", "excited and curious", etc.
	function matchMood(raw: string): string {
		const m = raw.toLowerCase();
		// Exact match first
		if (moodColors[m]) return m;
		// Check if any known mood is contained in the string (longest match wins)
		const keys = Object.keys(moodColors).sort((a, b) => b.length - a.length);
		for (const key of keys) {
			if (m.includes(key)) return key;
		}
		return "calm";
	}

	const resolvedMood = $derived(matchMood(mood));
	const baseColor = $derived(moodColors[resolvedMood]);

	// Mood-based animation energy
	type MoodEnergy = { speed: number; intensity: number; breatheRate: number; breatheDepth: number; rotSpeed: number };
	const moodEnergies: Record<string, MoodEnergy> = {
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
	const defaultEnergy: MoodEnergy = { speed: 0.8, intensity: 0.12, breatheRate: 1.2, breatheDepth: 0.04, rotSpeed: 0.15 };
	const energy = $derived(moodEnergies[resolvedMood] ?? defaultEnergy);

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

		const speed = thinking ? 3.0 : energy.speed;
		const intensity = thinking ? 0.25 : energy.intensity;
		const breathe = 1.0 + Math.sin(time * (thinking ? 2.5 : energy.breatheRate)) * (thinking ? 0.06 : energy.breatheDepth);

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
		meshRef.rotation.y += delta * (thinking ? 0.4 : energy.rotSpeed);
		meshRef.rotation.x = Math.sin(time * 0.3) * 0.1;
	});
</script>

<!-- Camera -->
<T.PerspectiveCamera makeDefault position={[0, 0, 3.2]} fov={45} />

<!-- Lighting — neutral base so mood color dominates -->
<T.AmbientLight intensity={0.15} color="#e0e0e0" />
<T.PointLight
	position={[2, 2, 3]}
	intensity={thinking ? 2.5 : 1.8}
	color={baseColor}
	castShadow={false}
/>
<T.PointLight position={[-2, -1, 2]} intensity={0.3} color={baseColor} />
<T.PointLight position={[0, 3, 0]} intensity={thinking ? 0.8 : 0.3} color="#e0e0e0" />

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
