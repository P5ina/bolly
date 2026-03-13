import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { revokeToken } from '$lib/server/google.js';
import { db } from '$lib/server/db/index.js';
import { googleAccounts, tenants } from '$lib/server/db/schema.js';
import { eq, and } from 'drizzle-orm';

/**
 * POST /dashboard/disconnect-google
 * Body: { token, instance, email }
 * Disconnects a specific Google account from an instance.
 */
export const POST: RequestHandler = async ({ request }) => {
	let token: string;
	let instance: string;
	let email: string;

	try {
		const body = await request.json();
		token = body.token;
		instance = body.instance;
		email = body.email;
	} catch {
		throw error(400, 'Invalid request body');
	}

	if (!token || !instance || !email) {
		throw error(400, 'Missing token, instance, or email');
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

	const tenantId = tenant[0].id;

	// Find the specific Google account
	const rows = await db()
		.select()
		.from(googleAccounts)
		.where(
			and(
				eq(googleAccounts.tenantId, tenantId),
				eq(googleAccounts.instanceSlug, instance),
				eq(googleAccounts.email, email),
			)
		)
		.limit(1);

	if (rows.length > 0) {
		// Best-effort revoke at Google
		try {
			await revokeToken(rows[0].accessToken);
		} catch {
			// ignore — token may already be expired
		}
		await db().delete(googleAccounts).where(eq(googleAccounts.id, rows[0].id));
	}

	return json({ ok: true });
};
