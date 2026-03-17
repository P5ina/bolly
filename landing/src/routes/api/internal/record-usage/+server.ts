import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { authenticateTenant } from '$lib/server/internal-auth.js';
import { db } from '$lib/server/db/index.js';
import { rateLimits } from '$lib/server/db/schema.js';
import { sql } from 'drizzle-orm';

export const POST: RequestHandler = async ({ request }) => {
	const tenant = await authenticateTenant(request);
	const body = await request.json();
	const tokens = typeof body.tokens === 'number' ? body.tokens : 0;

	if (tokens <= 0) throw error(400, 'Invalid token count');

	await db()
		.insert(rateLimits)
		.values({
			instanceId: tenant.id,
			tokensLast4h: tokens,
			tokensThisWeek: tokens,
			tokensThisMonth: tokens,
		})
		.onConflictDoUpdate({
			target: rateLimits.instanceId,
			set: {
				tokensLast4h: sql`${rateLimits.tokensLast4h} + ${tokens}`,
				tokensThisWeek: sql`${rateLimits.tokensThisWeek} + ${tokens}`,
				tokensThisMonth: sql`${rateLimits.tokensThisMonth} + ${tokens}`,
			},
		});

	return json({ ok: true });
};
