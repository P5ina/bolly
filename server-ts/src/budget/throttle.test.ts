import { describe, expect, it } from "vitest";
import { Throttle } from "./throttle.js";

describe("Throttle", () => {
  it("allows the first N calls within a window", () => {
    const t = new Throttle({ maxCalls: 5, windowMs: 60_000 });
    const start = 1_000_000;
    for (let i = 0; i < 5; i++) {
      expect(t.check("alice", start + i * 100)).toBe(true);
    }
  });

  it("rejects the N+1st call within the window", () => {
    const t = new Throttle({ maxCalls: 3, windowMs: 60_000 });
    const start = 1_000_000;
    t.check("alice", start);
    t.check("alice", start + 1);
    t.check("alice", start + 2);
    expect(t.check("alice", start + 3)).toBe(false);
  });

  it("tracks quota independently per user", () => {
    const t = new Throttle({ maxCalls: 2, windowMs: 60_000 });
    const now = 1_000_000;
    t.check("alice", now);
    t.check("alice", now + 1);
    expect(t.check("alice", now + 2)).toBe(false);
    expect(t.check("bob", now + 2)).toBe(true);
  });

  it("decays entries outside the window", () => {
    const t = new Throttle({ maxCalls: 2, windowMs: 60_000 });
    const now = 1_000_000;
    t.check("alice", now);
    t.check("alice", now + 30_000);
    // Old entry still counts — third call blocked
    expect(t.check("alice", now + 40_000)).toBe(false);
    // After window passes, first entry drops out
    expect(t.check("alice", now + 65_000)).toBe(true);
  });
});
