/**
 * Haptic feedback via Vibration API.
 * Silently no-ops on unsupported devices (desktop, iOS Safari).
 */

function vibrate(pattern: number | number[]) {
	if (typeof navigator === "undefined") return;
	if (!("vibrate" in navigator)) return;
	try {
		navigator.vibrate(pattern);
	} catch {
		// ignore
	}
}

/** Short tap — sending a message, pressing a button */
export function hapticLight() {
	vibrate(10);
}

/** Medium — receiving a message, attachment added */
export function hapticMedium() {
	vibrate(20);
}

/** Double tap — drop received, mood shift */
export function hapticDouble() {
	vibrate([12, 60, 12]);
}

/** Error — something went wrong */
export function hapticError() {
	vibrate([30, 50, 30]);
}

/** Soft reveal — onboarding intro */
export function hapticReveal() {
	vibrate([8, 40, 8, 40, 8]);
}
