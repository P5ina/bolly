import type { BudgetState } from "../types.js";

export const TIGHT_THRESHOLD = 0.7;

/**
 * Classify current spend vs cap into one of three budget states.
 * Pure function; no side effects.
 */
export function computeState(dollarsSpent: number, capUsd: number): BudgetState {
  if (capUsd <= 0) return "suppressed";
  const ratio = dollarsSpent / capUsd;
  if (ratio >= 1) return "suppressed";
  if (ratio >= TIGHT_THRESHOLD) return "tight";
  return "ok";
}
