import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { z } from "zod";
import { readToml } from "./toml-file.js";

const ConfigSchema = z.object({
  name: z.string(),
  nested: z.object({ value: z.number() }),
});

describe("readToml", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "bolly-toml-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("parses a valid TOML file through the schema", async () => {
    const f = join(dir, "config.toml");
    await writeFile(f, 'name = "alice"\n[nested]\nvalue = 42\n');
    const result = await readToml(f, ConfigSchema);
    expect(result).toEqual({ name: "alice", nested: { value: 42 } });
  });

  it("returns null when the file is missing", async () => {
    const f = join(dir, "missing.toml");
    const result = await readToml(f, ConfigSchema);
    expect(result).toBeNull();
  });

  it("throws when the TOML is malformed", async () => {
    const f = join(dir, "broken.toml");
    await writeFile(f, "not = [ valid");
    await expect(readToml(f, ConfigSchema)).rejects.toThrow(/parse/i);
  });
});
