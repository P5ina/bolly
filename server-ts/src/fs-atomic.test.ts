import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { atomicWrite } from "./fs-atomic.js";

describe("atomicWrite", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "bolly-atomic-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("writes content to the target path", async () => {
    const target = join(dir, "hello.txt");
    await atomicWrite(target, "hello world");
    const contents = await readFile(target, "utf8");
    expect(contents).toBe("hello world");
  });

  it("creates parent directories if missing", async () => {
    const target = join(dir, "nested", "deep", "file.txt");
    await atomicWrite(target, "ok");
    const contents = await readFile(target, "utf8");
    expect(contents).toBe("ok");
  });

  it("overwrites an existing file", async () => {
    const target = join(dir, "overwrite.txt");
    await writeFile(target, "old");
    await atomicWrite(target, "new");
    const contents = await readFile(target, "utf8");
    expect(contents).toBe("new");
  });

  it("cleans up its .tmp file on success", async () => {
    const target = join(dir, "cleanup.txt");
    await atomicWrite(target, "body");
    const { readdir } = await import("node:fs/promises");
    const entries = await readdir(dir);
    expect(entries).toEqual(["cleanup.txt"]);
  });
});
