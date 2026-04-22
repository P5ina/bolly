import { z } from "zod";

export const TextBlockSchema = z.object({
  type: z.literal("text"),
  text: z.string(),
});

export const ToolUseBlockSchema = z.object({
  type: z.literal("tool_use"),
  id: z.string().min(1),
  name: z.string().min(1),
  input: z.record(z.unknown()),
});

export const ToolResultBlockSchema = z.object({
  type: z.literal("tool_result"),
  tool_use_id: z.string().min(1),
  content: z.string(),
  is_error: z.boolean().optional(),
});

export const CompactionBlockSchema = z.object({
  type: z.literal("compaction"),
  content: z.string(),
});

export const ContentBlockSchema = z.discriminatedUnion("type", [
  TextBlockSchema,
  ToolUseBlockSchema,
  ToolResultBlockSchema,
  CompactionBlockSchema,
]);
export type ContentBlock = z.infer<typeof ContentBlockSchema>;

export const ConversationRoleSchema = z.enum(["user", "assistant"]);
export type ConversationRole = z.infer<typeof ConversationRoleSchema>;

export const ConversationEntrySchema = z.object({
  id: z.string().min(1),
  role: ConversationRoleSchema,
  content: z.array(ContentBlockSchema),
  ts: z.number().int().nonnegative(),
  model: z.string().optional(),
});
export type ConversationEntry = z.infer<typeof ConversationEntrySchema>;

export const ConversationSchema = z.array(ConversationEntrySchema);
export type Conversation = z.infer<typeof ConversationSchema>;
