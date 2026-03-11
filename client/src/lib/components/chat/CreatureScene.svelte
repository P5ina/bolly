<script lang="ts">
	import { T, useTask } from "@threlte/core";
	import { IcosahedronGeometry, SphereGeometry, Vector3 } from "three";
	import type { Mesh, PointLight } from "three";

	let { thinking = false, mood = "calm" }: { thinking?: boolean; mood?: string } = $props();

	let meshRef = $state<Mesh | undefined>();
	let coreRef = $state<Mesh | undefined>();
	let eyeLRef = $state<Mesh | undefined>();
	let eyeRRef = $state<Mesh | undefined>();
	let keyLightRef = $state<PointLight | undefined>();
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

	// Fuzzy mood matcher
	function matchMood(raw: string): string {
		const m = raw.toLowerCase();
		if (moodColors[m]) return m;
		const keys = Object.keys(moodColors).sort((a, b) => b.length - a.length);
		for (const key of keys) {
			if (m.includes(key)) return key;
		}
		return "calm";
	}

	const resolvedMood = $derived(matchMood(mood));
	const baseColor = $derived(moodColors[resolvedMood]);

	// Mood-based animation energy
	type MoodEnergy = {
		speed: number;
		intensity: number;
		breatheRate: number;
		breatheDepth: number;
		rotSpeed: number;
		squashX: number; // horizontal stretch
		squashY: number; // vertical stretch
	};
	const moodEnergies: Record<string, MoodEnergy> = {
		calm:          { speed: 0.8,  intensity: 0.12, breatheRate: 1.2, breatheDepth: 0.04, rotSpeed: 0.15, squashX: 1.0,  squashY: 1.0  },
		excited:       { speed: 2.0,  intensity: 0.20, breatheRate: 2.0, breatheDepth: 0.06, rotSpeed: 0.35, squashX: 0.95, squashY: 1.08 },
		energetic:     { speed: 2.2,  intensity: 0.22, breatheRate: 2.2, breatheDepth: 0.07, rotSpeed: 0.40, squashX: 0.93, squashY: 1.1  },
		playful:       { speed: 1.8,  intensity: 0.18, breatheRate: 1.8, breatheDepth: 0.05, rotSpeed: 0.30, squashX: 0.96, squashY: 1.06 },
		mischievous:   { speed: 1.9,  intensity: 0.19, breatheRate: 1.9, breatheDepth: 0.05, rotSpeed: 0.32, squashX: 0.96, squashY: 1.06 },
		curious:       { speed: 1.2,  intensity: 0.15, breatheRate: 1.5, breatheDepth: 0.05, rotSpeed: 0.22, squashX: 0.98, squashY: 1.04 },
		reflective:    { speed: 0.5,  intensity: 0.08, breatheRate: 0.8, breatheDepth: 0.03, rotSpeed: 0.08, squashX: 1.0,  squashY: 0.98 },
		contemplative: { speed: 0.5,  intensity: 0.08, breatheRate: 0.8, breatheDepth: 0.03, rotSpeed: 0.08, squashX: 1.0,  squashY: 0.98 },
		melancholy:    { speed: 0.4,  intensity: 0.06, breatheRate: 0.6, breatheDepth: 0.02, rotSpeed: 0.05, squashX: 1.06, squashY: 0.92 },
		sad:           { speed: 0.3,  intensity: 0.05, breatheRate: 0.5, breatheDepth: 0.02, rotSpeed: 0.04, squashX: 1.08, squashY: 0.88 },
		tired:         { speed: 0.3,  intensity: 0.05, breatheRate: 0.5, breatheDepth: 0.02, rotSpeed: 0.04, squashX: 1.06, squashY: 0.90 },
		worried:       { speed: 1.3,  intensity: 0.16, breatheRate: 1.6, breatheDepth: 0.03, rotSpeed: 0.20, squashX: 0.97, squashY: 1.02 },
		anxious:       { speed: 1.5,  intensity: 0.18, breatheRate: 1.8, breatheDepth: 0.03, rotSpeed: 0.25, squashX: 0.95, squashY: 1.04 },
		peaceful:      { speed: 0.6,  intensity: 0.10, breatheRate: 1.0, breatheDepth: 0.04, rotSpeed: 0.10, squashX: 1.02, squashY: 0.99 },
		loving:        { speed: 0.9,  intensity: 0.13, breatheRate: 1.3, breatheDepth: 0.05, rotSpeed: 0.18, squashX: 0.98, squashY: 1.03 },
		warm:          { speed: 0.9,  intensity: 0.13, breatheRate: 1.3, breatheDepth: 0.05, rotSpeed: 0.18, squashX: 0.98, squashY: 1.03 },
	};
	const defaultEnergy: MoodEnergy = { speed: 0.8, intensity: 0.12, breatheRate: 1.2, breatheDepth: 0.04, rotSpeed: 0.15, squashX: 1.0, squashY: 1.0 };
	const energy = $derived(moodEnergies[resolvedMood] ?? defaultEnergy);

	// Cache base geometry positions
	const baseGeo = new IcosahedronGeometry(1, 4);
	const basePositions = baseGeo.getAttribute("position").array.slice();
	baseGeo.dispose();

	// Orbital particles setup
	const PARTICLE_COUNT = 8;
	type Particle = {
		radius: number;
		speed: number;
		phase: number;
		tilt: number;
		size: number;
	};
	const particles: Particle[] = Array.from({ length: PARTICLE_COUNT }, (_, i) => ({
		radius: 1.4 + Math.random() * 0.8,
		speed: 0.3 + Math.random() * 0.5,
		phase: (i / PARTICLE_COUNT) * Math.PI * 2,
		tilt: Math.random() * Math.PI * 0.6 - Math.PI * 0.3,
		size: 0.03 + Math.random() * 0.03,
	}));

	let particlePositions = $state(particles.map(() => ({ x: 0, y: 0, z: 0 })));
	let particleOpacities = $state(particles.map(() => 0.4));

	// Eye positioning
	const EYE_SPREAD = 0.28;
	const EYE_HEIGHT = 0.18;
	const EYE_FORWARD = 0.85;

	// Multi-octave noise for organic deformation
	function noise3(x: number, y: number, z: number): number {
		return (
			Math.sin(x * 2.1) * Math.cos(y * 1.8) * Math.sin(z * 2.5) * 0.5 +
			Math.sin(x * 4.3 + 1.7) * Math.cos(y * 3.7 + 2.1) * Math.sin(z * 4.1 + 0.9) * 0.3 +
			Math.sin(x * 7.2 + 3.1) * Math.cos(y * 6.8 + 1.3) * Math.sin(z * 8.1 + 2.7) * 0.2
		);
	}

	// Smoothed values for lerping
	let smoothSpeed = 0.8;
	let smoothIntensity = 0.12;
	let smoothSquashX = 1.0;
	let smoothSquashY = 1.0;
	let smoothCoreGlow = 0.25;

	// Animate everything
	useTask((delta) => {
		time += delta;

		const lerpRate = 1 - Math.pow(0.02, delta);

		// Smooth transitions between states
		const targetSpeed = thinking ? 3.0 : energy.speed;
		const targetIntensity = thinking ? 0.25 : energy.intensity;
		const targetSquashX = thinking ? 1.0 : energy.squashX;
		const targetSquashY = thinking ? 1.0 : energy.squashY;
		const targetCoreGlow = thinking ? 0.6 : 0.2;

		smoothSpeed += (targetSpeed - smoothSpeed) * lerpRate;
		smoothIntensity += (targetIntensity - smoothIntensity) * lerpRate;
		smoothSquashX += (targetSquashX - smoothSquashX) * lerpRate;
		smoothSquashY += (targetSquashY - smoothSquashY) * lerpRate;
		smoothCoreGlow += (targetCoreGlow - smoothCoreGlow) * lerpRate;

		// --- Main blob deformation ---
		if (meshRef) {
			const geo = meshRef.geometry;
			const pos = geo.getAttribute("position");

			const speed = smoothSpeed;
			const intensity = smoothIntensity;
			const breathe = 1.0 + Math.sin(time * (thinking ? 2.5 : energy.breatheRate)) * (thinking ? 0.06 : energy.breatheDepth);

			for (let i = 0; i < pos.count; i++) {
				const bx = basePositions[i * 3];
				const by = basePositions[i * 3 + 1];
				const bz = basePositions[i * 3 + 2];

				// Multi-octave organic noise
				const n = noise3(
					bx + time * speed * 0.7,
					by + time * speed * 0.5,
					bz + time * speed * 0.9
				);

				const scale = breathe + n * intensity;

				// Apply squash/stretch for emotional body language
				pos.setXYZ(
					i,
					bx * scale * smoothSquashX,
					by * scale * smoothSquashY,
					bz * scale * smoothSquashX
				);
			}

			pos.needsUpdate = true;
			geo.computeVertexNormals();

			// Gentle rotation
			meshRef.rotation.y += delta * (thinking ? 0.4 : energy.rotSpeed);
			meshRef.rotation.x = Math.sin(time * 0.3) * 0.1;
		}

		// --- Inner core pulsing ---
		if (coreRef) {
			const corePulse = 0.42 + Math.sin(time * 1.8) * 0.06 + Math.sin(time * 3.1) * 0.03;
			coreRef.scale.setScalar(corePulse);
			coreRef.rotation.y = time * 0.2;
			coreRef.rotation.x = time * 0.15;
		}

		// --- Eye positioning (follow blob rotation) ---
		if (meshRef && eyeLRef && eyeRRef) {
			const rot = meshRef.rotation.y;

			// Eyes sit on the front surface of the blob
			const fwd = new Vector3(
				Math.sin(rot) * EYE_FORWARD,
				EYE_HEIGHT,
				Math.cos(rot) * EYE_FORWARD
			);
			const right = new Vector3(Math.cos(rot), 0, -Math.sin(rot));

			eyeLRef.position.set(
				fwd.x - right.x * EYE_SPREAD,
				fwd.y,
				fwd.z - right.z * EYE_SPREAD
			);
			eyeRRef.position.set(
				fwd.x + right.x * EYE_SPREAD,
				fwd.y,
				fwd.z + right.z * EYE_SPREAD
			);

			// Blink every ~4 seconds
			const blinkCycle = time % 4.0;
			const blinkScale = blinkCycle < 0.12 ? Math.cos(blinkCycle / 0.12 * Math.PI) * 0.5 + 0.5 : 1.0;

			const eyeSize = thinking ? 0.09 : 0.07;
			eyeLRef.scale.set(eyeSize, eyeSize * blinkScale, eyeSize);
			eyeRRef.scale.set(eyeSize, eyeSize * blinkScale, eyeSize);
		}

		// --- Orbital particles ---
		const newPositions = [];
		const newOpacities = [];
		for (let i = 0; i < PARTICLE_COUNT; i++) {
			const p = particles[i];
			const angle = time * p.speed + p.phase;
			const r = p.radius + Math.sin(time * 0.7 + i) * 0.15;

			// Orbit on tilted plane
			const x = Math.cos(angle) * r;
			const rawY = Math.sin(angle) * r * 0.4;
			const z = Math.sin(angle) * r;

			// Apply tilt
			const y = rawY * Math.cos(p.tilt) + z * Math.sin(p.tilt);
			const tz = rawY * -Math.sin(p.tilt) + z * Math.cos(p.tilt);

			newPositions.push({ x, y, z: tz });

			// Twinkle / fade based on distance from camera
			const distFade = Math.max(0, 1 - Math.abs(tz) / 2.5);
			const twinkle = 0.3 + Math.sin(time * 3 + i * 2.1) * 0.2;
			newOpacities.push(distFade * (thinking ? 0.8 : twinkle + 0.2));
		}
		particlePositions = newPositions;
		particleOpacities = newOpacities;

		// --- Animate key light color shift ---
		if (keyLightRef) {
			const pulse = thinking ? 0.3 : 0.1;
			keyLightRef.intensity = (thinking ? 3.5 : 2.5) + Math.sin(time * 2) * pulse;
		}
	});
