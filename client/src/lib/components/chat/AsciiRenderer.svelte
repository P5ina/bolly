<script lang="ts">
	let { thinking = false, mood = "calm" }: { thinking?: boolean; mood?: string } = $props();

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

	type MouthShape = "smile" | "grin" | "neutral" | "frown" | "open" | "smirk" | "tiny";
	type EyeShape = "normal" | "wide" | "sleepy" | "hearts" | "sparkle" | "closed";

	const moodFaces: Record<string, { mouth: MouthShape; eyes: EyeShape }> = {
		calm:          { mouth: "smile",   eyes: "normal"  },
		curious:       { mouth: "open",    eyes: "wide"    },
		excited:       { mouth: "grin",    eyes: "sparkle" },
		warm:          { mouth: "smile",   eyes: "normal"  },
		happy:         { mouth: "grin",    eyes: "sparkle" },
		joyful:        { mouth: "grin",    eyes: "sparkle" },
		reflective:    { mouth: "neutral", eyes: "sleepy"  },
		contemplative: { mouth: "neutral", eyes: "sleepy"  },
		melancholy:    { mouth: "frown",   eyes: "sleepy"  },
		sad:           { mouth: "frown",   eyes: "closed"  },
		worried:       { mouth: "tiny",    eyes: "wide"    },
		anxious:       { mouth: "tiny",    eyes: "wide"    },
		playful:       { mouth: "grin",    eyes: "sparkle" },
		mischievous:   { mouth: "smirk",   eyes: "normal"  },
		focused:       { mouth: "neutral", eyes: "normal"  },
		tired:         { mouth: "neutral", eyes: "sleepy"  },
		peaceful:      { mouth: "smile",   eyes: "closed"  },
		loving:        { mouth: "smile",   eyes: "hearts"  },
		tender:        { mouth: "smile",   eyes: "hearts"  },
		creative:      { mouth: "open",    eyes: "sparkle" },
		energetic:     { mouth: "grin",    eyes: "wide"    },
	};

	function matchMood(raw: string): string {
		const m = raw.toLowerCase();
		if (moodColors[m]) return m;
		const keys = Object.keys(moodColors).sort((a, b) => b.length - a.length);
		for (const key of keys) {
			if (m.includes(key)) return key;
		}
		return "calm";
	}

	const resolved = $derived(matchMood(mood));
	const color = $derived(moodColors[resolved]);
	const face = $derived(moodFaces[resolved] ?? moodFaces.calm);

	// Mouth SVG paths
	const mouths: Record<MouthShape, string> = {
		smile:   "M 32,52 Q 42,62 52,52",
		grin:    "M 30,50 Q 42,66 54,50",
		neutral: "M 34,52 L 50,52",
		frown:   "M 32,56 Q 42,48 52,56",
		open:    "M 34,50 Q 42,60 50,50 Q 42,56 34,50",
		smirk:   "M 34,52 Q 44,58 52,50",
		tiny:    "M 38,52 Q 42,56 46,52",
	};

	// Animation speeds per mood category
	const breatheSpeed = $derived(
		thinking ? "1.2s" :
		["excited", "energetic", "playful", "anxious"].includes(resolved) ? "1.5s" :
		["sad", "tired", "melancholy"].includes(resolved) ? "4s" :
		"2.5s"
	);

	const wobbleSpeed = $derived(
		thinking ? "0.8s" :
		["excited", "energetic", "playful"].includes(resolved) ? "1.2s" :
		["calm", "peaceful", "reflective"].includes(resolved) ? "3s" :
		"2s"
	);
</script>

