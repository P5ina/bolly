import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { Broadcaster } from "../events/broadcaster.js";
import { MockAnthropicClient } from "../mind/mock-client.js";
import { WorkerPool } from "../mind/pool.js";
import { createApp } from "./server.js";

describe("createApp", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-http-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("POST /api/chat returns 202 and triggers the mind worker", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "ack" }],
        usage: { input_tokens: 10, output_tokens: 1 },
      },
    ]);
    const broadcaster = new Broadcaster();
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
      body: JSON.stringify({ instance_slug: "alice", chat_id: "default", content: "hi" }),
    });

    expect(res.status).toBe(202);
  });

  it("POST /api/chat requires auth when configured", async () => {
    const pool = new WorkerPool({
      clientFactory: () => new MockAnthropicClient([]),
      home,
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3,
      pricePerMTokOut: 15,
      broadcaster: new Broadcaster(),
    });
    const { app } = createApp({ authToken: "secret", pool, broadcaster: new Broadcaster() });

    const res = await app.request("/api/chat", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ instance_slug: "x", chat_id: "y", content: "hi" }),
    });
    expect(res.status).toBe(401);
  });

  it("GET /api/health returns 200", async () => {
    const pool = new WorkerPool({
      clientFactory: () => new MockAnthropicClient([]),
      home,
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3,
      pricePerMTokOut: 15,
      broadcaster: new Broadcaster(),
    });
    const { app } = createApp({ authToken: undefined, pool, broadcaster: new Broadcaster() });

    const res = await app.request("/api/health");
    expect(res.status).toBe(200);
  });
});
