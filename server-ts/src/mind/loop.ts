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

export type MindFullTurnInputs = MindTurnInputs & {
  executeTool: (name: string, input: Record<string, unknown>) => Promise<string>;
  maxIterations: number;
};

export type MindFullTurnResult = {
  finalText: string;
  turns: number;
  hitMaxIterations: boolean;
  assistantContent: ContentBlock[];
  totalUsage: MindTurnResult["totalUsage"];
};

/**
 * Run the mind in a loop until Claude stops calling tools or maxIterations
 * is reached. Each tool_use block is executed via the supplied executeTool
 * callback; errors become is_error: true tool_result blocks rather than
 * throwing, so Claude can observe and recover.
 */
export async function runMindWithTools(inputs: MindFullTurnInputs): Promise<MindFullTurnResult> {
  const { client, model, systemPrompt, tools, executeTool, maxIterations } = inputs;

  let messages: Anthropic.Messages.MessageParam[] = [
    ...conversationToMessages(inputs.conversation),
    { role: "user", content: inputs.userMessage },
  ];

  let totalInput = 0;
  let totalOutput = 0;
  let totalCacheRead = 0;
  let totalCacheCreation = 0;

  let lastAssistantContent: ContentBlock[] = [];
  let turns = 0;

  for (let i = 0; i < maxIterations; i++) {
    turns++;
    const response = await client.messages.create({
      model,
      max_tokens: 4096,
      system: systemPrompt,
      tools: tools as unknown as Anthropic.Messages.ToolUnion[],
      messages,
    });

    totalInput += response.usage.input_tokens;
    totalOutput += response.usage.output_tokens;
    totalCacheRead += response.usage.cache_read_input_tokens ?? 0;
    totalCacheCreation += response.usage.cache_creation_input_tokens ?? 0;

    lastAssistantContent = response.content as unknown as ContentBlock[];

    // Append assistant turn to the running history
    messages = [
      ...messages,
      {
        role: "assistant",
        content: response.content as unknown as Anthropic.Messages.ContentBlockParam[],
      },
    ];

    if (response.stop_reason !== "tool_use") break;

    // Execute each tool_use block and produce tool_result blocks
    const toolResultBlocks: Anthropic.Messages.ContentBlockParam[] = [];
    for (const block of response.content) {
      if (block.type !== "tool_use") continue;
      let content: string;
      let isError = false;
      try {
        content = await executeTool(block.name, block.input as Record<string, unknown>);
      } catch (err) {
        content = `error: ${(err as Error).message}`;
        isError = true;
      }
      toolResultBlocks.push({
        type: "tool_result",
        tool_use_id: block.id,
        content,
        is_error: isError,
      });
    }

    messages = [...messages, { role: "user", content: toolResultBlocks }];
  }

  const finalText = lastAssistantContent
    .filter((b) => b.type === "text")
    .map((b) => (b as { type: "text"; text: string }).text)
    .join("");

  return {
    finalText,
    turns,
    hitMaxIterations: turns >= maxIterations,
    assistantContent: lastAssistantContent,
    totalUsage: {
      input_tokens: totalInput,
      output_tokens: totalOutput,
      cache_read_input_tokens: totalCacheRead,
      cache_creation_input_tokens: totalCacheCreation,
    },
  };
}
