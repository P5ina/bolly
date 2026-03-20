/** Reactive voice state — shared between ChatView, TTS engine, and blob. */

import { fetchVoiceEnabled, updateVoiceEnabled } from "$lib/api/client.js";

class VoiceState {
	enabled = $state(false);
	speaking = $state(false);
	/** Audio amplitude 0-1, updated each animation frame while speaking. */
	amplitude = $state(0);
	/** Word reveal progress 0-1, updated each animation frame while speaking. */
	revealProgress = $state(1);
	/** IDs of messages currently being spoken. */
	speakingIds = $state<Set<string>>(new Set());
	/** Current instance slug for server sync. */
	private slug = "";

	/** Load voice_enabled from server for a given instance. */
	async loadForInstance(slug: string) {
		this.slug = slug;
		try {
			const res = await fetchVoiceEnabled(slug);
			this.enabled = res.voice_enabled;
		} catch {
			// fall back to current state
		}
	}

	toggle() {
		this.enabled = !this.enabled;
		// Persist to server (fire-and-forget)
		if (this.slug) {
			updateVoiceEnabled(this.slug, this.enabled).catch(() => {});
		}
	}
}

let instance: VoiceState | undefined;

export function getVoiceState(): VoiceState {
	if (!instance) instance = new VoiceState();
	return instance;
}
