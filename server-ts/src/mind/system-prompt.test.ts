import { describe, expect, it } from "vitest";
import type { Skill } from "../types.js";
import { buildSystemPrompt } from "./system-prompt.js";

const SKILL: Skill = {
  frontmatter: {
    name: "email-check",
    triggers: [{ scheduled: "every weekday at 8am" }],
    tools: [],
    enabled: true,
  },
  body: "When triggered, read unread emails and surface urgent items.",
  path: "/skills/email-check.md",
};

describe("buildSystemPrompt", () => {
  it("includes employee name and company name", () => {
    const prompt = buildSystemPrompt({
      employeeName: "alice",
      companyName: "Acme",
      soul: "I am calm and attentive.",
      mood: "focused",
      rhythm: "morning person",
      enabledSkills: [],
      triageRules: "",
    });
    expect(prompt).toContain("alice");
    expect(prompt).toContain("Acme");
  });

  it("embeds soul / mood / rhythm", () => {
    const prompt = buildSystemPrompt({
      employeeName: "alice",
      companyName: "Acme",
      soul: "MY_SOUL",
      mood: "MY_MOOD",
      rhythm: "MY_RHYTHM",
      enabledSkills: [],
      triageRules: "",
    });
    expect(prompt).toContain("MY_SOUL");
    expect(prompt).toContain("MY_MOOD");
    expect(prompt).toContain("MY_RHYTHM");
  });

  it("lists each enabled skill by name and tool", () => {
    const prompt = buildSystemPrompt({
      employeeName: "alice",
      companyName: "Acme",
      soul: "",
      mood: "",
      rhythm: "",
      enabledSkills: [SKILL],
      triageRules: "",
    });
    expect(prompt).toContain("email-check");
    expect(prompt).toContain("run_email_check");
  });

  it("skips a skills block entirely when no skills are enabled", () => {
    const prompt = buildSystemPrompt({
      employeeName: "alice",
      companyName: "Acme",
      soul: "",
      mood: "",
      rhythm: "",
      enabledSkills: [],
      triageRules: "",
    });
    expect(prompt).not.toContain("<skills>");
  });

  it("includes triage rules when present", () => {
    const prompt = buildSystemPrompt({
      employeeName: "alice",
      companyName: "Acme",
      soul: "",
      mood: "",
      rhythm: "",
      enabledSkills: [],
      triageRules: "ALWAYS escalate urgent email",
    });
    expect(prompt).toContain("ALWAYS escalate urgent email");
  });
});
