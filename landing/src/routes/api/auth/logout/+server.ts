import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { invalidateSession, deleteSessionCookie } from '$lib/server/auth/index.js';

export const POST: RequestHandler = async ({ locals, cookies }) => {
	if (locals.sessionId) {
		await invalidateSession(locals.sessionId);
	}
	deleteSessionCookie(cookies);
	return json({ ok: true });
};
