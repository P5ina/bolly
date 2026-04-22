import Anthropic from "@anthropic-ai/sdk";

/**
 * The subset of the Anthropic SDK surface the mind uses.
 * The mock client in tests implements this same interface.
 */
export type MindClient = {
  messages: {
    create: Anthropic["messages"]["create"];
    stream: Anthropic["messages"]["stream"];
  };
};

export function createAnthropicClient(apiKey: string): MindClient {
  const client = new Anthropic({ apiKey });
  return {
    messages: {
      create: client.messages.create.bind(client.messages),
      stream: client.messages.stream.bind(client.messages),
    },
  };
}
