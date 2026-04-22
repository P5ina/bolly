import { describe, expect, it } from "vitest";
import * as api from "./index.js";

describe("public API", () => {
  it("re-exports the foundational modules", () => {
    expect(api.atomicWrite).toBeTypeOf("function");
    expect(api.readJson).toBeTypeOf("function");
    expect(api.writeJson).toBeTypeOf("function");
    expect(api.readToml).toBeTypeOf("function");
    expect(api.parseSkill).toBeTypeOf("function");
    expect(api.loadSkills).toBeTypeOf("function");
    expect(api.loadTriageRules).toBeTypeOf("function");
    expect(api.buildTriagePrompt).toBeTypeOf("function");
    expect(api.loadSettings).toBeTypeOf("function");
    expect(api.appendOutreach).toBeTypeOf("function");
    expect(api.readRecentOutreach).toBeTypeOf("function");
    expect(api.computeState).toBeTypeOf("function");
    expect(api.loadDaily).toBeTypeOf("function");
    expect(api.recordSpend).toBeTypeOf("function");
    expect(api.chargeAndCall).toBeTypeOf("function");
    expect(api.Throttle).toBeTypeOf("function");
    expect(api.todayUtc).toBeTypeOf("function");
  });

  it("re-exports path helpers", () => {
    expect(api.instanceDir).toBeTypeOf("function");
    expect(api.conversationFile).toBeTypeOf("function");
  });

  it("exposes DEFAULT_SETTINGS and DEFAULT_TRIAGE_TEMPLATE", () => {
    expect(api.DEFAULT_SETTINGS).toBeDefined();
    expect(api.DEFAULT_TRIAGE_TEMPLATE).toBeDefined();
  });
});

describe("Plan 2 public API", () => {
  it("re-exports runtime config, conversation store, mind, events, http", () => {
    expect(api.loadConfig).toBeTypeOf("function");
    expect(api.loadConversation).toBeTypeOf("function");
    expect(api.appendConversationEntry).toBeTypeOf("function");
    expect(api.buildSystemPrompt).toBeTypeOf("function");
    expect(api.skillToTool).toBeTypeOf("function");
    expect(api.builtInTools).toBeTypeOf("function");
    expect(api.createAnthropicClient).toBeTypeOf("function");
    expect(api.MockAnthropicClient).toBeTypeOf("function");
    expect(api.runMindTurn).toBeTypeOf("function");
    expect(api.runMindWithTools).toBeTypeOf("function");
    expect(api.runMindStreaming).toBeTypeOf("function");
    expect(api.runBudgetedMind).toBeTypeOf("function");
    expect(api.MindWorker).toBeTypeOf("function");
    expect(api.WorkerPool).toBeTypeOf("function");
    expect(api.Broadcaster).toBeTypeOf("function");
    expect(api.serializeServerEvent).toBeTypeOf("function");
    expect(api.createApp).toBeTypeOf("function");
    expect(api.requireAuth).toBeTypeOf("function");
  });
});
