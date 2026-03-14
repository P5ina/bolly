const buffers = new Map<string, AudioBuffer>();
let ctx: AudioContext | null = null;

const volumes: Record<string, number> = {
	message_send: 0.3,
	message_receive: 0.25,
	attachment_added: 0.25,
	mood_shift: 0.15,
	intro_reveal: 0.4,
	typewriter: 0.15,
	drop_received: 0.3,
	error: 0.3,
};

// Minimum ms between any two sounds to prevent overlap
const MIN_GAP_MS = 150;
let lastPlayTime = 0;

function getContext(): AudioContext {
	if (!ctx) ctx = new AudioContext();
	return ctx;
}

if (typeof document !== "undefined") {
	// Resume on user interaction (first click/tap unlocks audio)
	const unlock = () => {
		if (ctx?.state === "suspended") ctx.resume();
		document.removeEventListener("click", unlock);
		document.removeEventListener("touchstart", unlock);
	};
	document.addEventListener("click", unlock);
	document.addEventListener("touchstart", unlock);

	document.addEventListener("visibilitychange", () => {
		if (document.visibilityState === "visible" && ctx?.state === "suspended") {
			ctx.resume();
		}
	});
}

async function loadBuffer(name: string): Promise<AudioBuffer | null> {
	const existing = buffers.get(name);
	if (existing) return existing;

	try {
		const res = await fetch(`/sounds/${name}.mp3`);
		const data = await res.arrayBuffer();
		const ac = getContext();
		if (ac.state === "suspended") await ac.resume();
		const buffer = await ac.decodeAudioData(data);
		buffers.set(name, buffer);
		return buffer;
	} catch {
		return null;
	}
}

// Queue to serialize playback and prevent overlap
let playQueue: Promise<void> = Promise.resolve();

export function play(name: string) {
	if (typeof window === "undefined") return;

	playQueue = playQueue.then(() => playSound(name)).catch(() => {});
}

async function playSound(name: string) {
	const ac = getContext();

	// Ensure context is running (may be suspended until user gesture)
	if (ac.state === "suspended") {
		try {
			await ac.resume();
		} catch {
			return;
		}
	}
	if (ac.state !== "running") return;

	// Enforce minimum gap between sounds
	const now = performance.now();
	const elapsed = now - lastPlayTime;
	if (elapsed < MIN_GAP_MS) {
		await new Promise((r) => setTimeout(r, MIN_GAP_MS - elapsed));
	}

	// Load buffer (cached after first load)
	let buffer = buffers.get(name);
	if (!buffer) {
		buffer = (await loadBuffer(name)) ?? undefined;
		if (!buffer) return;
	}

	const source = ac.createBufferSource();
	const gain = ac.createGain();
	gain.gain.value = volumes[name] ?? 0.25;
	source.buffer = buffer;
	source.connect(gain).connect(ac.destination);
	source.start();
	lastPlayTime = performance.now();
}

/** Fire-and-forget playback without queue or gap enforcement. */
export function playImmediate(name: string) {
	if (typeof window === "undefined") return;
	const ac = getContext();
	if (ac.state !== "running") return;
	const buffer = buffers.get(name);
	if (!buffer) return;
	const source = ac.createBufferSource();
	const gain = ac.createGain();
	gain.gain.value = volumes[name] ?? 0.25;
	source.buffer = buffer;
	source.connect(gain).connect(ac.destination);
	source.start();
}

export function preload(...names: string[]) {
	if (typeof window === "undefined") return;
	names.forEach((n) => loadBuffer(n));
}
