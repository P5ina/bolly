import { getAudioContext, resumeAudioContext } from "./audio-context.js";

interface VoiceState {
	speaking: boolean;
	amplitude: number;
	revealProgress: number;
	speakingIds: Set<string>;
}

let analyser: AnalyserNode | null = null;
let currentSource: AudioBufferSourceNode | null = null;
let rafId = 0;

function getAnalyser(): AnalyserNode {
	if (!analyser) {
		const ac = getAudioContext();
		analyser = ac.createAnalyser();
		analyser.fftSize = 256;
		analyser.smoothingTimeConstant = 0.8;
	}
	return analyser;
}

/**
 * Warm up AudioContext from a user gesture (click/tap).
 * Must be called before any playback to satisfy autoplay policy.
 */
export function warmUpAudio(): void {
	resumeAudioContext();
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

/** Play an AudioBuffer with amplitude tracking and word reveal. */
function playBuffer(
	audioBuffer: AudioBuffer,
	voice: VoiceState,
	messageIds: string[],
): Promise<void> {
	const ac = getAudioContext();
	const an = getAnalyser();

	const source = ac.createBufferSource();
	source.buffer = audioBuffer;
	source.connect(an).connect(ac.destination);
	currentSource = source;

	voice.speaking = true;
	voice.revealProgress = 0;
	voice.speakingIds = new Set(messageIds);

	const totalDuration = audioBuffer.duration;
	const dataArray = new Uint8Array(an.frequencyBinCount);
	const startTime = ac.currentTime;

	function update() {
		if (!voice.speaking) return;

		const elapsed = ac.currentTime - startTime;
		voice.revealProgress = Math.min(elapsed / totalDuration, 1);

		// RMS amplitude from time-domain data
		an.getByteTimeDomainData(dataArray);
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

/** Audio queue — clips play sequentially, not overlapping. */
type QueueEntry = { base64: string; messageIds: string[] };
let audioQueue: QueueEntry[] = [];
let queuePlaying = false;

async function processQueue(voice: VoiceState): Promise<void> {
	if (queuePlaying) return;
	queuePlaying = true;

	while (audioQueue.length > 0) {
		const entry = audioQueue.shift()!;

		const ac = getAudioContext();
		if (ac.state === "suspended") await ac.resume();

		const binaryString = atob(entry.base64);
		const bytes = new Uint8Array(binaryString.length);
		for (let i = 0; i < binaryString.length; i++) {
			bytes[i] = binaryString.charCodeAt(i);
		}

		let audioBuffer: AudioBuffer;
		try {
			audioBuffer = await ac.decodeAudioData(bytes.buffer.slice(0));
		} catch (e) {
			console.warn("[tts] failed to decode audio:", e);
			continue;
		}

		await playBuffer(audioBuffer, voice, entry.messageIds);

		// Keep speaking=true between queue entries to avoid flash-hiding
		if (audioQueue.length > 0) {
			voice.speaking = true;
		}
	}

	queuePlaying = false;
}

/** Queue base64-encoded MP3 audio for sequential playback. */
export function playBase64Audio(
	base64: string,
	voice: VoiceState,
	messageIds: string[],
): void {
	audioQueue.push({ base64, messageIds });
	processQueue(voice);
}

/** Clear the audio queue (e.g. when user sends a new message). */
export function clearAudioQueue(): void {
	audioQueue = [];
}
