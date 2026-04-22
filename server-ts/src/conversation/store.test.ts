import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { appendConversationEntry, loadConversation, saveConversation } from "./store.js";
import type { ConversationEntry } from "./types.js";

function entry(id: string, ts: number): ConversationEntry {
  return {
    id,
    role: "user",
    content: [{ type: "text", text: `hello ${id}` }],
    ts,
  };
}

describe("conversation store", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-conv-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("loadConversation returns empty array when file is missing", async () => {
    const result = await loadConversation(home, "alice", "default");
    expect(result).toEqual([]);
  });

  it("saveConversation then loadConversation round-trips", async () => {
    await saveConversation(home, "alice", "default", [entry("a", 100), entry("b", 200)]);
    const result = await loadConversation(home, "alice", "default");
    expect(result.map((e) => e.id)).toEqual(["a", "b"]);
  });

  it("appendConversationEntry writes a new entry to a fresh file", async () => {
    await appendConversationEntry(home, "alice", "default", entry("a", 100));
    const result = await loadConversation(home, "alice", "default");
    expect(result).toHaveLength(1);
    expect(result[0]?.id).toBe("a");
  });

  it("appendConversationEntry appends to an existing file", async () => {
    await appendConversationEntry(home, "alice", "default", entry("a", 100));
    await appendConversationEntry(home, "alice", "default", entry("b", 200));
    const result = await loadConversation(home, "alice", "default");
    expect(result.map((e) => e.id)).toEqual(["a", "b"]);
  });

  it("saveConversation writes atomically (no .tmp residue)", async () => {
    await saveConversation(home, "alice", "default", [entry("a", 100)]);
    const { readdir } = await import("node:fs/promises");
    const dir = join(home, "instances", "alice", "chats", "default");
    const entries = await readdir(dir);
    expect(entries).toEqual(["conversation.json"]);
  });
});