</script>

<!-- Camera -->
<T.PerspectiveCamera makeDefault position={[0, 0, 3.2]} fov={45} />

<!-- Lighting -->
<T.AmbientLight intensity={0.05} color="#ffffff" />

<!-- Key light -->
<T.DirectionalLight
	bind:ref={keyLightRef}
	position={[3, 2, 4]}
	intensity={thinking ? 3.5 : 2.5}
	color={baseColor}
/>

<!-- Rim/back light -->
<T.PointLight
	position={[-3, 1, -2]}
	intensity={thinking ? 1.0 : 0.6}
	color="#ffffff"
/>

<!-- Fill from below -->
<T.PointLight
	position={[0, -3, 1]}
	intensity={0.15}
	color={baseColor}
/>

<!-- Main blob -->
<T.Mesh bind:ref={meshRef}>
	<T.IcosahedronGeometry args={[1, 4]} />
	<T.MeshStandardMaterial
		color={baseColor}
		emissive={baseColor}
		emissiveIntensity={thinking ? 0.08 : 0.03}
		roughness={0.55}
		metalness={0.15}
		wireframe={false}
		transparent
		opacity={0.92}
	/>
</T.Mesh>

<!-- Inner glow core -->
<T.Mesh bind:ref={coreRef}>
	<T.IcosahedronGeometry args={[0.45, 3]} />
	<T.MeshStandardMaterial
		color="#ffffff"
		emissive="#ffffff"
		emissiveIntensity={smoothCoreGlow}
		transparent
		opacity={thinking ? 0.2 : 0.08}
	/>
</T.Mesh>

<!-- Eyes -->
<T.Mesh bind:ref={eyeLRef}>
	<T.SphereGeometry args={[1, 8, 8]} />
	<T.MeshStandardMaterial
		color="#ffffff"
		emissive="#ffffff"
		emissiveIntensity={1.2}
		transparent
		opacity={0.9}
	/>
</T.Mesh>
<T.Mesh bind:ref={eyeRRef}>
	<T.SphereGeometry args={[1, 8, 8]} />
	<T.MeshStandardMaterial
		color="#ffffff"
		emissive="#ffffff"
		emissiveIntensity={1.2}
		transparent
		opacity={0.9}
	/>
</T.Mesh>

<!-- Orbital particles -->
{#each particles as p, i}
	<T.Mesh
		position.x={particlePositions[i].x}
		position.y={particlePositions[i].y}
		position.z={particlePositions[i].z}
	>
		<T.SphereGeometry args={[p.size, 4, 4]} />
		<T.MeshStandardMaterial
			color={baseColor}
			emissive={baseColor}
			emissiveIntensity={1.5}
			transparent
			opacity={particleOpacities[i]}
		/>
	</T.Mesh>
{/each}
