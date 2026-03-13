import { redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { getGoogleAuthUrl } from '$lib/server/google.js';
import { generateId } from '$lib/server/auth/index.js';
import { env } from '$env/dynamic/private';

export const GET: RequestHandler = async ({ locals, cookies, url }) => {
	if (!locals.user) redirect(302, '/login');

	const origin = env.ORIGIN ?? url.origin;
	const redirectUri = `${origin}/auth/google/callback`;
	const state = generateId(32);

	// Encode userId in state so callback can associate the tokens
	const statePayload = JSON.stringify({ userId: locals.user.id, nonce: state });
	const stateEncoded = Buffer.from(statePayload).toString('base64url');

	cookies.set('google_oauth_state', stateEncoded, {
		path: '/',
		httpOnly: true,
		secure: origin.startsWith('https'),
		sameSite: 'lax',
		maxAge: 600, // 10 minutes
	});

	const authUrl = getGoogleAuthUrl(redirectUri, stateEncoded);
	redirect(302, authUrl);
};
