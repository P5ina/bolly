import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { refreshAccessToken } from '$lib/server/google.js';
import { db } from '$lib/server/db/index.js';
import { googleAccounts, tenants } from '$lib/server/db/schema.js';
import { eq } from 'drizzle-orm';

/**
 * GET /api/google-token
 * Called by the Rust server to get a fresh Google access token.
 * Auth: Bearer {BOLLY_AUTH_TOKEN} (the tenant's auth token)
 */
export const GET: RequestHandler = async ({ request }) => {
	const auth = request.headers.get('authorization');
	if (!auth?.startsWith('Bearer ')) {
		throw error(401, 'Missing auth token');
	}
	const authToken = auth.slice(7);

	// Find the tenant by auth token → get userId
	const tenant = await db()
		.select()
		.from(tenants)
		.where(eq(tenants.authToken, authToken))
		.limit(1);

	if (tenant.length === 0) {
		throw error(401, 'Invalid auth token');
	}

	const userId = tenant[0].userId;

	// Get Google account for this user
	const rows = await db()
		.select()
		.from(googleAccounts)
		.where(eq(googleAccounts.userId, userId))
		.limit(1);

	if (rows.length === 0) {
		throw error(404, 'No Google account connected');
	}

	const account = rows[0];

	// Refresh if expired (or within 60s of expiry)
	const now = new Date();
	const expiryBuffer = new Date(account.expiresAt.getTime() - 60_000);

	let accessToken = account.accessToken;

	if (now >= expiryBuffer) {
		try {
			const refreshed = await refreshAccessToken(account.refreshToken);
			accessToken = refreshed.access_token;
			const newExpiry = new Date(Date.now() + refreshed.expires_in * 1000);

			await db()
				.update(googleAccounts)
				.set({ accessToken, expiresAt: newExpiry })
				.where(eq(googleAccounts.id, account.id));
		} catch (e) {
			console.error('Google token refresh failed:', e);
			throw error(502, 'Failed to refresh Google token');
		}
	}

	return json({ access_token: accessToken, email: account.email });
};