<div class="emoji-creature">
	<div class="emoji-glow" style="background: radial-gradient(circle, {color}15 0%, transparent 70%);"></div>

	<svg
		viewBox="0 0 84 84"
		class="emoji-face"
		class:emoji-thinking={thinking}
		style="--breathe-speed: {breatheSpeed}; --wobble-speed: {wobbleSpeed}; --mood-color: {color};"
	>
		<!-- Face circle -->
		<circle
			cx="42" cy="42" r="34"
			fill="none"
			stroke={color}
			stroke-width="2"
			opacity="0.5"
		/>

		<!-- Blush (for warm/loving moods) -->
		{#if ["loving", "tender", "warm", "happy", "joyful"].includes(resolved)}
			<circle cx="26" cy="48" r="5" fill={color} opacity="0.1" />
			<circle cx="58" cy="48" r="5" fill={color} opacity="0.1" />
		{/if}

		<!-- Eyes -->
		<g class="emoji-eyes">
			{#if face.eyes === "normal"}
				<circle cx="34" cy="38" r="3" fill={color} opacity="0.8">
					<animate attributeName="ry" values="3;0.3;3" dur="4s" repeatCount="indefinite" begin="2s" keyTimes="0;0.03;0.06" keySplines="0.4 0 0.2 1;0.4 0 0.2 1" calcMode="spline" />
				</circle>
				<circle cx="50" cy="38" r="3" fill={color} opacity="0.8">
					<animate attributeName="ry" values="3;0.3;3" dur="4s" repeatCount="indefinite" begin="2s" keyTimes="0;0.03;0.06" keySplines="0.4 0 0.2 1;0.4 0 0.2 1" calcMode="spline" />
				</circle>
			{:else if face.eyes === "wide"}
				<circle cx="34" cy="37" r="4" fill={color} opacity="0.8" />
				<circle cx="50" cy="37" r="4" fill={color} opacity="0.8" />
				<circle cx="34" cy="36" r="1.5" fill="white" opacity="0.6" />
				<circle cx="50" cy="36" r="1.5" fill="white" opacity="0.6" />
			{:else if face.eyes === "sleepy"}
				<ellipse cx="34" cy="39" rx="3.5" ry="1.5" fill={color} opacity="0.6" />
				<ellipse cx="50" cy="39" rx="3.5" ry="1.5" fill={color} opacity="0.6" />
			{:else if face.eyes === "hearts"}
				<text x="29" y="42" font-size="10" fill={color} opacity="0.8">&#x2665;</text>
				<text x="45" y="42" font-size="10" fill={color} opacity="0.8">&#x2665;</text>
			{:else if face.eyes === "sparkle"}
				<circle cx="34" cy="38" r="3" fill={color} opacity="0.8" />
				<circle cx="50" cy="38" r="3" fill={color} opacity="0.8" />
				<circle cx="35.5" cy="36.5" r="1" fill="white" opacity="0.9" />
				<circle cx="51.5" cy="36.5" r="1" fill="white" opacity="0.9" />
			{:else if face.eyes === "closed"}
				<path d="M 30,38 Q 34,42 38,38" stroke={color} stroke-width="1.5" fill="none" opacity="0.6" />
				<path d="M 46,38 Q 50,42 54,38" stroke={color} stroke-width="1.5" fill="none" opacity="0.6" />
			{/if}
		</g>

		<!-- Mouth -->
		<path
			d={mouths[face.mouth]}
			stroke={color}
			stroke-width="1.5"
			fill={face.mouth === "open" ? `${color}20` : "none"}
			stroke-linecap="round"
			opacity="0.6"
		/>

		<!-- Thinking indicator: animated dots -->
		{#if thinking}
			<g class="thinking-dots">
				<circle cx="34" cy="62" r="1.5" fill={color} opacity="0.5">
					<animate attributeName="opacity" values="0.2;0.8;0.2" dur="1.2s" repeatCount="indefinite" begin="0s" />
				</circle>
				<circle cx="42" cy="62" r="1.5" fill={color} opacity="0.5">
					<animate attributeName="opacity" values="0.2;0.8;0.2" dur="1.2s" repeatCount="indefinite" begin="0.2s" />
				</circle>
				<circle cx="50" cy="62" r="1.5" fill={color} opacity="0.5">
					<animate attributeName="opacity" values="0.2;0.8;0.2" dur="1.2s" repeatCount="indefinite" begin="0.4s" />
				</circle>
			</g>
		{/if}
	</svg>

	<!-- Floating particles -->
	<div class="emoji-particles">
		{#each Array(5) as _, i}
			<div
				class="emoji-particle"
				style="--i: {i}; --color: {color}; left: {15 + (i * 17) % 70}%; top: {10 + (i * 23) % 60}%;"
			></div>
		{/each}
	</div>
</div>

<style>
	.emoji-creature {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 160px;
		height: 180px;
		margin: 0 auto;
	}

	.emoji-glow {
		position: absolute;
		inset: -20px;
		border-radius: 50%;
		pointer-events: none;
		animation: glow-pulse 4s ease-in-out infinite;
	}

	@keyframes glow-pulse {
		0%, 100% { opacity: 0.6; transform: scale(1); }
		50% { opacity: 1; transform: scale(1.05); }
	}

	.emoji-face {
		width: 120px;
		height: 120px;
		animation:
			emoji-breathe var(--breathe-speed, 2.5s) ease-in-out infinite,
			emoji-wobble var(--wobble-speed, 2s) ease-in-out infinite;
		filter: drop-shadow(0 0 12px color-mix(in oklch, var(--mood-color, #8ab4f8) 30%, transparent));
		transition: filter 0.8s ease;
	}

	.emoji-thinking {
		filter: drop-shadow(0 0 20px color-mix(in oklch, var(--mood-color, #8ab4f8) 50%, transparent));
	}

	@keyframes emoji-breathe {
		0%, 100% { transform: scale(1); }
		50% { transform: scale(1.04); }
	}

	@keyframes emoji-wobble {
		0%, 100% { transform: rotate(0deg); }
		25% { transform: rotate(1.5deg); }
		75% { transform: rotate(-1.5deg); }
	}

	.emoji-eyes {
		animation: emoji-look 6s ease-in-out infinite;
	}

	@keyframes emoji-look {
		0%, 45%, 55%, 100% { transform: translateX(0); }
		50% { transform: translateX(1.5px); }
	}

	/* Particles */
	.emoji-particles {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}

	.emoji-particle {
		position: absolute;
		width: 3px;
		height: 3px;
		border-radius: 50%;
		background: var(--color);
		opacity: 0;
		animation: particle-float 8s ease-in-out infinite;
		animation-delay: calc(var(--i) * -1.6s);
	}

	@keyframes particle-float {
		0%, 100% { opacity: 0; transform: translateY(0) scale(0.5); }
		20% { opacity: 0.4; }
		50% { opacity: 0.2; transform: translateY(-20px) scale(1); }
		80% { opacity: 0.3; }
	}
</style>
