import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { authenticateTenant } from '$lib/server/internal-auth.js';
import { db } from '$lib/server/db/index.js';
import { rateLimits } from '$lib/server/db/schema.js';
import { eq, sql } from 'drizzle-orm';

export const GET: RequestHandler = async ({ request }) => {
	const tenant = await authenticateTenant(request);

	// BYOK — unlimited
	if (tenant.byokApiKey) {
		return json({ tokens_this_month: 0, tokens_limit: -1 });
	}

	const [row] = await db()
		.select({
			tokensThisMonth: sql<number>`CASE WHEN ${rateLimits.lastResetMonthly} < date_trunc('month', CURRENT_DATE) THEN 0 ELSE ${rateLimits.tokensThisMonth} END`,
		})
		.from(rateLimits)
		.where(eq(rateLimits.instanceId, tenant.id))
		.limit(1);

	return json({
		tokens_this_month: row?.tokensThisMonth ?? 0,
		tokens_limit: tenant.tokensPerMonth,
	});
};
