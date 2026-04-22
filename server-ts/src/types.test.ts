import { describe, expect, it } from "vitest";
import { BudgetStateSchema, EventSourceSchema, TriageDecisionSchema } from "./types.js";

describe("EventSourceSchema", () => {
  it("accepts the eight known sources", () => {
    const sources = [
      "user_msg",
      "user_activity",
      "email",
      "calendar",
      "scheduled",
      "idle",
      "skill_emit",
      "instance_emit",
    ];
    for (const s of sources) {
      expect(EventSourceSchema.parse(s)).toBe(s);
    }
  });

  it("rejects an unknown source", () => {
    expect(() => EventSourceSchema.parse("bogus")).toThrow();
  });
});

describe("TriageDecisionSchema", () => {
  it("accepts ignore, digest, escalate", () => {
    for (const d of ["ignore", "digest", "escalate"]) {
      expect(TriageDecisionSchema.parse(d)).toBe(d);
    }
  });

  it("rejects any other value", () => {
    expect(() => TriageDecisionSchema.parse("skip")).toThrow();
  });
});

describe("BudgetStateSchema", () => {
  it("accepts ok, tight, suppressed", () => {
    for (const s of ["ok", "tight", "suppressed"]) {
      expect(BudgetStateSchema.parse(s)).toBe(s);
    }
  });

  it("rejects any other value", () => {
    expect(() => BudgetStateSchema.parse("paused")).toThrow();
  });
});
