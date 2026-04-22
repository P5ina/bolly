import { join } from "node:path";

export function bollyHome(home: string): string {
  return home;
}

export function instanceDir(home: string, slug: string): string {
  return join(home, "instances", slug);
}

export function chatDir(home: string, slug: string, chatId: string): string {
  return join(instanceDir(home, slug), "chats", chatId);
}

export function conversationFile(home: string, slug: string, chatId: string): string {
  return join(chatDir(home, slug, chatId), "conversation.json");
}

export function skillsDir(home: string, slug: string): string {
  return join(instanceDir(home, slug), "skills");
}

export function skillFile(home: string, slug: string, name: string): string {
  return join(skillsDir(home, slug), `${name}.md`);
}

export function triageFile(home: string, slug: string): string {
  return join(instanceDir(home, slug), "triage.md");
}

export function budgetDir(home: string, slug: string): string {
  return join(instanceDir(home, slug), "budget");
}

export function budgetDailyFile(home: string, slug: string, day: string): string {
  return join(budgetDir(home, slug), `${day}.json`);
}

export function outreachFile(home: string, slug: string): string {
  return join(instanceDir(home, slug), "outreach.jsonl");
}

export function settingsFile(home: string, slug: string): string {
  return join(instanceDir(home, slug), "settings.toml");
}

export function sharedDir(home: string): string {
  return join(home, "shared");
}

export function sharedChannelDir(home: string): string {
  return join(sharedDir(home), "channel");
}

export function sharedInstancesFile(home: string): string {
  return join(sharedDir(home), "instances.json");
}
