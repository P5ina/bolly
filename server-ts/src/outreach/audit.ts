import { appendFile, mkdir, readFile } from "node:fs/promises";
import { dirname } from "node:path";
import { outreachFile } from "../paths.js";
import { type OutreachEntry, OutreachEntrySchema } from "../types.js";

export async function appendOutreach(
  home: string,
  slug: string,
  entry: OutreachEntry,
): Promise<void> {
  const path = outreachFile(home, slug);
  await mkdir(dirname(path), { recursive: true });
  await appendFile(path, `${JSON.stringify(entry)}\n`);
}

/**
 * Read the last N outreach entries, oldest-first within the returned slice.
 * Returns [] if the file does not exist. Malformed lines are skipped silently.
 */
export async function readRecentOutreach(
  home: string,
  slug: string,
  limit: number,
): Promise<OutreachEntry[]> {
  const path = outreachFile(home, slug);
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw err;
  }

  const lines = raw.split("\n").filter((l) => l.length > 0);
  const start = Math.max(0, lines.length - limit);
  const slice = lines.slice(start);

  const result: OutreachEntry[] = [];
  for (const line of slice) {
    try {
      const parsed = JSON.parse(line);
      result.push(OutreachEntrySchema.parse(parsed));
    } catch {
      // malformed line — skip
    }
  }
  return result;
}
