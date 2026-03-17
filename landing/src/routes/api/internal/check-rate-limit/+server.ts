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
				// Reset 4h window
				tokensLast4h: sql`CASE WHEN ${rateLimits.lastReset4h} < now() - interval '4 hours' THEN 0 ELSE ${rateLimits.tokensLast4h} END`,
				lastReset4h: sql`CASE WHEN ${rateLimits.lastReset4h} < now() - interval '4 hours' THEN now() ELSE ${rateLimits.lastReset4h} END`,
				// Reset weekly window
				tokensThisWeek: sql`CASE WHEN ${rateLimits.lastResetWeekly} < date_trunc('week', CURRENT_DATE) THEN 0 ELSE ${rateLimits.tokensThisWeek} END`,
				lastResetWeekly: sql`CASE WHEN ${rateLimits.lastResetWeekly} < date_trunc('week', CURRENT_DATE) THEN now() ELSE ${rateLimits.lastResetWeekly} END`,
				// Reset monthly window
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

	if (tenant.tokensPer4h > 0 && t4h >= tenant.tokensPer4h) {
		return json({ allowed: false, reason: `4-hour token limit reached (${tenant.tokensPer4h.toLocaleString()}). Resets every 4 hours.` });
	}
	if (tenant.tokensPerWeek > 0 && tw >= tenant.tokensPerWeek) {
		return json({ allowed: false, reason: `weekly token limit reached (${tenant.tokensPerWeek.toLocaleString()}). Resets Monday.` });
	}
	if (tenant.tokensPerMonth > 0 && tm >= tenant.tokensPerMonth) {
		return json({ allowed: false, reason: `monthly token limit reached (${tenant.tokensPerMonth.toLocaleString()}). Resets 1st of month.` });
	}

	return json({ allowed: true });
};
