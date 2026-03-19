/** Reactive voice state — shared between ChatView, TTS engine, and blob. */

class VoiceState {
	enabled = $state(false);
	speaking = $state(false);
	/** Audio amplitude 0-1, updated each animation frame while speaking. */
	amplitude = $state(0);
	/** Word reveal progress 0-1, updated each animation frame while speaking. */
	revealProgress = $state(1);
	/** IDs of messages currently being spoken. */
	speakingIds = $state<Set<string>>(new Set());

	constructor() {
		if (typeof localStorage !== "undefined") {
			this.enabled = localStorage.getItem("bolly:voice") === "true";
		}
	}

	toggle() {
		this.enabled = !this.enabled;
		if (typeof localStorage !== "undefined") {
			localStorage.setItem("bolly:voice", String(this.enabled));
		}
	}
}

let instance: VoiceState | undefined;

export function getVoiceState(): VoiceState {
	if (!instance) instance = new VoiceState();
	return instance;
}
