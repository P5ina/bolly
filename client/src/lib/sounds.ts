import { getAudioContext } from "./audio-context.js";

const buffers = new Map<string, AudioBuffer>();

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

async function loadBuffer(name: string): Promise<AudioBuffer | null> {
	const existing = buffers.get(name);
	if (existing) return existing;

	try {
		const res = await fetch(`/sounds/${name}.mp3`);
		const data = await res.arrayBuffer();
		const ac = getAudioContext();
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
	const ac = getAudioContext();

	// Fast path: buffer cached & context running — play immediately
	const buffer = buffers.get(name);
	if (buffer && ac.state === "running") {
		const now = performance.now();
		if (now - lastPlayTime < MIN_GAP_MS) return; // skip if too soon
		const source = ac.createBufferSource();
		const gain = ac.createGain();
		gain.gain.value = volumes[name] ?? 0.25;
		source.buffer = buffer;
		source.connect(gain).connect(ac.destination);
		source.start();
		lastPlayTime = now;
		return;
	}

	// Slow path: need to load buffer or resume context
	playQueue = playQueue.then(() => playSound(name)).catch(() => {});
}

async function playSound(name: string) {
	const ac = getAudioContext();

	if (ac.state === "suspended") {
		try {
			await ac.resume();
		} catch {
			return;
		}
	}
	if (ac.state !== "running") return;

	let buffer = buffers.get(name);
	if (!buffer) {
		buffer = (await loadBuffer(name)) ?? undefined;
		if (!buffer) return;
	}

	const now = performance.now();
	if (now - lastPlayTime < MIN_GAP_MS) return;

	const source = ac.createBufferSource();
	const gain = ac.createGain();
	gain.gain.value = volumes[name] ?? 0.25;
	source.buffer = buffer;
	source.connect(gain).connect(ac.destination);
	source.start();
	lastPlayTime = performance.now();
}

/** Fire-and-forget playback without queue or gap enforcement. */
export function playImmediate(name: string, opts?: { pitchRange?: [number, number] }) {
	if (typeof window === "undefined") return;
	const ac = getAudioContext();
	if (ac.state !== "running") return;
	const buffer = buffers.get(name);
	if (!buffer) return;
	const source = ac.createBufferSource();
	if (opts?.pitchRange) {
		const [lo, hi] = opts.pitchRange;
		source.playbackRate.value = lo + Math.random() * (hi - lo);
	}
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
