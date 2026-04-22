import type { BudgetState } from "../types.js";
import { type SpendDelta, loadDaily, recordSpend } from "./ledger.js";

export type ChargeContext = {
  home: string;
  slug: string;
  day: string;
  capUsd: number;
  tier: 2 | 3;
};

export type CallSuccess<T> = {
  ok: true;
  value?: T;
  usage: SpendDelta;
};

export type CallDowngraded = {
  downgraded: true;
  reason: string;
};

export type CallOutcome<T> = CallSuccess<T> | CallDowngraded;

/**
 * Wraps an LLM call with budget enforcement.
 * - Loads today's ledger.
 * - For tier 3 when state is suppressed, returns a downgraded outcome without invoking fn.
 * - Otherwise calls fn, records spend, returns success.
 */
export async function chargeAndCall<T>(
  ctx: ChargeContext,
  fn: (state: BudgetState) => Promise<CallSuccess<T>>,
): Promise<CallOutcome<T>> {
  const ledger = await loadDaily(ctx.home, ctx.slug, ctx.day, ctx.capUsd);

  if (ctx.tier === 3 && ledger.state === "suppressed") {
    return { downgraded: true, reason: "budget_cap" };
  }

  const result = await fn(ledger.state);
  await recordSpend(ctx.home, ctx.slug, ctx.day, ctx.capUsd, result.usage);
  return result;
}
