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

	// Try cookie → Bearer header → ?session= query param (desktop app)
	let sessionId = getSessionId(event.cookies);
	if (!sessionId) {
		const auth = event.request.headers.get('authorization');
		if (auth?.startsWith('Bearer ')) {
			sessionId = auth.slice(7);
		}
	}
	if (!sessionId) {
		sessionId = event.url.searchParams.get('session') ?? undefined;
	}

	if (sessionId) {
		const result = await validateSession(sessionId);
		if (result) {
			event.locals.user = result.user;
			event.locals.sessionId = result.sessionId;
		} else {
			console.log(`[auth] invalid session via ${authSource}, token: ${sessionId.slice(0, 6)}...${sessionId.slice(-4)}, len: ${sessionId.length}`);
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
