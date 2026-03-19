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

export type SceneMode = "home" | "selecting" | "intro" | "chat";
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

	setInstances(list: InstanceSummary[]): void;
	selectInstance(slug: string): void;
	enterHome(): void;
	enterChat(slug: string): void;
	setMood(m: string): void;
	setThinking(v: boolean): void;
	setVoiceAmplitude(v: number): void;
	skipIntro(): void;
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

	let selectStartTime = 0;
	let introStartTime = 0;

	// ── Audio ──
	let introAudio: HTMLAudioElement | null = null;
	let loopAudio: HTMLAudioElement | null = null;
	let ambientAudio: HTMLAudioElement | null = null;

	function startFullAudio() {
		if (!ambientAudio) {
			ambientAudio = new Audio("/sounds/ambient.mp3");
			ambientAudio.loop = true;
			ambientAudio.volume = 0.3;
		}
		ambientAudio.currentTime = 0;
		ambientAudio.play().catch(() => {});

		if (!introAudio) {
			introAudio = new Audio("/sounds/intro.mp3");
			introAudio.loop = false;
			introAudio.volume = 0.5;
		}
		introAudio.currentTime = 0;
		introAudio.play().catch(() => {});

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
	}

	function startLoopOnly() {
		if (!ambientAudio) {
			ambientAudio = new Audio("/sounds/ambient.mp3");
			ambientAudio.loop = true;
			ambientAudio.volume = 0.3;
		}
		ambientAudio.play().catch(() => {});

		if (!loopAudio) {
			loopAudio = new Audio("/sounds/loop.mp3");
			loopAudio.loop = true;
			loopAudio.volume = 0.5;
		}
		loopAudio.play().catch(() => {});
	}

	function stopAudio() {
		if (introAudio) { introAudio.pause(); introAudio = null; }
		if (loopAudio) { loopAudio.pause(); loopAudio = null; }
		if (ambientAudio) { ambientAudio.pause(); ambientAudio = null; }
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
			} else if (elapsed < PHASE_SETTLING) {
				introPhase = "traveling";
			} else if (elapsed < INTRO_DURATION) {
				introPhase = "settling";
			} else {
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

		setInstances(list) { instances = list; },

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
			mode = "home";
			selectedSlug = null;
			introProgress = 0;
			introPhase = "idle";
			selectProgress = 0;
			stopAudio();
		},

		enterChat(slug: string) {
			if (mode === "selecting" || mode === "intro") return;
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
