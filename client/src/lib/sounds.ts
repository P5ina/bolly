const cache = new Map<string, HTMLAudioElement>();

const volumes: Record<string, number> = {
	message_send: 0.3,
	message_receive: 0.25,
	attachment_added: 0.25,
	mood_shift: 0.15,
};

export function play(name: string) {
	if (typeof window === "undefined") return;

	let audio = cache.get(name);
	if (!audio) {
		audio = new Audio(`/sounds/${name}.mp3`);
		cache.set(name, audio);
	}

	audio.volume = volumes[name] ?? 0.25;
	audio.currentTime = 0;
	audio.play().catch(() => {});
}
