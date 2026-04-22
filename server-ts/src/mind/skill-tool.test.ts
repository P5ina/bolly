import { describe, expect, it } from "vitest";
import type { Skill } from "../types.js";
import { builtInTools, skillToTool, skillToToolName } from "./skill-tool.js";

const SKILL: Skill = {
  frontmatter: {
    name: "email-morning-check",
    triggers: [],
    tools: [],
    enabled: true,
  },
  body: "Read unread email and summarize urgent items.",
  path: "/skills/x.md",
};

describe("skillToToolName", () => {
  it("prefixes with run_ and replaces dashes with underscores", () => {
    expect(skillToToolName("email-morning-check")).toBe("run_email_morning_check");
  });

  it("preserves underscores", () => {
    expect(skillToToolName("simple_name")).toBe("run_simple_name");
  });
});

describe("skillToTool", () => {
  it("produces a tool definition with the right name and description", () => {
    const tool = skillToTool(SKILL);
    expect(tool.name).toBe("run_email_morning_check");
    expect(tool.description).toContain("Read unread email");
  });

  it("accepts an input schema with a free-form context argument", () => {
    const tool = skillToTool(SKILL);
    expect(tool.input_schema.type).toBe("object");
    expect(tool.input_schema.properties).toHaveProperty("context");
  });
});

describe("builtInTools", () => {
  it("provides send_push, send_email, defer_for_digest", () => {
    const names = builtInTools().map((t) => t.name);
    expect(names).toContain("send_push");
    expect(names).toContain("send_email");
    expect(names).toContain("defer_for_digest");
  });

  it("send_push has a title and body schema", () => {
    const push = builtInTools().find((t) => t.name === "send_push");
    expect(push?.input_schema.required).toContain("title");
    expect(push?.input_schema.required).toContain("body");
  });
});
