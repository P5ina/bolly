import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { OutreachEntry } from "../types.js";
import { appendOutreach, readRecentOutreach } from "./audit.js";

function entry(id: string, ts: number, title = `event-${id}`): OutreachEntry {
  return {
    id,
    ts,
    channel: "push",
    title,
    urgency: "medium",
    delivered: true,
    dedup_suppressed: false,
  };
}

describe("appendOutreach + readRecentOutreach", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-outreach-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("returns an empty list when the file is absent", async () => {
    const entries = await readRecentOutreach(home, "alice", 10);
    expect(entries).toEqual([]);
  });

  it("appends entries as JSON lines", async () => {
    await appendOutreach(home, "alice", entry("a", 100));
    await appendOutreach(home, "alice", entry("b", 200));
    const entries = await readRecentOutreach(home, "alice", 10);
    expect(entries.map((e) => e.id)).toEqual(["a", "b"]);
  });

  it("returns only the last N entries, newest last", async () => {
    for (let i = 0; i < 5; i++) {
      await appendOutreach(home, "alice", entry(`e${i}`, 1000 + i));
    }
    const entries = await readRecentOutreach(home, "alice", 3);
    expect(entries.map((e) => e.id)).toEqual(["e2", "e3", "e4"]);
  });

  it("tolerates and skips malformed lines", async () => {
    // Write a broken line directly, then a good one through the API
    const { appendFile, mkdir } = await import("node:fs/promises");
    const dir = join(home, "instances", "alice");
    await mkdir(dir, { recursive: true });
    await appendFile(join(dir, "outreach.jsonl"), "{ not valid json\n");
    await appendOutreach(home, "alice", entry("good", 500));
    const entries = await readRecentOutreach(home, "alice", 10);
    expect(entries.map((e) => e.id)).toEqual(["good"]);
  });
});
