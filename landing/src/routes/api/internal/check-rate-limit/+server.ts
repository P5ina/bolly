import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { authenticateTenant } from '$lib/server/internal-auth.js';
import { db } from '$lib/server/db/index.js';
import { rateLimits } from '$lib/server/db/schema.js';
import { sql } from 'drizzle-orm';

export const POST: RequestHandler = async ({ request }) => {
	const tenant = await authenticateTenant(request);

	// BYOK — no rate limits
	if (tenant.byokApiKey) {
		return json({ allowed: true });
	}

	const limit4h = tenant.tokensPer4h;
	const limitWeek = Math.floor(tenant.tokensPerMonth / 4);
	const limitMonth = tenant.tokensPerMonth;

	// Upsert + reset stale windows
	const [row] = await db()
		.insert(rateLimits)
		.values({
			instanceId: tenant.id,
			tokensLast4h: 0,
			tokensThisWeek: 0,
			tokensThisMonth: 0,
		})
		.onConflictDoUpdate({
			target: rateLimits.instanceId,
			set: {
				tokensLast4h: sql`CASE WHEN ${rateLimits.lastReset4h} < now() - interval '4 hours' THEN 0 ELSE ${rateLimits.tokensLast4h} END`,
				lastReset4h: sql`CASE WHEN ${rateLimits.lastReset4h} < now() - interval '4 hours' THEN now() ELSE ${rateLimits.lastReset4h} END`,
				tokensThisWeek: sql`CASE WHEN ${rateLimits.lastResetWeekly} < date_trunc('week', CURRENT_DATE) THEN 0 ELSE ${rateLimits.tokensThisWeek} END`,
				lastResetWeekly: sql`CASE WHEN ${rateLimits.lastResetWeekly} < date_trunc('week', CURRENT_DATE) THEN now() ELSE ${rateLimits.lastResetWeekly} END`,
				tokensThisMonth: sql`CASE WHEN ${rateLimits.lastResetMonthly} < date_trunc('month', CURRENT_DATE) THEN 0 ELSE ${rateLimits.tokensThisMonth} END`,
				lastResetMonthly: sql`CASE WHEN ${rateLimits.lastResetMonthly} < date_trunc('month', CURRENT_DATE) THEN now() ELSE ${rateLimits.lastResetMonthly} END`,
			},
		})
		.returning({
			tokensLast4h: rateLimits.tokensLast4h,
			tokensThisWeek: rateLimits.tokensThisWeek,
			tokensThisMonth: rateLimits.tokensThisMonth,
		});

	const t4h = row?.tokensLast4h ?? 0;
	const tw = row?.tokensThisWeek ?? 0;
	const tm = row?.tokensThisMonth ?? 0;

	if (limit4h > 0 && t4h >= limit4h) {
		return json({ allowed: false, reason: `4-hour limit reached (${limit4h.toLocaleString()} tokens). Resets every 4 hours.` });
	}
	if (limitWeek > 0 && tw >= limitWeek) {
		return json({ allowed: false, reason: `Weekly limit reached (${limitWeek.toLocaleString()} tokens). Resets Monday.` });
	}
	if (limitMonth > 0 && tm >= limitMonth) {
		return json({ allowed: false, reason: `Monthly limit reached (${limitMonth.toLocaleString()} tokens). Resets 1st of month.` });
	}

	return json({ allowed: true });
};
