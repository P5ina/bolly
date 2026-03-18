import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { refreshAccessToken } from '$lib/server/google.js';
import { db } from '$lib/server/db/index.js';
import { googleAccounts, tenants } from '$lib/server/db/schema.js';
import { eq, and } from 'drizzle-orm';

/**
 * GET /api/google-token?instance={slug}
 * Called by the Rust server to get fresh Google access tokens.
 * Auth: Bearer {BOLLY_AUTH_TOKEN} (the tenant's auth token)
 * Returns: { accounts: [{ access_token, email }, ...] }
 */
export const GET: RequestHandler = async ({ request, url }) => {
	const auth = request.headers.get('authorization');
	if (!auth?.startsWith('Bearer ')) {
		throw error(401, 'Missing auth token');
	}
	const authToken = auth.slice(7);

	// Find the tenant by auth token
	const tenant = await db()
		.select()
		.from(tenants)
		.where(eq(tenants.authToken, authToken))
		.limit(1);

	if (tenant.length === 0) {
		throw error(401, 'Invalid auth token');
	}

	const tenantId = tenant[0].id;
	const instance = url.searchParams.get('instance') ?? 'default';

	// Get all Google accounts for this tenant + instance
	const rows = await db()
		.select()
		.from(googleAccounts)
		.where(
			and(
				eq(googleAccounts.tenantId, tenantId),
				eq(googleAccounts.instanceSlug, instance),
			)
		);

	if (rows.length === 0) {
		throw error(404, 'No Google accounts connected');
	}

	// Refresh expired tokens and collect results
	const accounts: { access_token: string; email: string; scopes: string }[] = [];

	for (const account of rows) {
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
				console.error(`Google token refresh failed for ${account.email}:`, e);
				continue; // Skip this account, return others
			}
		}

		accounts.push({ access_token: accessToken, email: account.email, scopes: account.scopes });
	}

	if (accounts.length === 0) {
		throw error(502, 'Failed to refresh any Google tokens');
	}

	return json({ accounts });
};
