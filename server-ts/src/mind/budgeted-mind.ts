import { chargeAndCall } from "../budget/charge-and-call.js";
import type { ConversationEntry } from "../conversation/types.js";
import type { MindClient } from "./anthropic-client.js";
import { type MindFullTurnResult, runMindWithTools } from "./loop.js";
import type { ToolDefinition } from "./skill-tool.js";

export type BudgetedMindInputs = {
  client: MindClient;
  home: string;
  slug: string;
  day: string;
  capUsd: number;
  model: string;
  pricePerMTokIn: number;
  pricePerMTokOut: number;
  systemPrompt: string;
  tools: ToolDefinition[];
  conversation: ConversationEntry[];
  userMessage: string;
  executeTool: (name: string, input: Record<string, unknown>) => Promise<string>;
  maxIterations: number;
};

export type BudgetedMindResult = MindFullTurnResult | { downgraded: true; reason: string };

export async function runBudgetedMind(inputs: BudgetedMindInputs): Promise<BudgetedMindResult> {
  const outcome = await chargeAndCall(
    {
      home: inputs.home,
      slug: inputs.slug,
      day: inputs.day,
      capUsd: inputs.capUsd,
      tier: 3,
    },
    async () => {
      const result = await runMindWithTools({
        client: inputs.client,
        model: inputs.model,
        systemPrompt: inputs.systemPrompt,
        tools: inputs.tools,
        conversation: inputs.conversation,
        userMessage: inputs.userMessage,
        executeTool: inputs.executeTool,
        maxIterations: inputs.maxIterations,
      });

      const dollars =
        (result.totalUsage.input_tokens * inputs.pricePerMTokIn) / 1_000_000 +
        (result.totalUsage.output_tokens * inputs.pricePerMTokOut) / 1_000_000;

      return {
        ok: true as const,
        value: result,
        usage: {
          tokens_in: result.totalUsage.input_tokens,
          tokens_out: result.totalUsage.output_tokens,
          dollars,
        },
      };
    },
  );

  if ("downgraded" in outcome) {
    return { downgraded: true, reason: outcome.reason };
  }
  if (!outcome.value) {
    throw new Error("runBudgetedMind: expected value on success outcome");
  }
  return outcome.value;
}
