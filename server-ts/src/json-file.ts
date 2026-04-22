import { readFile } from "node:fs/promises";
import type { ZodSchema } from "zod";
import { atomicWrite } from "./fs-atomic.js";

/**
 * Read a JSON file and validate it against a zod schema.
 * Returns null if the file does not exist.
 * Throws on parse errors or schema failures.
 */
export async function readJson<T>(path: string, schema: ZodSchema<T>): Promise<T | null> {
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw err;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    throw new Error(`failed to parse JSON at ${path}: ${(err as Error).message}`);
  }

  return schema.parse(parsed);
}

/**
 * Write a value as pretty JSON to path, atomically.
 */
export async function writeJson<T>(path: string, value: T): Promise<void> {
  await atomicWrite(path, JSON.stringify(value, null, 2));
}
