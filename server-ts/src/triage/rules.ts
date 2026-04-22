import { readFile } from "node:fs/promises";
import { triageFile } from "../paths.js";

export const DEFAULT_TRIAGE_TEMPLATE = `# Triage rules

Default: unless matched below, ignore.

## Always escalate
- User sent a message in the app
- Email or event marked urgent
- A skill I installed explicitly asks for escalation

## Always digest
- Newsletter emails
- Notification-only emails
- Calendar changes in the past

## Quiet hours
- Between 22:00 and 07:00: only escalate if "emergency" in subject
`;

/**
 * Read the user's triage rules file. Returns the default template when
 * the file is missing — the mind will overwrite it once the user expresses
 * preferences.
 */
export async function loadTriageRules(home: string, slug: string): Promise<string> {
  try {
    return await readFile(triageFile(home, slug), "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      return DEFAULT_TRIAGE_TEMPLATE;
    }
    throw err;
  }
}
