import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_TRIAGE_TEMPLATE, loadTriageRules } from "./rules.js";

describe("loadTriageRules", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-triage-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("returns the default template when triage.md is missing", async () => {
    const rules = await loadTriageRules(home, "alice");
    expect(rules).toBe(DEFAULT_TRIAGE_TEMPLATE);
  });

  it("returns the raw file contents when triage.md exists", async () => {
    const dir = join(home, "instances", "alice");
    await mkdir(dir, { recursive: true });
    const body = "# My rules\n\nAlways digest newsletters.";
    await writeFile(join(dir, "triage.md"), body);
    const rules = await loadTriageRules(home, "alice");
    expect(rules).toBe(body);
  });

  it("DEFAULT_TRIAGE_TEMPLATE contains the word Default", () => {
    expect(DEFAULT_TRIAGE_TEMPLATE).toMatch(/default/i);
  });
});
