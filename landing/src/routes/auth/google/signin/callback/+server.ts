import { redirect, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { GOOGLE_CLIENT_SECRET, GOOGLE_CLIENT_ID, ORIGIN } from '$env/static/private';
import { generateId, createSession, setSessionCookie, hashPassword } from '$lib/server/auth/index.js';
import { db } from '$lib/server/db/index.js';
import { users } from '$lib/server/db/schema.js';
import { eq } from 'drizzle-orm';

const GOOGLE_TOKEN_URL = 'https://oauth2.googleapis.com/token';
const GOOGLE_USERINFO_URL = 'https://www.googleapis.com/oauth2/v2/userinfo';

export const GET: RequestHandler = async ({ url, cookies }) => {
	const code = url.searchParams.get('code');
	const state = url.searchParams.get('state');
	const storedState = cookies.get('google_signin_state');
	const redirectTo = cookies.get('google_signin_redirect') || '/dashboard';

	// Clean up cookies
	cookies.delete('google_signin_state', { path: '/' });
	cookies.delete('google_signin_redirect', { path: '/' });

	if (!code || !state || !storedState || state !== storedState) {
		throw error(400, 'Invalid OAuth state');
	}

	// Exchange code for tokens
	const tokenRes = await fetch(GOOGLE_TOKEN_URL, {
		method: 'POST',
		headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
		body: new URLSearchParams({
			code,
			client_id: GOOGLE_CLIENT_ID,
			client_secret: GOOGLE_CLIENT_SECRET,
			redirect_uri: `${ORIGIN}/auth/google/signin/callback`,
			grant_type: 'authorization_code',
		}),
	});

	if (!tokenRes.ok) {
		const text = await tokenRes.text();
		console.error('Google token exchange failed:', text);
		throw error(400, 'Google authentication failed');
	}

	const tokens = await tokenRes.json();

	// Get user info
	const infoRes = await fetch(GOOGLE_USERINFO_URL, {
		headers: { Authorization: `Bearer ${tokens.access_token}` },
	});

	if (!infoRes.ok) {
		throw error(400, 'Failed to get Google user info');
	}

	const googleUser: { email: string; name?: string } = await infoRes.json();
	const email = googleUser.email.toLowerCase().trim();

	// Find or create user
	let [user] = await db()
		.select()
		.from(users)
		.where(eq(users.email, email))
		.limit(1);

	if (!user) {
		// Create new user — auto-verified since Google confirmed the email
		const id = generateId();
		const randomPassword = generateId(40);
		[user] = await db()
			.insert(users)
			.values({
				id,
				email,
				name: googleUser.name || null,
				passwordHash: hashPassword(randomPassword),
				emailVerified: true,
			})
			.returning();
	} else if (!user.emailVerified) {
		// User exists but email not verified — mark as verified (Google confirmed it)
		await db()
			.update(users)
			.set({ emailVerified: true })
			.where(eq(users.id, user.id));
	}

	// Create session
	const sessionId = await createSession(user.id);
	setSessionCookie(cookies, sessionId);

	redirect(302, redirectTo);
};
