/** Presentation mode state — fullscreen cinematic chat for demos. */

class PresentationState {
	active = $state(false);

	constructor() {
		if (typeof window !== "undefined") {
			this.active = new URLSearchParams(window.location.search).has("present");
		}
	}

	toggle() {
		this.active = !this.active;
	}
}

let instance: PresentationState | undefined;

export function getPresentationState(): PresentationState {
	if (!instance) instance = new PresentationState();
	return instance;
}
