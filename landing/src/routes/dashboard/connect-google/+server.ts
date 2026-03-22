import { redirect, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { getGoogleAuthUrl } from '$lib/server/google.js';
import { generateId } from '$lib/server/auth/index.js';
import { db } from '$lib/server/db/index.js';
import { tenants } from '$lib/server/db/schema.js';
import { eq } from 'drizzle-orm';
import { ORIGIN } from '$env/static/private';

/**
 * GET /dashboard/connect-google?token={tenantAuthToken}&instance={slug}&redirect={clientUrl}
 * Initiates Google OAuth flow. Auth via tenant token (no login required).
 */
export const GET: RequestHandler = async ({ url, cookies }) => {
	const token = url.searchParams.get('token');
	const instance = url.searchParams.get('instance') ?? 'default';
	const clientRedirect = url.searchParams.get('redirect') ?? '/dashboard';

	if (!token) {
		throw error(400, 'Missing token parameter');
	}

	// Validate tenant auth token
	const tenant = await db()
		.select()
		.from(tenants)
		.where(eq(tenants.authToken, token))
		.limit(1);

	if (tenant.length === 0) {
		throw error(401, 'Invalid auth token');
	}

	const origin = ORIGIN;
	const redirectUri = `${origin}/auth/google/callback`;
	const nonce = generateId(32);

	// Encode tenant info in state so callback can associate the tokens
	const statePayload = JSON.stringify({
		tenantId: tenant[0].id,
		instanceSlug: instance,
		redirect: clientRedirect,
		nonce,
	});
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
