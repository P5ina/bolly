import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { authenticateTenant } from '$lib/server/internal-auth.js';
import { db } from '$lib/server/db/index.js';
import { rateLimits } from '$lib/server/db/schema.js';
import { eq, sql } from 'drizzle-orm';

export const POST: RequestHandler = async ({ request }) => {
	const tenant = await authenticateTenant(request);

	// BYOK — no rate limits
	if (tenant.byokApiKey) {
		return json({ allowed: true });
	}

	// Upsert + reset stale monthly counter
	const [row] = await db()
		.insert(rateLimits)
		.values({
			instanceId: tenant.id,
			messagesToday: 0,
			tokensThisMonth: 0,
		})
		.onConflictDoUpdate({
			target: rateLimits.instanceId,
			set: {
				tokensThisMonth: sql`CASE WHEN ${rateLimits.lastResetMonthly} < date_trunc('month', CURRENT_DATE) THEN 0 ELSE ${rateLimits.tokensThisMonth} END`,
				lastResetMonthly: sql`CASE WHEN ${rateLimits.lastResetMonthly} < date_trunc('month', CURRENT_DATE) THEN now() ELSE ${rateLimits.lastResetMonthly} END`,
			},
		})
		.returning({ tokensThisMonth: rateLimits.tokensThisMonth });

	const tokensThisMonth = row?.tokensThisMonth ?? 0;
	const limit = tenant.tokensPerMonth;

	if (limit > 0 && tokensThisMonth >= limit) {
		return json({ allowed: false, reason: `monthly token limit reached (${limit})` });
	}

	return json({ allowed: true });
};
