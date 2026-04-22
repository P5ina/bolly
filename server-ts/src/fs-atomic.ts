import { mkdir, rename, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

/**
 * Write body to path atomically by writing to a .tmp sibling and renaming.
 * Mirrors the Rust backend's write-tmp-then-rename pattern.
 */
export async function atomicWrite(path: string, body: string | Uint8Array): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  const tmp = `${path}.tmp`;
  await writeFile(tmp, body);
  await rename(tmp, path);
}
