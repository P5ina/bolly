import type { Skill } from "../types.js";
import { skillToToolName } from "./skill-tool.js";

export type SystemPromptInputs = {
  employeeName: string;
  companyName: string;
  soul: string;
  mood: string;
  rhythm: string;
  enabledSkills: Skill[];
  triageRules: string;
};

/**
 * Build the system prompt for the mind. Output is stable for the same inputs,
 * so it caches well via cache_control.
 */
export function buildSystemPrompt(inputs: SystemPromptInputs): string {
  const { employeeName, companyName, soul, mood, rhythm, enabledSkills, triageRules } = inputs;

  const skillsBlock =
    enabledSkills.length === 0
      ? ""
      : `

<skills>
${enabledSkills
  .map((s) => {
    const tool = skillToToolName(s.frontmatter.name);
    return `- ${s.frontmatter.name} (tool: ${tool})\n  ${s.body.trim()}`;
  })
  .join("\n\n")}
</skills>`;

  const triageBlock = triageRules.trim()
    ? `

<triage_rules>
${triageRules.trim()}
</triage_rules>`
    : "";

  return `You are Bolly, the AI coworker for ${employeeName} at ${companyName}.

<persona>${soul}</persona>
<mood>${mood}</mood>
<rhythm>${rhythm}</rhythm>${skillsBlock}${triageBlock}

Use the custom tools available to you when appropriate. You may reach out
to ${employeeName} via send_push, send_email, or defer_for_digest.`;
}
