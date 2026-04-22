import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { Broadcaster } from "../events/broadcaster.js";
import { MockAnthropicClient } from "./mock-client.js";
import { WorkerPool } from "./pool.js";

describe("WorkerPool", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-pool-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("creates one worker per (slug, chatId) on demand", () => {
    const pool = new WorkerPool({
      clientFactory: () => new MockAnthropicClient([]),
      home,
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3,
      pricePerMTokOut: 15,
      broadcaster: new Broadcaster(),
    });

    const a = pool.get("alice", "default");
    const a2 = pool.get("alice", "default");
    const b = pool.get("bob", "default");

    expect(a).toBe(a2);
    expect(a).not.toBe(b);
  });

  it("sweepStale removes workers past their TTL", () => {
    const pool = new WorkerPool({
      clientFactory: () => new MockAnthropicClient([]),
      home,
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3,
      pricePerMTokOut: 15,
      broadcaster: new Broadcaster(),
      warmTtlMs: 1000,
    });

    const w = pool.get("alice", "default");
    w.touch(1_000_000);
    pool.sweepStale(1_000_500);
    expect(pool.get("alice", "default")).toBe(w);

    pool.sweepStale(1_100_000);
    expect(pool.get("alice", "default")).not.toBe(w);
  });
});
