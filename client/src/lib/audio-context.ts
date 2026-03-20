/**
 * Single shared AudioContext for the entire application.
 * MDN best practice: one AudioContext per page.
 * https://developer.mozilla.org/en-US/docs/Web/API/Web_Audio_API/Best_practices
 */

let ctx: AudioContext | null = null;

export function getAudioContext(): AudioContext {
	if (!ctx) {
		ctx = new AudioContext();
	}
	return ctx;
}

/** Resume the AudioContext (call from user gesture to satisfy autoplay policy). */
export function resumeAudioContext(): void {
	// Create context eagerly if it doesn't exist yet — user gesture is
	// the best time to create it (guaranteed to start in "running" state).
	const ac = getAudioContext();
	if (ac.state === "suspended") ac.resume();
}

// Auto-resume on user interaction — DON'T remove listeners after first click,
// because the context may get suspended again (e.g. tab backgrounded).
if (typeof document !== "undefined") {
	const unlock = () => resumeAudioContext();
	document.addEventListener("click", unlock, { capture: true });
	document.addEventListener("touchstart", unlock, { capture: true });
	document.addEventListener("keydown", unlock, { capture: true });

	document.addEventListener("visibilitychange", () => {
		if (document.visibilityState === "visible") resumeAudioContext();
	});
}
