import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { type CallOutcome, chargeAndCall } from "./charge-and-call.js";

describe("chargeAndCall", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-cac-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("invokes fn with state ok when ledger is empty", async () => {
    let seenState: string | null = null;
    const result = await chargeAndCall(
      { home, slug: "alice", day: "2026-04-22", capUsd: 2.0, tier: 3 },
      async (state) => {
        seenState = state;
        return { ok: true, usage: { tokens_in: 10, tokens_out: 5, dollars: 0.01 } };
      },
    );
    expect(seenState).toBe("ok");
    expect((result as CallOutcome<unknown>).downgraded).toBeUndefined();
  });

  it("records spend after the call", async () => {
    await chargeAndCall(
      { home, slug: "alice", day: "2026-04-22", capUsd: 2.0, tier: 2 },
      async () => ({ ok: true, usage: { tokens_in: 100, tokens_out: 20, dollars: 0.05 } }),
    );
    const { loadDaily } = await import("./ledger.js");
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger.calls).toBe(1);
    expect(ledger.dollars_spent).toBeCloseTo(0.05);
  });

  it("downgrades tier 3 when state is suppressed without calling fn", async () => {
    // Seed ledger to suppressed
    const { recordSpend } = await import("./ledger.js");
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 0,
      tokens_out: 0,
      dollars: 2.5,
    });

    let fnCalled = false;
    const result = await chargeAndCall(
      { home, slug: "alice", day: "2026-04-22", capUsd: 2.0, tier: 3 },
      async () => {
        fnCalled = true;
        return { ok: true, usage: { tokens_in: 0, tokens_out: 0, dollars: 0 } };
      },
    );
    expect(fnCalled).toBe(false);
    expect(result).toEqual({ downgraded: true, reason: "budget_cap" });
  });

  it("allows tier 2 calls even when suppressed (triage is cheap)", async () => {
    const { recordSpend } = await import("./ledger.js");
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 0,
      tokens_out: 0,
      dollars: 2.5,
    });

    let fnCalled = false;
    await chargeAndCall(
      { home, slug: "alice", day: "2026-04-22", capUsd: 2.0, tier: 2 },
      async () => {
        fnCalled = true;
        return { ok: true, usage: { tokens_in: 10, tokens_out: 5, dollars: 0.001 } };
      },
    );
    expect(fnCalled).toBe(true);
  });
});
