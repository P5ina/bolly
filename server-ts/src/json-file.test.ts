import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { z } from "zod";
import { readJson, writeJson } from "./json-file.js";

const PersonSchema = z.object({ name: z.string(), age: z.number() });

describe("readJson", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "bolly-json-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("parses a valid JSON file through the schema", async () => {
    const f = join(dir, "person.json");
    await writeFile(f, JSON.stringify({ name: "alice", age: 30 }));
    const result = await readJson(f, PersonSchema);
    expect(result).toEqual({ name: "alice", age: 30 });
  });

  it("returns null when the file is missing", async () => {
    const f = join(dir, "missing.json");
    const result = await readJson(f, PersonSchema);
    expect(result).toBeNull();
  });

  it("throws when the JSON is malformed", async () => {
    const f = join(dir, "broken.json");
    await writeFile(f, "{ not valid");
    await expect(readJson(f, PersonSchema)).rejects.toThrow(/parse/i);
  });

  it("throws when the shape fails schema validation", async () => {
    const f = join(dir, "wrong.json");
    await writeFile(f, JSON.stringify({ name: "alice", age: "thirty" }));
    await expect(readJson(f, PersonSchema)).rejects.toThrow();
  });
});

describe("writeJson", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "bolly-json-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("writes a pretty-printed JSON file that round-trips", async () => {
    const f = join(dir, "round.json");
    await writeJson(f, { name: "bob", age: 25 });
    const roundTripped = await readJson(f, PersonSchema);
    expect(roundTripped).toEqual({ name: "bob", age: 25 });
  });
});
