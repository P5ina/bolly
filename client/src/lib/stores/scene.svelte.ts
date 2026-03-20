/**
 * Unified scene store — manages the shared 3D scene state across all routes.
 *
 * Modes:
 *   home      — glass orbs for each instance, raycasting active
 *   selecting — clicked orb moves to center, others fade, navigation starts
 *   intro     — sphere does rising/traveling/settling cinematic
 *   chat      — sphere at final position, mood/voice reactive
 */

import { getContext, setContext } from "svelte";
import type { InstanceSummary } from "$lib/api/types.js";

const SCENE_KEY = Symbol("scene");

export type SceneMode = "home" | "selecting" | "onboarding" | "intro" | "chat";
export type IntroPhase = "idle" | "rising" | "traveling" | "settling" | "done";

export interface SceneStore {
	readonly mode: SceneMode;
	readonly instances: InstanceSummary[];
	hoveredSlug: string | null;
	pendingSelect: string | null;
	readonly selectedSlug: string | null;
	readonly introProgress: number;
	readonly introPhase: IntroPhase;
	readonly selectProgress: number;
	readonly mood: string;
	readonly thinking: boolean;
	readonly voiceAmplitude: number;
	readonly musicEnabled: boolean;

	setInstances(list: InstanceSummary[]): void;
	setMusicEnabled(v: boolean): void;
	selectInstance(slug: string): void;
	enterHome(): void;
	enterOnboarding(slug: string): void;
	finishOnboarding(): void;
	enterChat(slug: string): void;
	setMood(m: string): void;
	setThinking(v: boolean): void;
	setVoiceAmplitude(v: number): void;
	skipIntro(): void;
	musicControl(action: string, track?: string, volume?: number): void;
	destroy(): void;
	tick(): void;
}

const playedSlugs = new Set<string>();

// Timing constants (seconds)
const SELECT_DURATION = 0.7;
const INTRO_DURATION = 6.0;
const PHASE_TRAVELING = 1.5;
const PHASE_SETTLING = 3.5;

