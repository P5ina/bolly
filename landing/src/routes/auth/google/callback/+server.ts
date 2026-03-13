import { redirect, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { exchangeCode, getGoogleUserInfo } from '$lib/server/google.js';
import { generateId } from '$lib/server/auth/index.js';
import { db } from '$lib/server/db/index.js';
import { googleAccounts } from '$lib/server/db/schema.js';
import { eq } from 'drizzle-orm';
import { env } from '$env/dynamic/private';

export const GET: RequestHandler = async ({ url, cookies }) => {
	const code = url.searchParams.get('code');
	const stateParam = url.searchParams.get('state');
	const storedState = cookies.get('google_oauth_state');

	if (!code || !stateParam || !storedState || stateParam !== storedState) {
		throw error(400, 'Invalid OAuth state');
	}

	cookies.delete('google_oauth_state', { path: '/' });

	// Decode state to get userId
	let userId: string;
	try {
		const payload = JSON.parse(Buffer.from(stateParam, 'base64url').toString());
		userId = payload.userId;
		if (!userId) throw new Error('missing userId');
	} catch {
		throw error(400, 'Invalid state payload');
	}

	const origin = env.ORIGIN ?? url.origin;
	const redirectUri = `${origin}/auth/google/callback`;

	const tokens = await exchangeCode(code, redirectUri);
	const userInfo = await getGoogleUserInfo(tokens.access_token);

	const expiresAt = new Date(Date.now() + tokens.expires_in * 1000);

	// Upsert: delete existing Google account for this user, then insert
	await db().delete(googleAccounts).where(eq(googleAccounts.userId, userId));
	await db().insert(googleAccounts).values({
		id: generateId(),
		userId,
		email: userInfo.email,
		accessToken: tokens.access_token,
		refreshToken: tokens.refresh_token,
		expiresAt,
		scopes: tokens.scope,
	});

	redirect(302, '/dashboard');
};
