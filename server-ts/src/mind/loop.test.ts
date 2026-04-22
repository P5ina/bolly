import { describe, expect, it } from "vitest";
import { runMindTurn } from "./loop.js";
import { MockAnthropicClient } from "./mock-client.js";

describe("runMindTurn — single-turn text", () => {
  it("sends a user message and returns the assistant text", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "claude-sonnet-4-6",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "hi there" }],
        usage: { input_tokens: 100, output_tokens: 20 },
      },
    ]);

    const result = await runMindTurn({
      client,
      model: "claude-sonnet-4-6",
      systemPrompt: "You are Bolly.",
      tools: [],
      conversation: [],
      userMessage: "hello",
    });

    expect(result.stopReason).toBe("end_turn");
    expect(result.assistantContent).toEqual([{ type: "text", text: "hi there" }]);
    expect(result.totalUsage.input_tokens).toBe(100);
    expect(result.totalUsage.output_tokens).toBe(20);
  });

  it("passes the full message array including the new user message", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "claude-sonnet-4-6",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "ok" }],
        usage: { input_tokens: 50, output_tokens: 5 },
      },
    ]);

    await runMindTurn({
      client,
      model: "claude-sonnet-4-6",
      systemPrompt: "You are Bolly.",
      tools: [],
      conversation: [{ id: "p1", role: "user", content: [{ type: "text", text: "prior" }], ts: 1 }],
      userMessage: "hello",
    });

    const call = client.calls[0];
    expect(call?.messages).toHaveLength(2);
    expect(call?.messages[0]?.content).toEqual([{ type: "text", text: "prior" }]);
    expect(call?.messages[1]?.content).toBe("hello");
  });

  it("includes the system prompt on the request", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "." }],
        usage: { input_tokens: 10, output_tokens: 1 },
      },
    ]);

    await runMindTurn({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "hi",
    });

    expect(client.calls[0]?.system).toBeDefined();
  });
});
