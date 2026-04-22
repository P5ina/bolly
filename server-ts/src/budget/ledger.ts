import { readJson, writeJson } from "../json-file.js";
import { budgetDailyFile } from "../paths.js";
import { type BudgetDaily, BudgetDailySchema } from "../types.js";
import { computeState } from "./state.js";

export function todayUtc(d: Date = new Date()): string {
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export async function loadDaily(
  home: string,
  slug: string,
  day: string,
  capUsd: number,
): Promise<BudgetDaily> {
  const path = budgetDailyFile(home, slug, day);
  const existing = await readJson(path, BudgetDailySchema);
  if (existing) return existing as BudgetDaily;

  return BudgetDailySchema.parse({
    day,
    calls: 0,
    tokens_in: 0,
    tokens_out: 0,
    dollars_spent: 0,
    cap_usd: capUsd,
    state: "ok",
  });
}

export type SpendDelta = {
  tokens_in: number;
  tokens_out: number;
  dollars: number;
};

export async function recordSpend(
  home: string,
  slug: string,
  day: string,
  capUsd: number,
  delta: SpendDelta,
): Promise<BudgetDaily> {
  const current = await loadDaily(home, slug, day, capUsd);
  return recordSpendFromLoaded(current, home, slug, day, capUsd, delta);
}

/**
 * Variant of {@link recordSpend} that accepts an already-loaded ledger.
 * Skips the disk read so callers that already have the ledger (e.g. chargeAndCall)
 * pay one read + one write instead of two reads + one write.
 */
export async function recordSpendFromLoaded(
  current: BudgetDaily,
  home: string,
  slug: string,
  day: string,
  capUsd: number,
  delta: SpendDelta,
): Promise<BudgetDaily> {
  const next: BudgetDaily = {
    day,
    calls: current.calls + 1,
    tokens_in: current.tokens_in + delta.tokens_in,
    tokens_out: current.tokens_out + delta.tokens_out,
    dollars_spent: current.dollars_spent + delta.dollars,
    cap_usd: capUsd,
    state: computeState(current.dollars_spent + delta.dollars, capUsd),
  };
  await writeJson(budgetDailyFile(home, slug, day), next);
  return next;
}
