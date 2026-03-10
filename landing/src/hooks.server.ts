import type { Handle } from '@sveltejs/kit';
import { getSessionId, validateSession } from '$lib/server/auth/index.js';

export const handle: Handle = async ({ event, resolve }) => {
	const sessionId = getSessionId(event.cookies);

	if (sessionId) {
		const result = await validateSession(sessionId);
		if (result) {
			event.locals.user = result.user;
			event.locals.sessionId = result.sessionId;
		}
	}

	return resolve(event);
};
