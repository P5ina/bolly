import matter from "gray-matter";
import { type Skill, SkillFrontmatterSchema } from "../types.js";

/**
 * Parse a skill file (YAML frontmatter + markdown body) into a Skill.
 * Throws if frontmatter is missing or does not match the schema.
 */
export function parseSkill(contents: string, path: string): Skill {
  if (!contents.trimStart().startsWith("---")) {
    throw new Error(`skill at ${path} is missing YAML frontmatter`);
  }

  const { data, content } = matter(contents);

  // gray-matter parses YAML dates as JS Date objects; coerce to ISO string so
  // the schema's z.string().optional() validation succeeds.
  const normalized =
    data.created instanceof Date
      ? { ...data, created: data.created.toISOString().slice(0, 10) }
      : data;

  const frontmatter = SkillFrontmatterSchema.parse(normalized);

  return {
    frontmatter,
    body: content.trimStart(),
    path,
  };
}
