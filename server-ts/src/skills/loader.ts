import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { skillsDir } from "../paths.js";
import type { Skill } from "../types.js";
import { parseSkill } from "./parse.js";

export type LoadSkillsOptions = {
  enabledOnly?: boolean;
};

/**
 * Load all skill .md files from an instance's skills directory.
 * Returns [] if the directory does not exist.
 */
export async function loadSkills(
  home: string,
  slug: string,
  opts: LoadSkillsOptions = {},
): Promise<Skill[]> {
  const dir = skillsDir(home, slug);
  let entries: string[];
  try {
    entries = await readdir(dir);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw err;
  }

  const results: Skill[] = [];
  for (const name of entries) {
    if (!name.endsWith(".md")) continue;
    const full = join(dir, name);
    const raw = await readFile(full, "utf8");
    const skill = parseSkill(raw, full);
    if (opts.enabledOnly && !skill.frontmatter.enabled) continue;
    results.push(skill);
  }
  return results;
}
