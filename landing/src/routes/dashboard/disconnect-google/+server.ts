import { redirect, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { revokeToken } from '$lib/server/google.js';
import { db } from '$lib/server/db/index.js';
import { googleAccounts } from '$lib/server/db/schema.js';
import { eq } from 'drizzle-orm';

export const POST: RequestHandler = async ({ locals }) => {
	if (!locals.user) redirect(302, '/login');

	const rows = await db()
		.select()
		.from(googleAccounts)
		.where(eq(googleAccounts.userId, locals.user.id))
		.limit(1);

	if (rows.length > 0) {
		// Best-effort revoke at Google
		try {
			await revokeToken(rows[0].accessToken);
		} catch {
			// ignore — token may already be expired
		}
		await db().delete(googleAccounts).where(eq(googleAccounts.userId, locals.user.id));
	}

	redirect(302, '/dashboard');
};
