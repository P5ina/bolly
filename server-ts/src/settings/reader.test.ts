import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, loadSettings } from "./reader.js";

describe("loadSettings", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-settings-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("returns DEFAULT_SETTINGS when settings.toml is missing", async () => {
    const s = await loadSettings(home, "alice");
    expect(s).toEqual(DEFAULT_SETTINGS);
  });

  it("merges user-provided fields over defaults", async () => {
    const dir = join(home, "instances", "alice");
    await mkdir(dir, { recursive: true });
    await writeFile(
      join(dir, "settings.toml"),
      `daily_budget_usd = 5.0

[quiet_hours]
start = "23:00"
end = "08:00"

[push]
daily_max = 10
`,
    );
    const s = await loadSettings(home, "alice");
    expect(s.daily_budget_usd).toBe(5.0);
    expect(s.quiet_hours.start).toBe("23:00");
    expect(s.quiet_hours.end).toBe("08:00");
    expect(s.push.daily_max).toBe(10);
    // Defaults preserved for unspecified fields
    expect(s.push.enabled).toBe(true);
    expect(s.email.enabled).toBe(DEFAULT_SETTINGS.email.enabled);
  });

  it("DEFAULT_SETTINGS has a 2.00 daily budget", () => {
    expect(DEFAULT_SETTINGS.daily_budget_usd).toBe(2.0);
  });

  it("DEFAULT_SETTINGS has 22:00-07:00 quiet hours", () => {
    expect(DEFAULT_SETTINGS.quiet_hours.start).toBe("22:00");
    expect(DEFAULT_SETTINGS.quiet_hours.end).toBe("07:00");
  });
});
