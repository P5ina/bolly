import { describe, expect, it } from "vitest";
import { TIGHT_THRESHOLD, computeState } from "./state.js";

describe("computeState", () => {
  it("returns ok when spend is below 70% of cap", () => {
    expect(computeState(0.5, 2.0)).toBe("ok");
    expect(computeState(1.39, 2.0)).toBe("ok");
    expect(computeState(0, 2.0)).toBe("ok");
  });

  it("returns tight at exactly 70% of cap", () => {
    expect(computeState(1.4, 2.0)).toBe("tight");
  });

  it("returns tight when spend is in [70%, 100%)", () => {
    expect(computeState(1.5, 2.0)).toBe("tight");
    expect(computeState(1.999, 2.0)).toBe("tight");
  });

  it("returns suppressed at exactly the cap", () => {
    expect(computeState(2.0, 2.0)).toBe("suppressed");
  });

  it("returns suppressed above the cap", () => {
    expect(computeState(3.0, 2.0)).toBe("suppressed");
  });

  it("exposes TIGHT_THRESHOLD as 0.7", () => {
    expect(TIGHT_THRESHOLD).toBe(0.7);
  });
});
