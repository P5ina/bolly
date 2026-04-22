import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadDaily, recordSpend, todayUtc } from "./ledger.js";

describe("todayUtc", () => {
  it("formats a Date as YYYY-MM-DD in UTC", () => {
    const d = new Date(Date.UTC(2026, 3, 22, 5, 30)); // month is 0-indexed
    expect(todayUtc(d)).toBe("2026-04-22");
  });

  it("rolls over at UTC midnight, not local", () => {
    const d = new Date(Date.UTC(2026, 3, 22, 23, 59));
    expect(todayUtc(d)).toBe("2026-04-22");
    const after = new Date(Date.UTC(2026, 3, 23, 0, 1));
    expect(todayUtc(after)).toBe("2026-04-23");
  });
});

describe("loadDaily", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-ledger-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("returns a fresh ledger with defaults when file missing", async () => {
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger).toEqual({
      day: "2026-04-22",
      calls: 0,
      tokens_in: 0,
      tokens_out: 0,
      dollars_spent: 0,
      cap_usd: 2.0,
      state: "ok",
    });
  });

  it("returns the persisted ledger when file exists", async () => {
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 100,
      tokens_out: 20,
      dollars: 0.5,
    });
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger.calls).toBe(1);
    expect(ledger.tokens_in).toBe(100);
    expect(ledger.tokens_out).toBe(20);
    expect(ledger.dollars_spent).toBeCloseTo(0.5);
  });
});

describe("recordSpend", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-ledger-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("accumulates spend across calls", async () => {
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 100,
      tokens_out: 10,
      dollars: 0.3,
    });
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 200,
      tokens_out: 30,
      dollars: 0.5,
    });
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger.calls).toBe(2);
    expect(ledger.tokens_in).toBe(300);
    expect(ledger.tokens_out).toBe(40);
    expect(ledger.dollars_spent).toBeCloseTo(0.8);
  });

  it("updates state when spend crosses tight threshold", async () => {
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 0,
      tokens_out: 0,
      dollars: 1.5,
    });
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger.state).toBe("tight");
  });

  it("updates state to suppressed when spend reaches cap", async () => {
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 0,
      tokens_out: 0,
      dollars: 2.0,
    });
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger.state).toBe("suppressed");
  });
});