export function createSceneStore(): SceneStore {
	let mode = $state<SceneMode>("home");
	let instances = $state<InstanceSummary[]>([]);
	let hoveredSlug = $state<string | null>(null);
	let pendingSelect = $state<string | null>(null);
	let selectedSlug = $state<string | null>(null);
	let introProgress = $state(0);
	let introPhase = $state<IntroPhase>("idle");
	let selectProgress = $state(0);
	let mood = $state("calm");
	let thinking = $state(false);
	let voiceAmplitude = $state(0);
	let musicEnabled = $state(true);

	let selectStartTime = 0;
	let introStartTime = 0;

	// ── Audio ──
	let introAudio: HTMLAudioElement | null = null;
	let loopAudio: HTMLAudioElement | null = null;
	let ambientAudio: HTMLAudioElement | null = null;
	let customAudio: HTMLAudioElement | null = null;

	// Pending audio start — retried on first user interaction if autoplay blocked
	let pendingAudioFn: (() => void) | null = null;
	let gestureListenerAdded = false;

	function addGestureListener() {
		if (gestureListenerAdded) return;
		gestureListenerAdded = true;
		const handler = () => {
			if (pendingAudioFn) {
				pendingAudioFn();
				pendingAudioFn = null;
			}
			document.removeEventListener("click", handler, true);
			document.removeEventListener("touchstart", handler, true);
			document.removeEventListener("keydown", handler, true);
			gestureListenerAdded = false;
		};
		document.addEventListener("click", handler, { capture: true, once: false });
		document.addEventListener("touchstart", handler, { capture: true, once: false });
		document.addEventListener("keydown", handler, { capture: true, once: false });
	}

	function tryPlay(audio: HTMLAudioElement): Promise<boolean> {
		return audio.play().then(() => true).catch(() => false);
	}

	function startFullAudio() {
		if (!musicEnabled) return;
		if (!ambientAudio) {
			ambientAudio = new Audio("/sounds/ambient.mp3");
			ambientAudio.loop = true;
			ambientAudio.volume = 0.3;
		}
		if (!introAudio) {
			introAudio = new Audio("/sounds/intro.mp3");
			introAudio.loop = false;
			introAudio.volume = 0.5;
		}
		if (!loopAudio) {
			loopAudio = new Audio("/sounds/loop.mp3");
			loopAudio.loop = true;
			loopAudio.volume = 0;
		}

		introAudio.onended = () => {
			if (loopAudio) {
				loopAudio.currentTime = 0;
				loopAudio.play().catch(() => {});
				fadeAudio(loopAudio, 0.5, 2000);
			}
		};

		const doPlay = () => {
			ambientAudio!.currentTime = 0;
			introAudio!.currentTime = 0;
			ambientAudio!.play().catch(() => {});
			introAudio!.play().catch(() => {});
		};

		// Try immediately; if blocked, queue for first interaction
		ambientAudio.currentTime = 0;
		introAudio.currentTime = 0;
		tryPlay(introAudio).then((ok) => {
			if (ok) {
				ambientAudio!.play().catch(() => {});
			} else {
				pendingAudioFn = doPlay;
				addGestureListener();
			}
		});
	}

	function startLoopOnly() {
		if (!musicEnabled) return;
		if (!ambientAudio) {
			ambientAudio = new Audio("/sounds/ambient.mp3");
			ambientAudio.loop = true;
			ambientAudio.volume = 0.3;
		}
		if (!loopAudio) {
			loopAudio = new Audio("/sounds/loop.mp3");
			loopAudio.loop = true;
			loopAudio.volume = 0.5;
		}

		const doPlay = () => {
			ambientAudio!.play().catch(() => {});
			loopAudio!.play().catch(() => {});
		};

		tryPlay(loopAudio).then((ok) => {
			if (ok) {
				ambientAudio!.play().catch(() => {});
			} else {
				pendingAudioFn = doPlay;
				addGestureListener();
			}
		});
	}

	function stopAudio() {
		if (introAudio) { introAudio.pause(); introAudio = null; }
		if (loopAudio) { loopAudio.pause(); loopAudio = null; }
		if (ambientAudio) { ambientAudio.pause(); ambientAudio = null; }
		if (customAudio) { customAudio.pause(); customAudio = null; }
	}

	function getTrackAudio(track: string): HTMLAudioElement | null {
		switch (track) {
			case "ambient": return ambientAudio;
			case "intro": return introAudio;
			case "loop": return loopAudio;
			case "custom": return customAudio;
			default: return null;
		}
	}

	function fadeAudio(audio: HTMLAudioElement, target: number, ms: number) {
		const start = audio.volume;
		const t0 = performance.now();
		(function step() {
			const p = Math.min((performance.now() - t0) / ms, 1);
			audio.volume = start + (target - start) * p;
			if (p < 1) requestAnimationFrame(step);
		})();
	}

	// ── Tick — called every frame by SharedScene ──
	function tick() {
		if (mode === "selecting") {
			const elapsed = (performance.now() - selectStartTime) / 1000;
			selectProgress = Math.min(elapsed / SELECT_DURATION, 1);
			if (elapsed >= SELECT_DURATION) {
				mode = "intro";
				introStartTime = performance.now();
				introProgress = 0;
				introPhase = "rising";
				selectProgress = 1;
			}
		} else if (mode === "intro") {
			const elapsed = (performance.now() - introStartTime) / 1000;
			introProgress = Math.min(elapsed / INTRO_DURATION, 1);
			if (elapsed < PHASE_TRAVELING) {
				introPhase = "rising";
			} else {
				// Switch to chat as soon as sphere starts traveling to final pos.
				// The sphere animation continues smoothly via lerp in SharedScene.
				mode = "chat";
				introPhase = "done";
				introProgress = 1;
			}
		}
	}

	const store: SceneStore = {
		get mode() { return mode; },
		get instances() { return instances; },
		get hoveredSlug() { return hoveredSlug; },
		set hoveredSlug(v) { hoveredSlug = v; },
		get pendingSelect() { return pendingSelect; },
		set pendingSelect(v) { pendingSelect = v; },
		get selectedSlug() { return selectedSlug; },
		get introProgress() { return introProgress; },
		get introPhase() { return introPhase; },
		get selectProgress() { return selectProgress; },
		get mood() { return mood; },
		get thinking() { return thinking; },
		get voiceAmplitude() { return voiceAmplitude; },
		get musicEnabled() { return musicEnabled; },

		setInstances(list) { instances = list; },
		setMusicEnabled(v) { musicEnabled = v; if (!v) stopAudio(); },

		selectInstance(slug: string) {
			if (mode !== "home") return;
			selectedSlug = slug;
			mode = "selecting";
			selectStartTime = performance.now();
			selectProgress = 0;
			pendingSelect = slug;
			if (!playedSlugs.has(slug)) {
				playedSlugs.add(slug);
				startFullAudio();
			}
		},

		enterHome() {
			if (mode === "selecting" || mode === "intro") return;
			// Clear played so intro replays on next visit
			playedSlugs.clear();
			mode = "home";
			selectedSlug = null;
			introProgress = 0;
			introPhase = "idle";
			selectProgress = 0;
			stopAudio();
		},

		enterOnboarding(slug: string) {
			selectedSlug = slug;
			mode = "onboarding";
			introProgress = 0;
			introPhase = "idle";
		},

		finishOnboarding() {
			// Transition onboarding → intro → chat
			if (mode !== "onboarding") return;
			mode = "intro";
			introStartTime = performance.now();
			introProgress = 0;
			introPhase = "rising";
			if (selectedSlug && !playedSlugs.has(selectedSlug)) {
				playedSlugs.add(selectedSlug);
				startFullAudio();
			}
		},

		enterChat(slug: string) {
			if (mode === "selecting" || mode === "intro" || mode === "onboarding") return;
			selectedSlug = slug;
			if (playedSlugs.has(slug)) {
				mode = "chat";
				introPhase = "done";
				introProgress = 1;
				startLoopOnly();
			} else {
				playedSlugs.add(slug);
				mode = "intro";
				introStartTime = performance.now();
				introProgress = 0;
				introPhase = "rising";
				startFullAudio();
			}
		},

		setMood(m) { mood = m; },
		setThinking(v) { thinking = v; },
		setVoiceAmplitude(v) { voiceAmplitude = v; },

		musicControl(action: string, track?: string, volume?: number) {
			if (action === "pause") {
				stopAudio();
				return;
			}
			if (action === "set_volume" && track) {
				const audio = getTrackAudio(track);
				if (audio && volume !== undefined) {
					fadeAudio(audio, Math.max(0, Math.min(1, volume)), 500);
				}
				return;
			}
			if (action === "play" && track) {
				const vol = volume ?? 0.5;
				const isBuiltIn = track === "ambient" || track === "intro" || track === "loop";
				if (!isBuiltIn) {
					// Custom audio — URL or relative path (e.g. /api/instances/.../file)
					if (customAudio) { customAudio.pause(); }
					customAudio = new Audio(track);
					customAudio.loop = true;
					customAudio.volume = vol;
					customAudio.play().catch(() => {});
				} else {
					// Built-in track
					const builtIn: Record<string, () => void> = {
						ambient: () => {
							if (!ambientAudio) {
								ambientAudio = new Audio("/sounds/ambient.mp3");
								ambientAudio.loop = true;
							}
							ambientAudio.volume = vol;
							ambientAudio.play().catch(() => {});
						},
						intro: () => {
							if (!introAudio) {
								introAudio = new Audio("/sounds/intro.mp3");
								introAudio.loop = false;
							}
							introAudio.volume = vol;
							introAudio.currentTime = 0;
							introAudio.play().catch(() => {});
						},
						loop: () => {
							if (!loopAudio) {
								loopAudio = new Audio("/sounds/loop.mp3");
								loopAudio.loop = true;
							}
							loopAudio.volume = vol;
							loopAudio.play().catch(() => {});
						},
					};
					builtIn[track]?.();
				}
			}
		},

		skipIntro() {
			mode = "chat";
			introPhase = "done";
			introProgress = 1;
		},

		destroy() {
			stopAudio();
		},

		tick,
	};

	return store;
}

export function setSceneStore(s: SceneStore) {
	setContext(SCENE_KEY, s);
}

export function getSceneStore(): SceneStore {
	return getContext<SceneStore>(SCENE_KEY);
}
