import type Anthropic from "@anthropic-ai/sdk";
import type { MindClient } from "./anthropic-client.js";

export type MockMessage = {
  id: string;
  role: "assistant";
  model: string;
  stop_reason: "end_turn" | "tool_use" | "max_tokens" | "stop_sequence" | "pause_turn" | "refusal";
  content: Array<
    | { type: "text"; text: string }
    | { type: "tool_use"; id: string; name: string; input: Record<string, unknown> }
    | { type: "compaction"; content: string }
  >;
  usage: {
    input_tokens: number;
    output_tokens: number;
    cache_read_input_tokens?: number;
    cache_creation_input_tokens?: number;
  };
};

type CreateParams = Anthropic.Messages.MessageCreateParamsNonStreaming;

/**
 * In-memory Anthropic client that replays a queue of canned messages.
 * Records every call for later assertion. Implements the same shape as
 * MindClient so the loop can accept either one.
 */
export class MockAnthropicClient implements MindClient {
  readonly calls: CreateParams[] = [];
  private readonly queue: MockMessage[];

  constructor(responses: MockMessage[]) {
    this.queue = [...responses];
  }

  readonly messages = {
    create: async (params: CreateParams): Promise<MockMessage> => {
      this.calls.push(params);
      const next = this.queue.shift();
      if (!next) throw new Error("MockAnthropicClient: no queued response");
      return next;
    },
    // Streaming mock ships in Task 13; create alone covers Tasks 9-12.
    stream: (): never => {
      throw new Error("MockAnthropicClient.stream: not implemented in this fixture");
    },
  } as unknown as MindClient["messages"];
}
