<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { getSceneStore } from "$lib/stores/scene.svelte.js";

	const store = getSceneStore();

	let container: HTMLDivElement | undefined = $state();

	function easeOutCubic(x: number) { return 1 - Math.pow(1 - x, 3); }
	function easeInOutQuart(x: number) {
		return x < 0.5 ? 8 * x * x * x * x : 1 - Math.pow(-2 * x + 2, 4) / 2;
	}

	// ── Video state machine ──
	let videoState = $state<'idle' | 'to-thinking' | 'thinking' | 'to-idle'>('idle');

	const videoFiles: Record<string, string> = {
		'idle': '/orb-idle-loop',
		'to-thinking': '/orb-idle-to-thinking',
		'thinking': '/orb-thinking-loop',
		'to-idle': '/orb-thinking-to-idle',
	};

	let isOnboarding = $derived(store.mode === 'onboarding');
	let onboardingDone = $state(false);
	let videoSrc = $derived(
		isOnboarding && !onboardingDone ? '/orb-onboarding.mp4' : videoFiles[videoState] + '.mp4'
	);
	let isThinking = $derived(store.thinking);
	let pendingIdle = $state(false); // thinking ended while transition was playing

	$effect(() => {
		if (isThinking) {
			pendingIdle = false;
			if (videoState === 'idle') {
				videoState = 'to-thinking';
			} else if (videoState === 'to-idle') {
				// Already transitioning back — let it finish, then go to thinking
				// Actually just stay, it'll go to idle then we pick it up
			}
		} else if (!isThinking) {
			if (videoState === 'thinking') {
				videoState = 'to-idle';
			} else if (videoState === 'to-thinking') {
				// Transition still playing — queue the return
				pendingIdle = true;
			}
		}
	});

	function handleVideoEnded(e: Event) {
		if (isOnboarding && !onboardingDone) {
			onboardingDone = true;
			return;
		}
		if (videoState === 'to-thinking') {
			if (pendingIdle) {
				// Thinking ended while transition was playing — go back
				pendingIdle = false;
				videoState = 'to-idle';
			} else {
				videoState = 'thinking';
			}
		} else if (videoState === 'to-idle') {
			videoState = 'idle';
		}
	}

	// ── Orb state ──
	interface OrbState {
		slug: string;
		// Position as % of container (50 = center)
		x: number;
		y: number;
		// Size in px
		size: number;
		opacity: number;
		visible: boolean;
	}

	let orbs = $state<OrbState[]>([]);
	let raf: number;
	let prevTime = 0;
	let elapsed = 0;
	let lastMode = '';
	let lastWideVideo = false;

	// Convert 3D world X coord to CSS left% (camera at z=5, FOV 50)
	// At z=5 with FOV 50, visible width ≈ 2 * 5 * tan(25°) ≈ 4.66
	// So worldX=1.8 → 50% + 1.8/4.66*50% ≈ 69.3%
	const WORLD_TO_PCT = 50 / 4.66; // ~10.73% per world unit
	const BASE_SIZE = 450; // px for scale=1.0 (cropped square video)

	const HOME_SCALE = 0.5;
	const FINAL_SCALE = 1.2;
	const FINAL_X = 1.8;
	const FINAL_Y = -0.1;

	function animate() {
		raf = requestAnimationFrame(animate);

		const now = performance.now();
		const delta = (now - prevTime) / 1000;
		prevTime = now;
		if (delta > 0.1) return; // skip large gaps

		store.tick();

		const m = store.mode;
		const sel = store.selectedSlug;
		const instances = store.instances;

		// Build slug list
		const slugs: string[] = instances.map(i => i.slug);
		if (sel && !slugs.includes(sel)) slugs.push(sel);

		// Ensure orbs array matches slugs
		const existing = new Map(orbs.map(o => [o.slug, o]));
		const newOrbs: OrbState[] = slugs.map(slug => {
			return existing.get(slug) ?? { slug, x: 50, y: 50, size: 0, opacity: 0, visible: false };
		});

		// Home positions
		const count = instances.length;
		const spacing = 1.4;
		const totalW = (count - 1) * spacing;
		const startX = -totalW / 2;
		const homePositions = new Map<string, number>();
		instances.forEach((inst, i) => {
			homePositions.set(inst.slug, startX + i * spacing);
		});

		const useLerp = m === "home" || m === "chat" || m === "onboarding";
		const lerpF = Math.min(delta * 6, 1);

		for (const orb of newOrbs) {
			const isSelected = orb.slug === sel;
			const isHovered = orb.slug === store.hoveredSlug;
			const homeX = homePositions.get(orb.slug) ?? 0;

			let tx = 50 + homeX * WORLD_TO_PCT;
			let ty = 50;
			let ts = HOME_SCALE * BASE_SIZE;
			let to = 1;

			if (m === "home") {
				ts = (isHovered ? 0.58 : HOME_SCALE) * BASE_SIZE;
			} else if (m === "onboarding") {
				if (isSelected) {
					tx = 50; ty = 50;
					// Full viewport background - use container size
					const cw = container?.clientWidth ?? 1200;
					const ch = container?.clientHeight ?? 800;
					ts = Math.max(cw, ch) * 1.2;
					to = 0.6;
				} else {
					ts = 0; to = 0;
				}
			} else if (m === "selecting") {
				const e = easeInOutQuart(store.selectProgress);
				if (isSelected) {
					const hx = 50 + homeX * WORLD_TO_PCT;
					tx = hx + (50 - hx) * e;
					ty = 50;
					ts = (HOME_SCALE + e * (FINAL_SCALE - HOME_SCALE)) * BASE_SIZE;
				} else {
					ts = HOME_SCALE * (1 - e) * BASE_SIZE;
					to = 1 - e;
				}
			} else if (m === "intro") {
				if (isSelected) {
					const p = store.introProgress;
					if (p < 0.25) {
						const e = easeOutCubic(p / 0.25);
						tx = 50; ty = 50;
						ts = (FINAL_SCALE + e * 0.15) * BASE_SIZE;
					} else if (p < 0.58) {
						const e = easeInOutQuart((p - 0.25) / 0.33);
						tx = 50 + FINAL_X * WORLD_TO_PCT * e;
						ty = 50 + FINAL_Y * WORLD_TO_PCT * e;
						ts = (FINAL_SCALE * 1.15 + (FINAL_SCALE - FINAL_SCALE * 1.15) * e) * BASE_SIZE;
					} else {
						tx = 50 + FINAL_X * WORLD_TO_PCT;
						ty = 50 + FINAL_Y * WORLD_TO_PCT;
						ts = FINAL_SCALE * BASE_SIZE;
					}
				} else {
					ts = 0; to = 0;
				}
			} else if (m === "chat") {
				if (isSelected) {
					if (store.presenting) {
						tx = 50;
						ty = 50;
						ts = FINAL_SCALE * BASE_SIZE * 1.5;
					} else {
						tx = 50 + FINAL_X * WORLD_TO_PCT;
						ty = 50 + FINAL_Y * WORLD_TO_PCT;
						ts = FINAL_SCALE * BASE_SIZE;
					}
				} else {
					ts = 0; to = 0;
				}
			}

			// Thinking videos are 16:9, idle are square — compensate size
			const wideVideo = videoState === 'thinking' || videoState === 'to-thinking' || videoState === 'to-idle';
			if (wideVideo) ts *= 1.8;
			const formatChanged = wideVideo !== lastWideVideo;

			if (useLerp && !formatChanged) {
				orb.x += (tx - orb.x) * lerpF;
				orb.y += (ty - orb.y) * lerpF;
				orb.size += (ts - orb.size) * lerpF;
				orb.opacity += (to - orb.opacity) * lerpF;
			} else {
				orb.x = tx;
				orb.y = ty;
				orb.size = ts;
				orb.opacity = to;
			}

			orb.visible = orb.size > 1 && orb.opacity > 0.01;
		}

		orbs = newOrbs;
		lastWideVideo = videoState === 'thinking' || videoState === 'to-thinking' || videoState === 'to-idle';

		if (m !== lastMode) {
			lastMode = m;
		}
	}

	onMount(() => {
		if (window.innerWidth < 640) return;
		prevTime = performance.now();
		animate();
	});

	onDestroy(() => {
		if (raf) cancelAnimationFrame(raf);
	});

	function hitTestOrb(clientX: number, clientY: number): string | null {
		if (!container) return null;
		for (const orb of orbs) {
			if (!orb.visible) continue;
			const rect = container.getBoundingClientRect();
			const orbCx = rect.left + rect.width * orb.x / 100;
			const orbCy = rect.top + rect.height * orb.y / 100;
			const dx = clientX - orbCx;
			const dy = clientY - orbCy;
			const hitRadius = orb.size * 0.22;
			if (Math.sqrt(dx * dx + dy * dy) <= hitRadius) return orb.slug;
		}
		return null;
	}

	function handleSceneMove(e: MouseEvent) {
		if (store.mode !== "home" || !container) return;
		const hit = hitTestOrb(e.clientX, e.clientY);
		store.hoveredSlug = hit;
		(e.currentTarget as HTMLElement).style.cursor = hit ? 'pointer' : 'default';
	}

	function handleSceneClick(e: MouseEvent) {
		if (store.mode !== "home") return;
		const hit = hitTestOrb(e.clientX, e.clientY);
		if (hit) store.selectInstance(hit);
	}
