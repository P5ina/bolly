import type Anthropic from "@anthropic-ai/sdk";
import type { ContentBlock, ConversationEntry } from "../conversation/types.js";
import type { MindClient } from "./anthropic-client.js";
import type { ToolDefinition } from "./skill-tool.js";

export type MindTurnInputs = {
  client: MindClient;
  model: string;
  systemPrompt: string;
  tools: ToolDefinition[];
  conversation: ConversationEntry[];
  userMessage: string;
};

export type MindTurnResult = {
  stopReason: string;
  assistantContent: ContentBlock[];
  totalUsage: {
    input_tokens: number;
    output_tokens: number;
    cache_read_input_tokens: number;
    cache_creation_input_tokens: number;
  };
};

/**
 * Convert a Conversation into the Anthropic messages array shape.
 * Each entry becomes one role-tagged message; content blocks pass through.
 */
function conversationToMessages(
  conversation: ConversationEntry[],
): Anthropic.Messages.MessageParam[] {
  return conversation.map((e) => ({
    role: e.role,
    content: e.content as unknown as Anthropic.Messages.ContentBlockParam[],
  }));
}

/**
 * Run a single mind turn without tool use or continuation.
 * Higher-level wrappers compose multiple turns for tool use etc.
 */
export async function runMindTurn(inputs: MindTurnInputs): Promise<MindTurnResult> {
  const { client, model, systemPrompt, tools, conversation, userMessage } = inputs;

  const messages: Anthropic.Messages.MessageParam[] = [
    ...conversationToMessages(conversation),
    { role: "user", content: userMessage },
  ];

  const response = await client.messages.create({
    model,
    max_tokens: 4096,
    system: systemPrompt,
    tools: tools as unknown as Anthropic.Messages.ToolUnion[],
    messages,
  });

  return {
    stopReason: response.stop_reason ?? "end_turn",
    assistantContent: response.content as unknown as ContentBlock[],
    totalUsage: {
      input_tokens: response.usage.input_tokens,
      output_tokens: response.usage.output_tokens,
      cache_read_input_tokens: response.usage.cache_read_input_tokens ?? 0,
      cache_creation_input_tokens: response.usage.cache_creation_input_tokens ?? 0,
    },
  };
}
