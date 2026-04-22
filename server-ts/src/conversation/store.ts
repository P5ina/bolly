import { readJson, writeJson } from "../json-file.js";
import { conversationFile } from "../paths.js";
import { type Conversation, type ConversationEntry, ConversationSchema } from "./types.js";

export async function loadConversation(
  home: string,
  slug: string,
  chatId: string,
): Promise<Conversation> {
  const path = conversationFile(home, slug, chatId);
  const existing = await readJson(path, ConversationSchema);
  return (existing ?? []) as Conversation;
}

export async function saveConversation(
  home: string,
  slug: string,
  chatId: string,
  conversation: Conversation,
): Promise<void> {
  await writeJson(conversationFile(home, slug, chatId), conversation);
}

export async function appendConversationEntry(
  home: string,
  slug: string,
  chatId: string,
  entry: ConversationEntry,
): Promise<void> {
  const existing = await loadConversation(home, slug, chatId);
  existing.push(entry);
  await saveConversation(home, slug, chatId, existing);
}
