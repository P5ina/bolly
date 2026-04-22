import { describe, expect, it } from "vitest";
import { runMindTurn, runMindWithTools } from "./loop.js";
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

describe("runMindWithTools — tool-use loop", () => {
  it("executes a tool call and continues to end_turn", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "tool_use",
        content: [
          {
            type: "tool_use",
            id: "toolu_a",
            name: "send_push",
            input: { title: "Q2 review", body: "urgent" },
          },
        ],
        usage: { input_tokens: 100, output_tokens: 10 },
      },
      {
        id: "msg_2",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "sent" }],
        usage: { input_tokens: 120, output_tokens: 5 },
      },
    ]);

    const toolCalls: Array<{ name: string; input: unknown }> = [];
    const result = await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "push me",
      executeTool: async (name, input) => {
        toolCalls.push({ name, input });
        return "ok";
      },
      maxIterations: 5,
    });

    expect(toolCalls).toEqual([
      { name: "send_push", input: { title: "Q2 review", body: "urgent" } },
    ]);
    expect(result.finalText).toBe("sent");
    expect(result.turns).toBe(2);
    expect(result.totalUsage.input_tokens).toBe(220);
    expect(result.totalUsage.output_tokens).toBe(15);
  });

  it("stops with an error if an unknown tool is called", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "tool_use",
        content: [
          {
            type: "tool_use",
            id: "toolu_b",
            name: "does_not_exist",
            input: {},
          },
        ],
        usage: { input_tokens: 100, output_tokens: 10 },
      },
      {
        id: "msg_2",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "caught error" }],
        usage: { input_tokens: 120, output_tokens: 5 },
      },
    ]);

    const result = await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "do it",
      executeTool: async (name) => {
        throw new Error(`unknown tool: ${name}`);
      },
      maxIterations: 5,
    });

    expect(result.finalText).toBe("caught error");
    // Second call should have the is_error tool_result
    const secondCall = client.calls[1];
    const toolResult = secondCall?.messages[secondCall.messages.length - 1];
    expect(toolResult?.role).toBe("user");
    const firstBlock = (toolResult?.content as Array<{ type: string; is_error?: boolean }>)[0];
    expect(firstBlock?.type).toBe("tool_result");
    expect(firstBlock?.is_error).toBe(true);
  });

  it("stops at maxIterations", async () => {
    const toolUseMsg = {
      id: "msg_loop",
      role: "assistant" as const,
      model: "mock",
      stop_reason: "tool_use" as const,
      content: [
        {
          type: "tool_use" as const,
          id: "toolu_x",
          name: "send_push",
          input: { title: "x", body: "y" },
        },
      ],
      usage: { input_tokens: 10, output_tokens: 2 },
    };
    const client = new MockAnthropicClient([toolUseMsg, toolUseMsg, toolUseMsg]);

    const result = await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "push me",
      executeTool: async () => "ok",
      maxIterations: 2,
    });

    expect(result.turns).toBe(2);
    expect(result.hitMaxIterations).toBe(true);
  });
});

describe("runMindWithTools — max_tokens continuation", () => {
  it("re-prompts on stop_reason: max_tokens and merges output", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "max_tokens",
        content: [{ type: "text", text: "part one" }],
        usage: { input_tokens: 100, output_tokens: 4096 },
      },
      {
        id: "msg_2",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "part two" }],
        usage: { input_tokens: 200, output_tokens: 100 },
      },
    ]);

    const result = await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "write a long thing",
      executeTool: async () => "unused",
      maxIterations: 5,
    });

    expect(result.finalText).toBe("part two");
    expect(result.turns).toBe(2);
    // Second call should include a continuation nudge as the last user message
    const secondCall = client.calls[1];
    const lastMsg = secondCall?.messages[secondCall.messages.length - 1];
    expect(lastMsg?.role).toBe("user");
    expect(JSON.stringify(lastMsg?.content)).toMatch(/continue/i);
  });
});

describe("runMindWithTools — prompt caching", () => {
  it("passes top-level cache_control: ephemeral by default", async () => {
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

    await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "hi",
      executeTool: async () => "unused",
      maxIterations: 5,
    });

    const call = client.calls[0] as unknown as { cache_control?: { type: string } };
    expect(call.cache_control).toEqual({ type: "ephemeral" });
  });
});
