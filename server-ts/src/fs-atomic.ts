import { randomUUID } from "node:crypto";
import { mkdir, rename, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

/**
 * Write body to path atomically by writing to a unique .tmp sibling and renaming.
 * Mirrors the Rust backend's write-tmp-then-rename pattern.
 * The temp suffix is randomized so two concurrent writers to the same path do
 * not clobber each other's in-flight temp files.
 */
export async function atomicWrite(path: string, body: string | Uint8Array): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  const tmp = `${path}.${randomUUID()}.tmp`;
  await writeFile(tmp, body);
  await rename(tmp, path);
}
