import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { Broadcaster } from "./events/broadcaster.js";
import type { ServerEvent } from "./events/server-event.js";
import { createApp } from "./http/server.js";
import { MockAnthropicClient } from "./mind/mock-client.js";
import { WorkerPool } from "./mind/pool.js";

describe("E2E: HTTP chat → mind worker → WebSocket-style broadcast", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-e2e-"));
    // Seed a minimal instance with soul.md
    const instDir = join(home, "instances", "alice");
    await mkdir(instDir, { recursive: true });
    await writeFile(join(instDir, "soul.md"), "Alice's Bolly is helpful.");
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("POST /api/chat produces a full event sequence matching the Rust wire format", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_assist",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "Hi Alice, what's up?" }],
        usage: { input_tokens: 200, output_tokens: 50 },
      },
    ]);

    const broadcaster = new Broadcaster();
    const received: ServerEvent[] = [];
    broadcaster.subscribe((e) => received.push(e));

    const pool = new WorkerPool({
      clientFactory: () => client,
      home,
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3,
      pricePerMTokOut: 15,
      broadcaster,
    });

    const { app } = createApp({ authToken: undefined, pool, broadcaster });

    const res = await app.request("/api/chat", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        instance_slug: "alice",
        chat_id: "default",
        content: "hey",
      }),
    });
    expect(res.status).toBe(202);

    // handleUserMessage is fire-and-forget; wait for the worker to finish
    // by polling for the AgentStopped event.
    for (let i = 0; i < 100 && !received.some((e) => e.type === "agent_stopped"); i++) {
      await new Promise((r) => setTimeout(r, 10));
    }

    const types = received.map((e) => e.type);
    expect(types).toContain("agent_running");
    expect(types.filter((t) => t === "chat_message_created")).toHaveLength(2); // user + assistant
    expect(types).toContain("agent_stopped");

    // Verify the wire format is valid JSON the client expects
    const assistantMsg = received.find(
      (e): e is Extract<ServerEvent, { type: "chat_message_created" }> =>
        e.type === "chat_message_created" && e.message.role === "Assistant",
    );
    expect(assistantMsg?.message.content).toBe("Hi Alice, what's up?");
    expect(assistantMsg?.message.kind).toBe("Message");
    expect(assistantMsg?.message.model).toBe("mock");
  });
});
