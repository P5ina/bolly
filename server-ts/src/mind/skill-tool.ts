import type { Skill } from "../types.js";

/**
 * A JSON-schema object suitable for the Anthropic Messages API `tools` array.
 */
export type ToolDefinition = {
  name: string;
  description: string;
  input_schema: {
    type: "object";
    properties: Record<string, unknown>;
    required?: string[];
  };
};

export function skillToToolName(skillName: string): string {
  return `run_${skillName.replace(/-/g, "_")}`;
}

export function skillToTool(skill: Skill): ToolDefinition {
  return {
    name: skillToToolName(skill.frontmatter.name),
    description: skill.body.trim(),
    input_schema: {
      type: "object",
      properties: {
        context: {
          type: "string",
          description: "Free-form context describing why this skill should run",
        },
      },
      required: [],
    },
  };
}

export function builtInTools(): ToolDefinition[] {
  return [
    {
      name: "send_push",
      description:
        "Send a push notification to the employee. Use for time-sensitive, short messages worth interrupting for.",
      input_schema: {
        type: "object",
        properties: {
          title: { type: "string", description: "Short title, <100 chars" },
          body: { type: "string", description: "Body text" },
          urgency: {
            type: "string",
            enum: ["low", "medium", "high"],
            description: "Interrupt priority",
          },
        },
        required: ["title", "body"],
      },
    },
    {
      name: "send_email",
      description:
        "Send an email to the employee. Use for longer, async messages such as morning briefs or summaries.",
      input_schema: {
        type: "object",
        properties: {
          subject: { type: "string" },
          body_markdown: { type: "string" },
        },
        required: ["subject", "body_markdown"],
      },
    },
    {
      name: "defer_for_digest",
      description:
        "File a note for the employee's next return to the app. Use for anything not pressing.",
      input_schema: {
        type: "object",
        properties: {
          summary: { type: "string" },
        },
        required: ["summary"],
      },
    },
  ];
}
