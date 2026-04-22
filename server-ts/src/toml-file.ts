import { readFile } from "node:fs/promises";
import { parse as parseToml } from "smol-toml";
import type { ZodSchema } from "zod";

/**
 * Read a TOML file and validate it against a zod schema.
 * Returns null if the file does not exist.
 */
export async function readToml<T>(path: string, schema: ZodSchema<T>): Promise<T | null> {
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw err;
  }

  let parsed: unknown;
  try {
    parsed = parseToml(raw);
  } catch (err) {
    throw new Error(`failed to parse TOML at ${path}: ${(err as Error).message}`);
  }

  return schema.parse(parsed);
}
