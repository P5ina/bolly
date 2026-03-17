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
		return json({
			tokens_last_4h: 0, tokens_4h_limit: -1,
			tokens_this_week: 0, tokens_week_limit: -1,
			tokens_this_month: 0, tokens_month_limit: -1,
		});
	}

	const monthly = tenant.tokensPerMonth;
	const budget4h = Math.floor(monthly / 60);
	const budgetWeek = Math.floor(monthly / 4);

	const [row] = await db()
		.select({
			tokensLast4h: sql<number>`CASE WHEN ${rateLimits.lastReset4h} < now() - interval '4 hours' THEN 0 ELSE ${rateLimits.tokensLast4h} END`,
			tokensThisWeek: sql<number>`CASE WHEN ${rateLimits.lastResetWeekly} < date_trunc('week', CURRENT_DATE) THEN 0 ELSE ${rateLimits.tokensThisWeek} END`,
			tokensThisMonth: sql<number>`CASE WHEN ${rateLimits.lastResetMonthly} < date_trunc('month', CURRENT_DATE) THEN 0 ELSE ${rateLimits.tokensThisMonth} END`,
			rollover4h: sql<number>`CASE WHEN ${rateLimits.lastReset4h} < now() - interval '4 hours' THEN 0 ELSE ${rateLimits.rollover4h} END`,
		})
		.from(rateLimits)
		.where(eq(rateLimits.instanceId, tenant.id))
		.limit(1);

	const rollover = row?.rollover4h ?? 0;

	return json({
		tokens_last_4h: row?.tokensLast4h ?? 0,
		tokens_4h_limit: budget4h + rollover,
		tokens_this_week: row?.tokensThisWeek ?? 0,
		tokens_week_limit: budgetWeek,
		tokens_this_month: row?.tokensThisMonth ?? 0,
		tokens_month_limit: monthly,
	});
};
