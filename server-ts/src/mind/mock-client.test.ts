import { describe, expect, it } from "vitest";
import { MockAnthropicClient, type MockMessage } from "./mock-client.js";

const TEXT_MESSAGE: MockMessage = {
  id: "msg_1",
  role: "assistant",
  model: "mock",
  stop_reason: "end_turn",
  content: [{ type: "text", text: "hello" }],
  usage: { input_tokens: 100, output_tokens: 20 },
};

const TOOL_USE_MESSAGE: MockMessage = {
  id: "msg_2",
  role: "assistant",
  model: "mock",
  stop_reason: "tool_use",
  content: [
    {
      type: "tool_use",
      id: "toolu_1",
      name: "send_push",
      input: { title: "hi", body: "there" },
    },
  ],
  usage: { input_tokens: 120, output_tokens: 30 },
};

describe("MockAnthropicClient.messages.create", () => {
  it("returns queued messages in FIFO order", async () => {
    const client = new MockAnthropicClient([TEXT_MESSAGE]);
    const m = await client.messages.create({
      model: "mock",
      max_tokens: 1024,
      messages: [{ role: "user", content: "hi" }],
    });
    expect(m.content[0]).toEqual({ type: "text", text: "hello" });
  });

  it("records each call so tests can assert on the request shape", async () => {
    const client = new MockAnthropicClient([TEXT_MESSAGE]);
    await client.messages.create({
      model: "mock",
      max_tokens: 1024,
      messages: [{ role: "user", content: "hi" }],
    });
    expect(client.calls).toHaveLength(1);
    expect(client.calls[0]?.messages[0]?.content).toBe("hi");
  });

  it("throws when queue is empty", async () => {
    const client = new MockAnthropicClient([]);
    await expect(
      client.messages.create({
        model: "mock",
        max_tokens: 1024,
        messages: [{ role: "user", content: "hi" }],
      }),
    ).rejects.toThrow(/no queued response/i);
  });

  it("supports a tool_use stop reason", async () => {
    const client = new MockAnthropicClient([TOOL_USE_MESSAGE]);
    const m = await client.messages.create({
      model: "mock",
      max_tokens: 1024,
      messages: [{ role: "user", content: "push me" }],
    });
    expect(m.stop_reason).toBe("tool_use");
    expect(m.content[0]).toMatchObject({ type: "tool_use", name: "send_push" });
  });
});
