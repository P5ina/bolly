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

/**
 * Warm up AudioContext from a user gesture (click/tap).
 * Must be called before any playback to satisfy autoplay policy.
 */
export function warmUpAudio(): void {
	const ac = getContext();
	if (ac.state === "suspended") ac.resume();
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
	const ac = getContext();

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

/** Play base64-encoded MP3 audio received from the server via WebSocket. */
export async function playBase64Audio(
	base64: string,
	voice: VoiceState,
	messageIds: string[],
): Promise<void> {
	stopTts(voice);

	const ac = getContext();
	if (ac.state === "suspended") await ac.resume();

	const binaryString = atob(base64);
	const bytes = new Uint8Array(binaryString.length);
	for (let i = 0; i < binaryString.length; i++) {
		bytes[i] = binaryString.charCodeAt(i);
	}

	let audioBuffer: AudioBuffer;
	try {
		audioBuffer = await ac.decodeAudioData(bytes.buffer);
	} catch (e) {
		console.warn("[tts] failed to decode base64 audio:", e);
		return;
	}

	return playBuffer(audioBuffer, voice, messageIds);
}
