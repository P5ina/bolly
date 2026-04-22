import type { BudgetState, Event } from "../types.js";

export type OutreachHint = {
  channel: "push" | "email" | "digest";
  title: string;
  ts: number;
  urgency: "low" | "medium" | "high";
};

export type TriagePromptInputs = {
  soulSnippet: string;
  mood: string;
  budgetState: BudgetState;
  dollarsSpent: number;
  capUsd: number;
  triageRules: string;
  event: Event;
  recentOutreach: OutreachHint[];
};

const EVENT_PAYLOAD_MAX = 1024;

function truncateJson(value: unknown, limit: number): string {
  const serialized = JSON.stringify(value);
  if (serialized.length <= limit) return serialized;
  return `${serialized.slice(0, limit)}[truncated]`;
}

function budgetDirective(state: BudgetState): string {
  if (state === "tight") return "Only escalate if truly urgent.";
  if (state === "suppressed") return "budget cap reached — prefer digest.";
  return "";
}

export function buildTriagePrompt(inputs: TriagePromptInputs): string {
  const {
    soulSnippet,
    mood,
    budgetState,
    dollarsSpent,
    capUsd,
    triageRules,
    event,
    recentOutreach,
  } = inputs;

  const outreachLines = recentOutreach
    .map((o) => `  - ${o.channel}: ${o.title} (${o.urgency})`)
    .join("\n");

  const directive = budgetDirective(budgetState);

  return `You are the triage layer for Bolly. Decide: ignore | digest | escalate.

<soul>${soulSnippet}</soul>
<mood>${mood}</mood>
<budget_state>${dollarsSpent.toFixed(2)}/${capUsd.toFixed(2)} — ${budgetState}</budget_state>
${directive ? `<directive>${directive}</directive>\n` : ""}<triage_rules>
${triageRules}
</triage_rules>

<recent_outreach>
${outreachLines || "  (none)"}
</recent_outreach>

<event source="${event.source}" id="${event.id}" ts="${event.ts}">
${truncateJson(event.payload, EVENT_PAYLOAD_MAX)}
</event>

Respond with exactly one line:
DECISION=<ignore|digest|escalate> REASON=<short sentence>`;
}
