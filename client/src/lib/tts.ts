import { getAuthToken } from "$lib/api/client.js";

interface VoiceState {
	speaking: boolean;
	amplitude: number;
	revealProgress: number;
	speakingIds: Set<string>;
}

let ctx: AudioContext | null = null;
let analyser: AnalyserNode | null = null;
let currentSource: AudioBufferSourceNode | null = null;
let rafId = 0;

function getContext(): AudioContext {
	if (!ctx) {
		ctx = new AudioContext();
		analyser = ctx.createAnalyser();
		analyser.fftSize = 256;
		analyser.smoothingTimeConstant = 0.8;
	}
	return ctx;
}

/** Stop any currently playing TTS audio. */
export function stopTts(voice: VoiceState) {
	if (currentSource) {
		try { currentSource.stop(); } catch { /* already stopped */ }
		currentSource = null;
	}
	cancelAnimationFrame(rafId);
	voice.speaking = false;
	voice.amplitude = 0;
	voice.revealProgress = 1;
	voice.speakingIds = new Set();
}

/** Speak text via ElevenLabs TTS proxy. Returns when audio finishes. */
export async function speak(
	slug: string,
	text: string,
	voice: VoiceState,
	messageIds: string[],
): Promise<void> {
	if (!text.trim()) return;

	// Stop any previous playback
	stopTts(voice);

	const ac = getContext();
	if (ac.state === "suspended") await ac.resume();

	const token = getAuthToken();
	const headers: Record<string, string> = { "Content-Type": "application/json" };
	if (token) headers["Authorization"] = `Bearer ${token}`;

	const res = await fetch("/api/tts", {
		method: "POST",
		headers,
		body: JSON.stringify({ text, instance_slug: slug }),
	});

	if (!res.ok) {
		console.warn("[tts] failed:", res.status, await res.text().catch(() => ""));
		return;
	}

	const arrayBuffer = await res.arrayBuffer();
	if (arrayBuffer.byteLength === 0) return;

	let audioBuffer: AudioBuffer;
	try {
		audioBuffer = await ac.decodeAudioData(arrayBuffer);
	} catch (e) {
		console.warn("[tts] failed to decode audio:", e);
		return;
	}

	const source = ac.createBufferSource();
	source.buffer = audioBuffer;
	source.connect(analyser!).connect(ac.destination);
	currentSource = source;

	voice.speaking = true;
	voice.revealProgress = 0;
	voice.speakingIds = new Set(messageIds);

	const totalDuration = audioBuffer.duration;
	const dataArray = new Uint8Array(analyser!.frequencyBinCount);
	const startTime = ac.currentTime;

	function update() {
		if (!voice.speaking) return;

		const elapsed = ac.currentTime - startTime;
		voice.revealProgress = Math.min(elapsed / totalDuration, 1);

		// RMS amplitude from time-domain data
		analyser!.getByteTimeDomainData(dataArray);
		let sum = 0;
		for (let i = 0; i < dataArray.length; i++) {
			const v = (dataArray[i] - 128) / 128;
			sum += v * v;
		}
		voice.amplitude = Math.sqrt(sum / dataArray.length);

		rafId = requestAnimationFrame(update);
	}

	source.start();
	rafId = requestAnimationFrame(update);

	return new Promise<void>((resolve) => {
		source.onended = () => {
			voice.speaking = false;
			voice.amplitude = 0;
			voice.revealProgress = 1;
			voice.speakingIds = new Set();
			currentSource = null;
			cancelAnimationFrame(rafId);
			resolve();
		};
	});
}
