import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { Broadcaster } from "../events/broadcaster.js";
import type { ServerEvent } from "../events/server-event.js";
import { MockAnthropicClient } from "./mock-client.js";
import { MindWorker } from "./worker.js";

describe("MindWorker", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-worker-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("handles a chat message end-to-end and persists both turns", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "hi back" }],
        usage: { input_tokens: 100, output_tokens: 10 },
      },
    ]);

    const broadcaster = new Broadcaster();
    const received: ServerEvent[] = [];
    broadcaster.subscribe((e) => received.push(e));

    const worker = new MindWorker({
      client,
      home,
      slug: "alice",
      chatId: "default",
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3.0,
      pricePerMTokOut: 15.0,
      broadcaster,
    });

    await worker.handleUserMessage("hello");

    // Two broadcasts: AgentRunning (on start) and AgentStopped (on end),
    // plus at least one ChatMessageCreated for the assistant reply.
    const types = received.map((e) => e.type);
    expect(types).toContain("agent_running");
    expect(types).toContain("chat_message_created");
    expect(types).toContain("agent_stopped");

    // Conversation persisted: user turn + assistant turn.
    const { loadConversation } = await import("../conversation/store.js");
    const conv = await loadConversation(home, "alice", "default");
    expect(conv.map((e) => e.role)).toEqual(["user", "assistant"]);
  });

  it("tracks warm state: active after use, teardown-eligible after TTL", async () => {
    const client = new MockAnthropicClient([]);
    const worker = new MindWorker({
      client,
      home,
      slug: "alice",
      chatId: "default",
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3.0,
      pricePerMTokOut: 15.0,
      broadcaster: new Broadcaster(),
      warmTtlMs: 1000,
    });

    worker.touch(1_000_000);
    expect(worker.isStaleAt(1_000_500)).toBe(false);
    expect(worker.isStaleAt(1_002_001)).toBe(true);
  });
});
