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

export type MockStream = {
  deltas: string[];
  final: MockMessage;
};

/**
 * In-memory Anthropic client that replays a queue of canned messages.
 * Records every call for later assertion. Implements the same shape as
 * MindClient so the loop can accept either one.
 */
export class MockAnthropicClient implements MindClient {
  readonly calls: CreateParams[] = [];
  readonly streamCalls: CreateParams[] = [];
  private readonly queue: MockMessage[];
  private readonly streamQueue: MockStream[];

  constructor(responses: MockMessage[], opts: { streams?: MockStream[] } = {}) {
    this.queue = [...responses];
    this.streamQueue = [...(opts.streams ?? [])];
  }

  readonly messages = {
    create: async (params: CreateParams): Promise<MockMessage> => {
      this.calls.push(params);
      const next = this.queue.shift();
      if (!next) throw new Error("MockAnthropicClient: no queued response");
      return next;
    },
    stream: (params: CreateParams) => {
      this.streamCalls.push(params);
      const next = this.streamQueue.shift();
      if (!next) throw new Error("MockAnthropicClient.stream: no queued stream");
      const { deltas, final } = next;

      async function* events() {
        yield { type: "message_start", message: { id: final.id } };
        yield { type: "content_block_start", index: 0, content_block: { type: "text", text: "" } };
        for (const delta of deltas) {
          yield {
            type: "content_block_delta",
            index: 0,
            delta: { type: "text_delta", text: delta },
          };
        }
        yield { type: "content_block_stop", index: 0 };
        yield { type: "message_stop" };
      }

      const iter = events();
      return {
        [Symbol.asyncIterator]() {
          return iter;
        },
        finalMessage: async () => final,
      };
    },
  } as unknown as MindClient["messages"];
}
