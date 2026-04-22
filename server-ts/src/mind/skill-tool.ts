export function skillToToolName(skillName: string): string {
  return `run_${skillName.replace(/-/g, "_")}`;
}
