import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { todayUtc } from "../budget/ledger.js";
import { appendConversationEntry, loadConversation } from "../conversation/store.js";
import type { ConversationEntry } from "../conversation/types.js";
import type { Broadcaster } from "../events/broadcaster.js";
import type { ChatMessage } from "../events/server-event.js";
import { instanceDir } from "../paths.js";
import { loadSettings } from "../settings/reader.js";
import { loadSkills } from "../skills/loader.js";
import { loadTriageRules } from "../triage/rules.js";
import type { MindClient } from "./anthropic-client.js";
import { runBudgetedMind } from "./budgeted-mind.js";
import { builtInTools, skillToTool } from "./skill-tool.js";
import { buildSystemPrompt } from "./system-prompt.js";

const DEFAULT_WARM_TTL_MS = 10 * 60 * 1000;

export type MindWorkerOptions = {
  client: MindClient;
  home: string;
  slug: string;
  chatId: string;
  companyName: string;
  model: string;
  pricePerMTokIn: number;
  pricePerMTokOut: number;
  broadcaster: Broadcaster;
  warmTtlMs?: number;
};

async function tryReadFile(path: string): Promise<string> {
  try {
    return await readFile(path, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return "";
    throw err;
  }
}

export class MindWorker {
  private lastActivityMs = Date.now();
  private readonly warmTtlMs: number;

  constructor(private readonly opts: MindWorkerOptions) {
    this.warmTtlMs = opts.warmTtlMs ?? DEFAULT_WARM_TTL_MS;
  }

  touch(nowMs = Date.now()): void {
    this.lastActivityMs = nowMs;
  }

  isStaleAt(nowMs: number): boolean {
    return nowMs - this.lastActivityMs > this.warmTtlMs;
  }

  async handleUserMessage(text: string): Promise<void> {
    const { home, slug, chatId, broadcaster } = this.opts;
    this.touch();

    broadcaster.emit({ type: "agent_running", instance_slug: slug, chat_id: chatId });

    try {
      const userEntry: ConversationEntry = {
        id: `msg_${Date.now()}_u`,
        role: "user",
        content: [{ type: "text", text }],
        ts: Date.now(),
      };
      await appendConversationEntry(home, slug, chatId, userEntry);

      const userMsg: ChatMessage = {
        id: userEntry.id,
        role: "User",
        content: text,
        created_at: String(userEntry.ts),
        kind: "Message",
      };
      broadcaster.emit({
        type: "chat_message_created",
        instance_slug: slug,
        chat_id: chatId,
        message: userMsg,
      });

      // Assemble context
      const instDir = instanceDir(home, slug);
      const [soul, mood, rhythm, enabledSkills, triageRules, settings, conversation] =
        await Promise.all([
          tryReadFile(join(instDir, "soul.md")),
          tryReadFile(join(instDir, "mood.md")),
          tryReadFile(join(instDir, "rhythm.json")),
          loadSkills(home, slug, { enabledOnly: true }),
          loadTriageRules(home, slug),
          loadSettings(home, slug),
          loadConversation(home, slug, chatId),
        ]);

      const systemPrompt = buildSystemPrompt({
        employeeName: slug,
        companyName: this.opts.companyName,
        soul,
        mood,
        rhythm,
        enabledSkills,
        triageRules,
      });

      const tools = [...builtInTools(), ...enabledSkills.map(skillToTool)];

      const outcome = await runBudgetedMind({
        client: this.opts.client,
        home,
        slug,
        day: todayUtc(),
        capUsd: settings.daily_budget_usd,
        model: this.opts.model,
        pricePerMTokIn: this.opts.pricePerMTokIn,
        pricePerMTokOut: this.opts.pricePerMTokOut,
        systemPrompt,
        tools,
        // Don't include the just-appended user entry twice — pass the pre-append
        // conversation and runBudgetedMind adds it as userMessage.
        conversation,
        userMessage: text,
        executeTool: async (name) => `[${name} not wired in Plan 2]`,
        maxIterations: 10,
      });

      if ("downgraded" in outcome && outcome.downgraded) {
        broadcaster.emit({ type: "agent_stopped", instance_slug: slug, chat_id: chatId });
        return;
      }

      // At this point outcome is MindFullTurnResult (downgraded variant handled above)
      const result = outcome as import("./budgeted-mind.js").BudgetedMindResult & {
        downgraded?: false;
        assistantContent: import("../conversation/types.js").ContentBlock[];
        finalText: string;
      };

      const assistantEntry: ConversationEntry = {
        id: `msg_${Date.now()}_a`,
        role: "assistant",
        content: result.assistantContent,
        ts: Date.now(),
        model: this.opts.model,
      };
      await appendConversationEntry(home, slug, chatId, assistantEntry);

      broadcaster.emit({
        type: "chat_message_created",
        instance_slug: slug,
        chat_id: chatId,
        message: {
          id: assistantEntry.id,
          role: "Assistant",
          content: result.finalText,
          created_at: String(assistantEntry.ts),
          kind: "Message",
          model: this.opts.model,
        },
      });
    } finally {
      broadcaster.emit({ type: "agent_stopped", instance_slug: slug, chat_id: chatId });
    }
  }
}
