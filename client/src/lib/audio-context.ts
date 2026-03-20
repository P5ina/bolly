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
	if (ctx?.state === "suspended") ctx.resume();
}

// Auto-resume on user interaction
if (typeof document !== "undefined") {
	const unlock = () => {
		resumeAudioContext();
		document.removeEventListener("click", unlock);
		document.removeEventListener("touchstart", unlock);
		document.removeEventListener("keydown", unlock);
	};
	document.addEventListener("click", unlock, { capture: true });
	document.addEventListener("touchstart", unlock, { capture: true });
	document.addEventListener("keydown", unlock, { capture: true });

	document.addEventListener("visibilitychange", () => {
		if (document.visibilityState === "visible") resumeAudioContext();
	});
}
