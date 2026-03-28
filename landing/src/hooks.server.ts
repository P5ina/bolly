import type { Handle } from '@sveltejs/kit';
import { getSessionId, validateSession } from '$lib/server/auth/index.js';

export const handle: Handle = async ({ event, resolve }) => {
	// CORS preflight for desktop app (Bearer auth from tauri:// origin)
	if (event.request.method === 'OPTIONS') {
		const origin = event.request.headers.get('origin') ?? '';
		if (origin.startsWith('tauri://') || origin.startsWith('http://localhost')) {
			return new Response(null, {
				headers: {
					'Access-Control-Allow-Origin': origin,
					'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
					'Access-Control-Allow-Headers': 'Authorization, Content-Type',
					'Access-Control-Max-Age': '86400',
				},
			});
		}
	}

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

	const response = await resolve(event);

	// Add CORS headers for desktop app requests
	const origin = event.request.headers.get('origin') ?? '';
	if (origin.startsWith('tauri://') || origin.startsWith('http://localhost')) {
		response.headers.set('Access-Control-Allow-Origin', origin);
		response.headers.set('Access-Control-Allow-Headers', 'Authorization, Content-Type');
	}

	return response;
};
