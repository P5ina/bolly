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
