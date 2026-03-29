import { redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { GOOGLE_CLIENT_ID, ORIGIN } from '$env/static/private';
import { generateId } from '$lib/server/auth/index.js';

const GOOGLE_AUTH_URL = 'https://accounts.google.com/o/oauth2/v2/auth';
const SIGNIN_SCOPES = 'openid email profile';

export const GET: RequestHandler = async ({ url, cookies }) => {
	const redirectTo = url.searchParams.get('redirect') || '/dashboard';

	// CSRF state token
	const state = generateId(32);
	cookies.set('google_signin_state', state, {
		path: '/',
		httpOnly: true,
		maxAge: 600, // 10 minutes
		sameSite: 'lax',
	});
	cookies.set('google_signin_redirect', redirectTo, {
		path: '/',
		httpOnly: true,
		maxAge: 600,
		sameSite: 'lax',
	});

	const params = new URLSearchParams({
		client_id: GOOGLE_CLIENT_ID,
		redirect_uri: `${ORIGIN}/auth/google/signin/callback`,
		response_type: 'code',
		scope: SIGNIN_SCOPES,
		state,
		prompt: 'select_account',
	});

	redirect(302, `${GOOGLE_AUTH_URL}?${params.toString()}`);
};
