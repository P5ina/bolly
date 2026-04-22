import { describe, expect, it } from "vitest";
import { parseSkill } from "./parse.js";

const VALID_SKILL = `---
name: email-morning-check
created: 2026-04-22
triggers:
  - scheduled: "every weekday at 8am"
  - event: "email arrives with 'urgent' in subject"
tools:
  - read_email
  - send_push
enabled: true
---

When triggered, read unread emails since last check.
Summarize anything the user would care about.
`;

describe("parseSkill", () => {
  it("parses a well-formed skill into frontmatter + body", () => {
    const skill = parseSkill(VALID_SKILL, "/instances/alice/skills/email.md");
    expect(skill.frontmatter.name).toBe("email-morning-check");
    expect(skill.frontmatter.tools).toEqual(["read_email", "send_push"]);
    expect(skill.frontmatter.enabled).toBe(true);
    expect(skill.frontmatter.triggers).toHaveLength(2);
    expect(skill.body).toContain("When triggered");
    expect(skill.path).toBe("/instances/alice/skills/email.md");
  });

  it("applies defaults for missing optional fields", () => {
    const minimal = `---
name: minimal
---

body here
`;
    const skill = parseSkill(minimal, "/x.md");
    expect(skill.frontmatter.enabled).toBe(true);
    expect(skill.frontmatter.tools).toEqual([]);
    expect(skill.frontmatter.triggers).toEqual([]);
  });

  it("throws when frontmatter is absent", () => {
    expect(() => parseSkill("no frontmatter here", "/x.md")).toThrow(/frontmatter/i);
  });

  it("throws when the required name field is missing", () => {
    const bad = `---
description: anonymous skill
---

body
`;
    expect(() => parseSkill(bad, "/x.md")).toThrow();
  });

  it("respects enabled: false", () => {
    const disabled = `---
name: sleeping
enabled: false
---

not right now
`;
    const skill = parseSkill(disabled, "/x.md");
    expect(skill.frontmatter.enabled).toBe(false);
  });
});
