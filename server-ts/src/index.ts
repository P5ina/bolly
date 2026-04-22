export const VERSION = "1.0.0-alpha.0";

export * from "./types.js";
export * from "./paths.js";
export { atomicWrite } from "./fs-atomic.js";
export { readJson, writeJson } from "./json-file.js";
export { readToml } from "./toml-file.js";

export { parseSkill } from "./skills/parse.js";
export { loadSkills, type LoadSkillsOptions } from "./skills/loader.js";

export { loadTriageRules, DEFAULT_TRIAGE_TEMPLATE } from "./triage/rules.js";
export {
  buildTriagePrompt,
  type OutreachHint,
  type TriagePromptInputs,
} from "./triage/prompt.js";

export { loadSettings, DEFAULT_SETTINGS, type Settings } from "./settings/reader.js";

export { appendOutreach, readRecentOutreach } from "./outreach/audit.js";

export { computeState, TIGHT_THRESHOLD } from "./budget/state.js";
export {
  loadDaily,
  recordSpend,
  recordSpendFromLoaded,
  todayUtc,
  type SpendDelta,
} from "./budget/ledger.js";
export {
  chargeAndCall,
  type ChargeContext,
  type CallSuccess,
  type CallDowngraded,
  type CallOutcome,
} from "./budget/charge-and-call.js";
export { Throttle, type ThrottleConfig } from "./budget/throttle.js";
