import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadSkills } from "./loader.js";

async function seed(home: string, slug: string, name: string, body: string): Promise<void> {
  const dir = join(home, "instances", slug, "skills");
  await mkdir(dir, { recursive: true });
  await writeFile(join(dir, `${name}.md`), body);
}

describe("loadSkills", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-skills-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("returns an empty array when no skills directory exists", async () => {
    const skills = await loadSkills(home, "alice");
    expect(skills).toEqual([]);
  });

  it("loads all .md files from the skills directory", async () => {
    await seed(home, "alice", "one", "---\nname: one\n---\n\nbody one");
    await seed(home, "alice", "two", "---\nname: two\n---\n\nbody two");
    const skills = await loadSkills(home, "alice");
    const names = skills.map((s) => s.frontmatter.name).sort();
    expect(names).toEqual(["one", "two"]);
  });

  it("skips non-.md files", async () => {
    await seed(home, "alice", "real", "---\nname: real\n---\n\nbody");
    await writeFile(join(home, "instances", "alice", "skills", "readme.txt"), "not a skill");
    const skills = await loadSkills(home, "alice");
    expect(skills).toHaveLength(1);
    expect(skills[0]?.frontmatter.name).toBe("real");
  });

  it("filters out disabled skills when enabledOnly=true", async () => {
    await seed(home, "alice", "on", "---\nname: on\nenabled: true\n---\n\nbody");
    await seed(home, "alice", "off", "---\nname: off\nenabled: false\n---\n\nbody");
    const enabled = await loadSkills(home, "alice", { enabledOnly: true });
    expect(enabled.map((s) => s.frontmatter.name)).toEqual(["on"]);
  });

  it("returns all skills when enabledOnly omitted", async () => {
    await seed(home, "alice", "on", "---\nname: on\nenabled: true\n---\n\nbody");
    await seed(home, "alice", "off", "---\nname: off\nenabled: false\n---\n\nbody");
    const all = await loadSkills(home, "alice");
    expect(all).toHaveLength(2);
  });
});
