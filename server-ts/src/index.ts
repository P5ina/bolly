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

// Plan 2 — Mind runtime
export { loadConfig, type RuntimeConfig } from "./config.js";

export {
  loadConversation,
  saveConversation,
  appendConversationEntry,
} from "./conversation/store.js";
export {
  ContentBlockSchema,
  ConversationEntrySchema,
  ConversationSchema,
  type ContentBlock,
  type ConversationEntry,
  type Conversation,
} from "./conversation/types.js";

export { buildSystemPrompt, type SystemPromptInputs } from "./mind/system-prompt.js";
export {
  skillToTool,
  skillToToolName,
  builtInTools,
  type ToolDefinition,
} from "./mind/skill-tool.js";
export { createAnthropicClient, type MindClient } from "./mind/anthropic-client.js";
export { MockAnthropicClient, type MockMessage, type MockStream } from "./mind/mock-client.js";
export {
  runMindTurn,
  runMindWithTools,
  runMindStreaming,
  type MindTurnInputs,
  type MindTurnResult,
  type MindFullTurnInputs,
  type MindFullTurnResult,
  type MindStreamingInputs,
  type MindStreamingResult,
} from "./mind/loop.js";
export { runBudgetedMind, type BudgetedMindResult } from "./mind/budgeted-mind.js";
export { MindWorker, type MindWorkerOptions } from "./mind/worker.js";
export { WorkerPool, type WorkerPoolOptions } from "./mind/pool.js";

export { Broadcaster, type Subscriber } from "./events/broadcaster.js";
export {
  serializeServerEvent,
  type ServerEvent,
  type ChatMessage,
} from "./events/server-event.js";

export { createApp, type AppOptions } from "./http/server.js";
export { requireAuth } from "./http/auth.js";
