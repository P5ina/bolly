import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { recordSpend } from "../budget/ledger.js";
import { runBudgetedMind } from "./budgeted-mind.js";
import { MockAnthropicClient } from "./mock-client.js";

describe("runBudgetedMind", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-bm-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("runs the mind and records spend on the ledger", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "." }],
        usage: { input_tokens: 1000, output_tokens: 200 },
      },
    ]);

    const result = await runBudgetedMind({
      client,
      home,
      slug: "alice",
      day: "2026-04-22",
      capUsd: 2.0,
      model: "mock",
      pricePerMTokIn: 3.0,
      pricePerMTokOut: 15.0,
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "hi",
      executeTool: async () => "unused",
      maxIterations: 5,
    });

    expect(result.downgraded).toBeUndefined();
    const { loadDaily } = await import("../budget/ledger.js");
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    // 1000 * 3/1M + 200 * 15/1M = 0.003 + 0.003 = 0.006
    expect(ledger.dollars_spent).toBeCloseTo(0.006, 4);
    expect(ledger.calls).toBe(1);
  });

  it("downgrades when budget is suppressed without invoking the client", async () => {
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 0,
      tokens_out: 0,
      dollars: 2.5,
    });

    const client = new MockAnthropicClient([
      {
        id: "should-not-run",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "." }],
        usage: { input_tokens: 10, output_tokens: 1 },
      },
    ]);

    const result = await runBudgetedMind({
      client,
      home,
      slug: "alice",
      day: "2026-04-22",
      capUsd: 2.0,
      model: "mock",
      pricePerMTokIn: 3.0,
      pricePerMTokOut: 15.0,
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "hi",
      executeTool: async () => "unused",
      maxIterations: 5,
    });

    expect(result.downgraded).toBe(true);
    expect(client.calls).toHaveLength(0);
  });
});
