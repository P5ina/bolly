import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { authenticateTenant } from '$lib/server/internal-auth.js';
import { db } from '$lib/server/db/index.js';
import { rateLimits } from '$lib/server/db/schema.js';
import { eq, sql } from 'drizzle-orm';

// Derive sub-limits from monthly budget
// 4h budget = monthly / 60 (~50K for 3M plan, enough for 10-20 tool-heavy messages)
// weekly budget = monthly / 4
function deriveLimits(monthly: number) {
	return {
		budget4h: Math.floor(monthly / 60),
		budgetWeek: Math.floor(monthly / 4),
	};
}

export const POST: RequestHandler = async ({ request }) => {
	const tenant = await authenticateTenant(request);

	// BYOK — no rate limits
	if (tenant.byokApiKey) {
		return json({ allowed: true });
	}

	const monthly = tenant.tokensPerMonth;
	if (monthly <= 0) {
		return json({ allowed: true });
	}

	const { budget4h, budgetWeek } = deriveLimits(monthly);

	// Upsert + reset stale windows with rollover
	const [row] = await db()
		.insert(rateLimits)
		.values({
			instanceId: tenant.id,
			tokensLast4h: 0,
			tokensThisWeek: 0,
			tokensThisMonth: 0,
			rollover4h: 0,
		})
		.onConflictDoUpdate({
			target: rateLimits.instanceId,
			set: {
				// 4h window: on reset, carry unused tokens (capped at 2x budget)
				rollover4h: sql`CASE
					WHEN ${rateLimits.lastReset4h} < now() - interval '4 hours'
					THEN LEAST(GREATEST(${budget4h} + ${rateLimits.rollover4h} - ${rateLimits.tokensLast4h}, 0), ${budget4h * 2})
					ELSE ${rateLimits.rollover4h}
				END`,
				tokensLast4h: sql`CASE
					WHEN ${rateLimits.lastReset4h} < now() - interval '4 hours' THEN 0
					ELSE ${rateLimits.tokensLast4h}
				END`,
				lastReset4h: sql`CASE
					WHEN ${rateLimits.lastReset4h} < now() - interval '4 hours' THEN now()
					ELSE ${rateLimits.lastReset4h}
				END`,
				// Weekly reset
				tokensThisWeek: sql`CASE
					WHEN ${rateLimits.lastResetWeekly} < date_trunc('week', CURRENT_DATE) THEN 0
					ELSE ${rateLimits.tokensThisWeek}
				END`,
				lastResetWeekly: sql`CASE
					WHEN ${rateLimits.lastResetWeekly} < date_trunc('week', CURRENT_DATE) THEN now()
					ELSE ${rateLimits.lastResetWeekly}
				END`,
				// Monthly reset
				tokensThisMonth: sql`CASE
					WHEN ${rateLimits.lastResetMonthly} < date_trunc('month', CURRENT_DATE) THEN 0
					ELSE ${rateLimits.tokensThisMonth}
				END`,
				lastResetMonthly: sql`CASE
					WHEN ${rateLimits.lastResetMonthly} < date_trunc('month', CURRENT_DATE) THEN now()
					ELSE ${rateLimits.lastResetMonthly}
				END`,
			},
		})
		.returning({
			tokensLast4h: rateLimits.tokensLast4h,
			tokensThisWeek: rateLimits.tokensThisWeek,
			tokensThisMonth: rateLimits.tokensThisMonth,
			rollover4h: rateLimits.rollover4h,
		});

	const t4h = row?.tokensLast4h ?? 0;
	const tw = row?.tokensThisWeek ?? 0;
	const tm = row?.tokensThisMonth ?? 0;
	const rollover = row?.rollover4h ?? 0;

	// Effective 4h limit = base budget + rollover from unused previous windows
	const effective4h = budget4h + rollover;

	if (t4h >= effective4h) {
		return json({ allowed: false, reason: `4-hour limit reached (${effective4h.toLocaleString()} tokens). Resets every 4 hours. Unused tokens roll over.` });
	}
	if (tw >= budgetWeek) {
		return json({ allowed: false, reason: `Weekly limit reached (${budgetWeek.toLocaleString()} tokens). Resets Monday.` });
	}
	if (tm >= monthly) {
		return json({ allowed: false, reason: `Monthly limit reached (${monthly.toLocaleString()} tokens). Resets 1st of month.` });
	}

	return json({ allowed: true });
};
