import type { Handle } from '@sveltejs/kit';
import { getSessionId, validateSession } from '$lib/server/auth/index.js';

export const handle: Handle = async ({ event, resolve }) => {
	// Try cookie first, then Authorization: Bearer header (for desktop app)
	let sessionId = getSessionId(event.cookies);
	if (!sessionId) {
		const auth = event.request.headers.get('authorization');
		if (auth?.startsWith('Bearer ')) {
			sessionId = auth.slice(7);
		}
	}

	if (sessionId) {
		const result = await validateSession(sessionId);
		if (result) {
			event.locals.user = result.user;
			event.locals.sessionId = result.sessionId;
		}
	}

	return resolve(event);
};
