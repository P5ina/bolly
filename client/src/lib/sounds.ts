const buffers = new Map<string, AudioBuffer>();
let ctx: AudioContext | null = null;

const volumes: Record<string, number> = {
	message_send: 0.3,
	message_receive: 0.25,
	attachment_added: 0.25,
	mood_shift: 0.15,
	intro_reveal: 0.4,
	drop_received: 0.3,
};

function getContext(): AudioContext {
	if (!ctx) ctx = new AudioContext();
	if (ctx.state === "suspended") ctx.resume();
	return ctx;
}

async function loadBuffer(name: string): Promise<AudioBuffer | null> {
	const existing = buffers.get(name);
	if (existing) return existing;

	try {
		const res = await fetch(`/sounds/${name}.mp3`);
		const data = await res.arrayBuffer();
		const buffer = await getContext().decodeAudioData(data);
		buffers.set(name, buffer);
		return buffer;
	} catch {
		return null;
	}
}

export function play(name: string) {
	if (typeof window === "undefined") return;

	const ac = getContext();

	const cached = buffers.get(name);
	if (cached) {
		const source = ac.createBufferSource();
		const gain = ac.createGain();
		gain.gain.value = volumes[name] ?? 0.25;
		source.buffer = cached;
		source.connect(gain).connect(ac.destination);
		source.start();
		return;
	}

	loadBuffer(name).then((buffer) => {
		if (!buffer) return;
		const source = ac.createBufferSource();
		const gain = ac.createGain();
		gain.gain.value = volumes[name] ?? 0.25;
		source.buffer = buffer;
		source.connect(gain).connect(ac.destination);
		source.start();
	});
}

export function preload(...names: string[]) {
	if (typeof window === "undefined") return;
	names.forEach((n) => loadBuffer(n));
}