</script>

<div class="scene-root" bind:this={container}>
{#if store.mode === 'home'}
	<div class="hit-layer"
		role="none"
		onmousemove={handleSceneMove}
		onclick={handleSceneClick}
		onmouseleave={() => { store.hoveredSlug = null; if (container) container.style.cursor = 'default'; }}
	></div>
{/if}
	{#each orbs as orb (orb.slug)}
		{#if orb.visible}
			<button
				class="orb-btn"
				style="left: {orb.x}%; top: {orb.y}%; width: {orb.size}px; height: {orb.size}px; opacity: {orb.opacity};"
				disabled={store.mode !== "home"}
			>
				<video
					autoplay muted playsinline
					loop={(isOnboarding && onboardingDone) || (!isOnboarding && (videoState === 'idle' || videoState === 'thinking'))}
					class="orb-vid"
					class:no-mask={videoState === 'thinking' || videoState === 'to-thinking' || videoState === 'to-idle'}
					src={videoSrc}
					onended={handleVideoEnded}
					onloadeddata={(e) => { const v = e.target as HTMLVideoElement; v.play().catch(() => {}); }}
				></video>
			</button>
		{/if}
	{/each}
</div>

<style>
	.scene-root {
		position: absolute;
		inset: 0;
		z-index: 0;
		overflow: hidden;
		pointer-events: none;
	}

	.hit-layer {
		position: absolute;
		inset: 0;
		pointer-events: auto;
		z-index: 10;
	}

	.orb-btn {
		position: absolute;
		background: none;
		border: none;
		padding: 0;
		cursor: pointer;
		pointer-events: auto;
		transform: translate(-50%, -50%);
		will-change: left, top, width, height, opacity;
		overflow: visible;
	}

	.orb-btn:disabled {
		cursor: default;
	}

	.orb-vid {
		width: 100%;
		height: 100%;
		object-fit: contain;
		pointer-events: none;
		mask-image: radial-gradient(circle at 50% 50%, black 30%, transparent 48%);
		-webkit-mask-image: radial-gradient(circle at 50% 50%, black 30%, transparent 48%);
	}

	.orb-vid.no-mask {
		mask-image: none;
		-webkit-mask-image: none;
	}

	@media (max-width: 640px) {
		.scene-root { display: none; }
	}

	@media (prefers-reduced-motion: reduce) {
		.orb-vid { display: none; }
	}
</style>
