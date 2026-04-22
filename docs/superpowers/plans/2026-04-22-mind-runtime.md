# Bolly v1 — Mind Runtime Implementation Plan (Plan 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Compose Plan 1's foundations into a runnable TypeScript server. One employee sends a user message, the mind loops Claude via Messages API (with custom tools, prompt caching, compaction, streaming), persists the conversation, and streams back `ServerEvent`s over WebSocket. No event queue, no triage, no outreach delivery, no cross-instance — those land in Plans 3–5.

**Architecture:**
- Hono HTTP server with auth middleware, `POST /api/chat`, and WebSocket at `/api/ws`.
- One in-process `MindWorker` per active employee (lazy-started, warm for 10 min after last activity).
- Each worker drives the `runMind` loop: Messages API → handle `tool_use` → execute custom tools inline → loop until `end_turn`. Streaming via `messages.stream()`; compaction via `compact-2026-01-12` beta header; prompt caching via top-level `cache_control`.
- Conversation persists to `instances/{slug}/chats/{chat_id}/conversation.json` (atomic writes).
- `ServerEvent` wire format is byte-compatible with the current Rust backend so the SvelteKit client runs unmodified.

**Tech Stack (added in Plan 2):**
- `@anthropic-ai/sdk` ^0.40.0 (Anthropic Messages API client with `.messages.create`, `.messages.stream`, beta support for `compact-2026-01-12`)
- `hono` ^4.6.0 + `@hono/node-server` ^1.13.0 (HTTP framework + Node adapter)
- `@hono/node-ws` (WebSocket upgrade)

**Depends on Plan 1 foundations:**
- `parseSkill`, `loadSkills` — read enabled skills from disk
- `loadTriageRules` — system prompt context
- `loadSettings` — per-employee budget / prefs
- `chargeAndCall`, `loadDaily`, `recordSpend` — budget gate around Anthropic calls
- `conversationFile`, `instanceDir` — path helpers
- `readJson`, `writeJson`, `atomicWrite` — persistence
- `Skill`, `Settings` types

**Scope (what ships):**
- HTTP server with auth, chat endpoint, WebSocket broadcast
- MindWorker: lazy-started per employee, warm TTL, graceful shutdown
- Full Messages API mind loop: text, tool use, compaction, max_tokens continuation, streaming
- Custom-tool registry (one tool per enabled skill, plus built-in outreach stubs)
- System prompt assembly from soul.md + mood + rhythm + skills + triage
- Conversation persistence (load / append / snapshot)
- Budget integration (every tier-3 Anthropic call goes through `chargeAndCall`)
- ServerEvent types matching Rust wire format exactly
- In-memory mock Anthropic client for unit tests
- One real-SDK integration test (gated by `INTEGRATION=1`)

**Out of scope (later plans):**
- Event queue, triage gate, scheduled jobs → Plan 3
- Outreach delivery (push / email / digest) → Plan 4
- Cross-instance events + shared directory → Plan 5
- Polish, E2E, cost validation, `docker compose` image → Plan 6
- Client-side SvelteKit updates (separate, parallel track)

**Commit convention:** One commit per task. Co-Authored-By trailer on every commit. `feat:` / `test:` / `chore:` / `fix:` prefixes.

---

## File Structure

```
server-ts/
├── package.json                   MODIFIED: add sdk, hono deps
└── src/
    ├── config.ts                  NEW: env loader (API key, home, port)
    ├── config.test.ts
    ├── conversation/
    │   ├── types.ts               NEW: ConversationEntry, ContentBlock
    │   ├── types.test.ts
    │   ├── store.ts               NEW: load / append / snapshot
    │   └── store.test.ts
    ├── mind/
    │   ├── system-prompt.ts       NEW: assemble system prompt
    │   ├── system-prompt.test.ts
    │   ├── skill-tool.ts          NEW: Skill → Anthropic Tool
    │   ├── skill-tool.test.ts
    │   ├── anthropic-client.ts    NEW: real client factory
    │   ├── mock-client.ts         NEW: in-memory fake
    │   ├── mock-client.test.ts
    │   ├── loop.ts                NEW: the mind loop
    │   ├── loop.test.ts
    │   ├── worker.ts              NEW: per-employee MindWorker
    │   ├── worker.test.ts
    │   └── pool.ts                NEW: worker pool
    │   └── pool.test.ts
    ├── events/
    │   ├── server-event.ts        NEW: ServerEvent union + schemas
    │   ├── server-event.test.ts
    │   ├── broadcaster.ts         NEW: WS broadcast channel
    │   └── broadcaster.test.ts
    ├── http/
    │   ├── auth.ts                NEW: bearer-token middleware
    │   ├── auth.test.ts
    │   ├── chat.ts                NEW: POST /api/chat
    │   ├── ws.ts                  NEW: WebSocket endpoint
    │   └── server.ts              NEW: Hono app
    ├── main.ts                    NEW: boot + shutdown
    └── index.ts                   MODIFIED: add new public exports
```

Expected size after Plan 2: ~1500–2000 LoC in `src/` + ~1500 LoC of tests. Still well inside the 4000-LoC budget.

---

## Task 1: Add SDK + HTTP deps

**Files:**
- Modify: `server-ts/package.json`

- [ ] **Step 1: Add dependencies**

From `server-ts/`:

```bash
pnpm add @anthropic-ai/sdk@^0.40.0 hono@^4.6.0 @hono/node-server@^1.13.0 @hono/node-ws@^1.0.0
```

- [ ] **Step 2: Verify install**

```bash
pnpm install
pnpm check
pnpm test
```

Expected: install succeeds; `check` passes (new deps don't affect foundation); `test` still reports 82/82 passing.

- [ ] **Step 3: Commit**

```bash
git add server-ts/package.json server-ts/pnpm-lock.yaml
git commit -m "$(cat <<'EOF'
chore(server-ts): add Anthropic SDK + Hono for mind runtime

Adds @anthropic-ai/sdk (Messages API client, compaction beta),
hono (HTTP framework) and @hono/node-server / @hono/node-ws
(Node adapter + WebSocket upgrade).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Config loader

**Files:**
- Create: `server-ts/src/config.ts`
- Create: `server-ts/src/config.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { describe, expect, it } from "vitest";
import { loadConfig, type RuntimeConfig } from "./config.js";

describe("loadConfig", () => {
  it("reads required fields from env", () => {
    const cfg = loadConfig({
      ANTHROPIC_API_KEY: "sk-ant-test",
      BOLLY_HOME: "/tmp/bolly",
    });
    expect(cfg.anthropicApiKey).toBe("sk-ant-test");
    expect(cfg.bollyHome).toBe("/tmp/bolly");
  });

  it("defaults http port to 4242", () => {
    const cfg = loadConfig({
      ANTHROPIC_API_KEY: "sk-ant-test",
      BOLLY_HOME: "/tmp/bolly",
    });
    expect(cfg.httpPort).toBe(4242);
  });

  it("reads BOLLY_HTTP_PORT override", () => {
    const cfg = loadConfig({
      ANTHROPIC_API_KEY: "sk-ant-test",
      BOLLY_HOME: "/tmp/bolly",
      BOLLY_HTTP_PORT: "8080",
    });
    expect(cfg.httpPort).toBe(8080);
  });

  it("reads optional BOLLY_AUTH_TOKEN", () => {
    const cfg = loadConfig({
      ANTHROPIC_API_KEY: "sk-ant-test",
      BOLLY_HOME: "/tmp/bolly",
      BOLLY_AUTH_TOKEN: "secret",
    });
    expect(cfg.authToken).toBe("secret");
  });

  it("authToken is undefined when BOLLY_AUTH_TOKEN is unset", () => {
    const cfg = loadConfig({
      ANTHROPIC_API_KEY: "sk-ant-test",
      BOLLY_HOME: "/tmp/bolly",
    });
    expect(cfg.authToken).toBeUndefined();
  });

  it("throws when ANTHROPIC_API_KEY is missing", () => {
    expect(() => loadConfig({ BOLLY_HOME: "/tmp/bolly" })).toThrow(/ANTHROPIC_API_KEY/);
  });

  it("throws when BOLLY_HOME is missing", () => {
    expect(() => loadConfig({ ANTHROPIC_API_KEY: "sk-ant-test" })).toThrow(/BOLLY_HOME/);
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/config.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
export type RuntimeConfig = {
  anthropicApiKey: string;
  bollyHome: string;
  httpPort: number;
  authToken: string | undefined;
};

type Env = Record<string, string | undefined>;

/**
 * Load runtime config from a plain env-like object.
 * Accepts `process.env` at boot; accepts a mock at test time.
 */
export function loadConfig(env: Env): RuntimeConfig {
  const anthropicApiKey = env.ANTHROPIC_API_KEY;
  if (!anthropicApiKey) throw new Error("ANTHROPIC_API_KEY is required");

  const bollyHome = env.BOLLY_HOME;
  if (!bollyHome) throw new Error("BOLLY_HOME is required");

  const port = env.BOLLY_HTTP_PORT ? Number(env.BOLLY_HTTP_PORT) : 4242;
  if (!Number.isFinite(port) || port <= 0) {
    throw new Error(`BOLLY_HTTP_PORT is not a valid port: ${env.BOLLY_HTTP_PORT}`);
  }

  return {
    anthropicApiKey,
    bollyHome,
    httpPort: port,
    authToken: env.BOLLY_AUTH_TOKEN,
  };
}
```

- [ ] **Step 4: Run test, verify PASS (7 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/config.ts server-ts/src/config.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add runtime config loader

Reads ANTHROPIC_API_KEY, BOLLY_HOME, optional BOLLY_HTTP_PORT (default 4242)
and optional BOLLY_AUTH_TOKEN from a plain env object. Accepts process.env
at boot and a mock at test time.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Conversation entry types

**Files:**
- Create: `server-ts/src/conversation/types.ts`
- Create: `server-ts/src/conversation/types.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { describe, expect, it } from "vitest";
import {
  ContentBlockSchema,
  ConversationEntrySchema,
  type ConversationEntry,
} from "./types.js";

describe("ContentBlockSchema", () => {
  it("accepts a text block", () => {
    const parsed = ContentBlockSchema.parse({ type: "text", text: "hi" });
    expect(parsed).toEqual({ type: "text", text: "hi" });
  });

  it("accepts a tool_use block", () => {
    const parsed = ContentBlockSchema.parse({
      type: "tool_use",
      id: "toolu_1",
      name: "send_push",
      input: { title: "hi" },
    });
    expect(parsed.type).toBe("tool_use");
  });

  it("accepts a tool_result block", () => {
    const parsed = ContentBlockSchema.parse({
      type: "tool_result",
      tool_use_id: "toolu_1",
      content: "done",
    });
    expect(parsed.type).toBe("tool_result");
  });

  it("rejects unknown block types", () => {
    expect(() => ContentBlockSchema.parse({ type: "wut", x: 1 })).toThrow();
  });
});

describe("ConversationEntrySchema", () => {
  it("parses a minimal user entry", () => {
    const entry = {
      id: "msg_1",
      role: "user" as const,
      content: [{ type: "text" as const, text: "hello" }],
      ts: 1714000000000,
    };
    const parsed: ConversationEntry = ConversationEntrySchema.parse(entry);
    expect(parsed.role).toBe("user");
  });

  it("parses an assistant entry with model field", () => {
    const entry = {
      id: "msg_2",
      role: "assistant" as const,
      content: [{ type: "text" as const, text: "hi back" }],
      ts: 1714000001000,
      model: "claude-sonnet-4-6",
    };
    const parsed = ConversationEntrySchema.parse(entry);
    expect(parsed.model).toBe("claude-sonnet-4-6");
  });

  it("rejects entries with invalid role", () => {
    expect(() =>
      ConversationEntrySchema.parse({
        id: "msg_3",
        role: "system",
        content: [],
        ts: 0,
      }),
    ).toThrow();
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/conversation/types.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
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
```

- [ ] **Step 4: Run test, verify PASS (7 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/conversation/types.ts server-ts/src/conversation/types.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add conversation entry schema

Discriminated union on content block: text, tool_use, tool_result, compaction.
Each ConversationEntry has id, role, content blocks, timestamp, and optional
model name (assistant turns only).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Conversation store

**Files:**
- Create: `server-ts/src/conversation/store.ts`
- Create: `server-ts/src/conversation/store.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  appendConversationEntry,
  loadConversation,
  saveConversation,
} from "./store.js";
import type { ConversationEntry } from "./types.js";

function entry(id: string, ts: number): ConversationEntry {
  return {
    id,
    role: "user",
    content: [{ type: "text", text: `hello ${id}` }],
    ts,
  };
}

describe("conversation store", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-conv-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("loadConversation returns empty array when file is missing", async () => {
    const result = await loadConversation(home, "alice", "default");
    expect(result).toEqual([]);
  });

  it("saveConversation then loadConversation round-trips", async () => {
    await saveConversation(home, "alice", "default", [entry("a", 100), entry("b", 200)]);
    const result = await loadConversation(home, "alice", "default");
    expect(result.map((e) => e.id)).toEqual(["a", "b"]);
  });

  it("appendConversationEntry writes a new entry to a fresh file", async () => {
    await appendConversationEntry(home, "alice", "default", entry("a", 100));
    const result = await loadConversation(home, "alice", "default");
    expect(result).toHaveLength(1);
    expect(result[0]?.id).toBe("a");
  });

  it("appendConversationEntry appends to an existing file", async () => {
    await appendConversationEntry(home, "alice", "default", entry("a", 100));
    await appendConversationEntry(home, "alice", "default", entry("b", 200));
    const result = await loadConversation(home, "alice", "default");
    expect(result.map((e) => e.id)).toEqual(["a", "b"]);
  });

  it("saveConversation writes atomically (no .tmp residue)", async () => {
    await saveConversation(home, "alice", "default", [entry("a", 100)]);
    const { readdir } = await import("node:fs/promises");
    const dir = join(home, "instances", "alice", "chats", "default");
    const entries = await readdir(dir);
    expect(entries).toEqual(["conversation.json"]);
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/conversation/store.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
import { readJson, writeJson } from "../json-file.js";
import { conversationFile } from "../paths.js";
import {
  type Conversation,
  ConversationSchema,
  type ConversationEntry,
} from "./types.js";

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
```

- [ ] **Step 4: Run test, verify PASS (5 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/conversation/store.ts server-ts/src/conversation/store.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add conversation store

load / save / append helpers backed by conversation.json.
All writes go through the atomic json-file helper.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: System prompt assembler

**Files:**
- Create: `server-ts/src/mind/system-prompt.ts`
- Create: `server-ts/src/mind/system-prompt.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { describe, expect, it } from "vitest";
import type { Skill } from "../types.js";
import { buildSystemPrompt } from "./system-prompt.js";

const SKILL: Skill = {
  frontmatter: {
    name: "email-check",
    triggers: [{ scheduled: "every weekday at 8am" }],
    tools: [],
    enabled: true,
  },
  body: "When triggered, read unread emails and surface urgent items.",
  path: "/skills/email-check.md",
};

describe("buildSystemPrompt", () => {
  it("includes employee name and company name", () => {
    const prompt = buildSystemPrompt({
      employeeName: "alice",
      companyName: "Acme",
      soul: "I am calm and attentive.",
      mood: "focused",
      rhythm: "morning person",
      enabledSkills: [],
      triageRules: "",
    });
    expect(prompt).toContain("alice");
    expect(prompt).toContain("Acme");
  });

  it("embeds soul / mood / rhythm", () => {
    const prompt = buildSystemPrompt({
      employeeName: "alice",
      companyName: "Acme",
      soul: "MY_SOUL",
      mood: "MY_MOOD",
      rhythm: "MY_RHYTHM",
      enabledSkills: [],
      triageRules: "",
    });
    expect(prompt).toContain("MY_SOUL");
    expect(prompt).toContain("MY_MOOD");
    expect(prompt).toContain("MY_RHYTHM");
  });

  it("lists each enabled skill by name and tool", () => {
    const prompt = buildSystemPrompt({
      employeeName: "alice",
      companyName: "Acme",
      soul: "",
      mood: "",
      rhythm: "",
      enabledSkills: [SKILL],
      triageRules: "",
    });
    expect(prompt).toContain("email-check");
    expect(prompt).toContain("run_email_check");
  });

  it("skips a skills block entirely when no skills are enabled", () => {
    const prompt = buildSystemPrompt({
      employeeName: "alice",
      companyName: "Acme",
      soul: "",
      mood: "",
      rhythm: "",
      enabledSkills: [],
      triageRules: "",
    });
    expect(prompt).not.toContain("<skills>");
  });

  it("includes triage rules when present", () => {
    const prompt = buildSystemPrompt({
      employeeName: "alice",
      companyName: "Acme",
      soul: "",
      mood: "",
      rhythm: "",
      enabledSkills: [],
      triageRules: "ALWAYS escalate urgent email",
    });
    expect(prompt).toContain("ALWAYS escalate urgent email");
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/mind/system-prompt.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
import type { Skill } from "../types.js";
import { skillToToolName } from "./skill-tool.js";

export type SystemPromptInputs = {
  employeeName: string;
  companyName: string;
  soul: string;
  mood: string;
  rhythm: string;
  enabledSkills: Skill[];
  triageRules: string;
};

/**
 * Build the system prompt for the mind. Output is stable for the same inputs,
 * so it caches well via cache_control.
 */
export function buildSystemPrompt(inputs: SystemPromptInputs): string {
  const {
    employeeName,
    companyName,
    soul,
    mood,
    rhythm,
    enabledSkills,
    triageRules,
  } = inputs;

  const skillsBlock =
    enabledSkills.length === 0
      ? ""
      : `

<skills>
${enabledSkills
  .map((s) => {
    const tool = skillToToolName(s.frontmatter.name);
    return `- ${s.frontmatter.name} (tool: ${tool})\n  ${s.body.trim()}`;
  })
  .join("\n\n")}
</skills>`;

  const triageBlock = triageRules.trim()
    ? `

<triage_rules>
${triageRules.trim()}
</triage_rules>`
    : "";

  return `You are Bolly, the AI coworker for ${employeeName} at ${companyName}.

<persona>${soul}</persona>
<mood>${mood}</mood>
<rhythm>${rhythm}</rhythm>${skillsBlock}${triageBlock}

Use the custom tools available to you when appropriate. You may reach out
to ${employeeName} via send_push, send_email, or defer_for_digest.`;
}
```

- [ ] **Step 4: Run test, verify PASS (5 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/mind/system-prompt.ts server-ts/src/mind/system-prompt.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add system prompt assembler

Composes soul + mood + rhythm + enabled skills + triage rules into a
cache-stable system prompt string. One skill → one custom tool name.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Skill → tool mapping

**Files:**
- Create: `server-ts/src/mind/skill-tool.ts`
- Create: `server-ts/src/mind/skill-tool.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { describe, expect, it } from "vitest";
import type { Skill } from "../types.js";
import { builtInTools, skillToTool, skillToToolName } from "./skill-tool.js";

const SKILL: Skill = {
  frontmatter: {
    name: "email-morning-check",
    triggers: [],
    tools: [],
    enabled: true,
  },
  body: "Read unread email and summarize urgent items.",
  path: "/skills/x.md",
};

describe("skillToToolName", () => {
  it("prefixes with run_ and replaces dashes with underscores", () => {
    expect(skillToToolName("email-morning-check")).toBe("run_email_morning_check");
  });

  it("preserves underscores", () => {
    expect(skillToToolName("simple_name")).toBe("run_simple_name");
  });
});

describe("skillToTool", () => {
  it("produces a tool definition with the right name and description", () => {
    const tool = skillToTool(SKILL);
    expect(tool.name).toBe("run_email_morning_check");
    expect(tool.description).toContain("Read unread email");
  });

  it("accepts an input schema with a free-form context argument", () => {
    const tool = skillToTool(SKILL);
    expect(tool.input_schema.type).toBe("object");
    expect(tool.input_schema.properties).toHaveProperty("context");
  });
});

describe("builtInTools", () => {
  it("provides send_push, send_email, defer_for_digest", () => {
    const names = builtInTools().map((t) => t.name);
    expect(names).toContain("send_push");
    expect(names).toContain("send_email");
    expect(names).toContain("defer_for_digest");
  });

  it("send_push has a title and body schema", () => {
    const push = builtInTools().find((t) => t.name === "send_push");
    expect(push?.input_schema.required).toContain("title");
    expect(push?.input_schema.required).toContain("body");
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/mind/skill-tool.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
import type { Skill } from "../types.js";

/**
 * A JSON-schema object suitable for the Anthropic Messages API `tools` array.
 */
export type ToolDefinition = {
  name: string;
  description: string;
  input_schema: {
    type: "object";
    properties: Record<string, unknown>;
    required?: string[];
  };
};

export function skillToToolName(skillName: string): string {
  return `run_${skillName.replace(/-/g, "_")}`;
}

export function skillToTool(skill: Skill): ToolDefinition {
  return {
    name: skillToToolName(skill.frontmatter.name),
    description: skill.body.trim(),
    input_schema: {
      type: "object",
      properties: {
        context: {
          type: "string",
          description: "Free-form context describing why this skill should run",
        },
      },
      required: [],
    },
  };
}

export function builtInTools(): ToolDefinition[] {
  return [
    {
      name: "send_push",
      description:
        "Send a push notification to the employee. Use for time-sensitive, short messages worth interrupting for.",
      input_schema: {
        type: "object",
        properties: {
          title: { type: "string", description: "Short title, <100 chars" },
          body: { type: "string", description: "Body text" },
          urgency: {
            type: "string",
            enum: ["low", "medium", "high"],
            description: "Interrupt priority",
          },
        },
        required: ["title", "body"],
      },
    },
    {
      name: "send_email",
      description:
        "Send an email to the employee. Use for longer, async messages such as morning briefs or summaries.",
      input_schema: {
        type: "object",
        properties: {
          subject: { type: "string" },
          body_markdown: { type: "string" },
        },
        required: ["subject", "body_markdown"],
      },
    },
    {
      name: "defer_for_digest",
      description:
        "File a note for the employee's next return to the app. Use for anything not pressing.",
      input_schema: {
        type: "object",
        properties: {
          summary: { type: "string" },
        },
        required: ["summary"],
      },
    },
  ];
}
```

- [ ] **Step 4: Run test, verify PASS (6 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/mind/skill-tool.ts server-ts/src/mind/skill-tool.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add skill → Anthropic tool mapping

skillToToolName normalizes skill names into run_* tool names.
skillToTool converts a Skill into a JSON-schema tool definition.
builtInTools returns the outreach tools (send_push, send_email, defer_for_digest)
that every employee's mind has access to.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Anthropic client factory

**Files:**
- Create: `server-ts/src/mind/anthropic-client.ts`
- Create: `server-ts/src/mind/anthropic-client.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { describe, expect, it } from "vitest";
import { createAnthropicClient, type MindClient } from "./anthropic-client.js";

describe("createAnthropicClient", () => {
  it("returns an object with messages.create and messages.stream methods", () => {
    const client: MindClient = createAnthropicClient("sk-ant-test");
    expect(typeof client.messages.create).toBe("function");
    expect(typeof client.messages.stream).toBe("function");
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/mind/anthropic-client.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
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
```

- [ ] **Step 4: Run test, verify PASS (1 test)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/mind/anthropic-client.ts server-ts/src/mind/anthropic-client.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add Anthropic client factory

Thin factory around @anthropic-ai/sdk that returns only the two surfaces
the mind uses: messages.create and messages.stream. Exports a MindClient
type so the mock client in tests stays structurally compatible.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Mock Anthropic client

**Files:**
- Create: `server-ts/src/mind/mock-client.ts`
- Create: `server-ts/src/mind/mock-client.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { describe, expect, it } from "vitest";
import { MockAnthropicClient, type MockMessage } from "./mock-client.js";

const TEXT_MESSAGE: MockMessage = {
  id: "msg_1",
  role: "assistant",
  model: "mock",
  stop_reason: "end_turn",
  content: [{ type: "text", text: "hello" }],
  usage: { input_tokens: 100, output_tokens: 20 },
};

const TOOL_USE_MESSAGE: MockMessage = {
  id: "msg_2",
  role: "assistant",
  model: "mock",
  stop_reason: "tool_use",
  content: [
    {
      type: "tool_use",
      id: "toolu_1",
      name: "send_push",
      input: { title: "hi", body: "there" },
    },
  ],
  usage: { input_tokens: 120, output_tokens: 30 },
};

describe("MockAnthropicClient.messages.create", () => {
  it("returns queued messages in FIFO order", async () => {
    const client = new MockAnthropicClient([TEXT_MESSAGE]);
    const m = await client.messages.create({
      model: "mock",
      max_tokens: 1024,
      messages: [{ role: "user", content: "hi" }],
    });
    expect(m.content[0]).toEqual({ type: "text", text: "hello" });
  });

  it("records each call so tests can assert on the request shape", async () => {
    const client = new MockAnthropicClient([TEXT_MESSAGE]);
    await client.messages.create({
      model: "mock",
      max_tokens: 1024,
      messages: [{ role: "user", content: "hi" }],
    });
    expect(client.calls).toHaveLength(1);
    expect(client.calls[0]?.messages[0]?.content).toBe("hi");
  });

  it("throws when queue is empty", async () => {
    const client = new MockAnthropicClient([]);
    await expect(
      client.messages.create({
        model: "mock",
        max_tokens: 1024,
        messages: [{ role: "user", content: "hi" }],
      }),
    ).rejects.toThrow(/no queued response/i);
  });

  it("supports a tool_use stop reason", async () => {
    const client = new MockAnthropicClient([TOOL_USE_MESSAGE]);
    const m = await client.messages.create({
      model: "mock",
      max_tokens: 1024,
      messages: [{ role: "user", content: "push me" }],
    });
    expect(m.stop_reason).toBe("tool_use");
    expect(m.content[0]).toMatchObject({ type: "tool_use", name: "send_push" });
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/mind/mock-client.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
import type Anthropic from "@anthropic-ai/sdk";
import type { MindClient } from "./anthropic-client.js";

export type MockMessage = {
  id: string;
  role: "assistant";
  model: string;
  stop_reason:
    | "end_turn"
    | "tool_use"
    | "max_tokens"
    | "stop_sequence"
    | "pause_turn"
    | "refusal";
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
    // Streaming mock ships in a later task; create alone covers Tasks 10–13.
    stream: (): never => {
      throw new Error("MockAnthropicClient.stream: not implemented in this fixture");
    },
  } as unknown as MindClient["messages"];
}
```

- [ ] **Step 4: Run test, verify PASS (4 tests)**

- [ ] **Step 5: `pnpm check`**

If the `as unknown as MindClient["messages"]` triggers a biome rule, accept it — this is the minimum cast to let the mock fake the SDK surface. The real SDK types are deeply nested, and re-typing them in the mock would be high-maintenance.

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/mind/mock-client.ts server-ts/src/mind/mock-client.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add in-memory mock Anthropic client

Replays a queue of canned MockMessage responses; records each call for
assertions. Covers messages.create only; streaming mock added in Task 14.
Implements MindClient so tests can swap it in without adapters.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: Mind loop — single-turn text response

**Files:**
- Create: `server-ts/src/mind/loop.ts`
- Create: `server-ts/src/mind/loop.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { describe, expect, it } from "vitest";
import { MockAnthropicClient } from "./mock-client.js";
import { runMindTurn } from "./loop.js";

describe("runMindTurn — single-turn text", () => {
  it("sends a user message and returns the assistant text", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "claude-sonnet-4-6",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "hi there" }],
        usage: { input_tokens: 100, output_tokens: 20 },
      },
    ]);

    const result = await runMindTurn({
      client,
      model: "claude-sonnet-4-6",
      systemPrompt: "You are Bolly.",
      tools: [],
      conversation: [],
      userMessage: "hello",
    });

    expect(result.stopReason).toBe("end_turn");
    expect(result.assistantContent).toEqual([{ type: "text", text: "hi there" }]);
    expect(result.totalUsage.input_tokens).toBe(100);
    expect(result.totalUsage.output_tokens).toBe(20);
  });

  it("passes the full message array including the new user message", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "claude-sonnet-4-6",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "ok" }],
        usage: { input_tokens: 50, output_tokens: 5 },
      },
    ]);

    await runMindTurn({
      client,
      model: "claude-sonnet-4-6",
      systemPrompt: "You are Bolly.",
      tools: [],
      conversation: [
        { id: "p1", role: "user", content: [{ type: "text", text: "prior" }], ts: 1 },
      ],
      userMessage: "hello",
    });

    const call = client.calls[0];
    expect(call?.messages).toHaveLength(2);
    expect(call?.messages[0]?.content).toEqual([{ type: "text", text: "prior" }]);
    expect(call?.messages[1]?.content).toBe("hello");
  });

  it("includes the system prompt on the request", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "." }],
        usage: { input_tokens: 10, output_tokens: 1 },
      },
    ]);

    await runMindTurn({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "hi",
    });

    expect(client.calls[0]?.system).toBeDefined();
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/mind/loop.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
import type Anthropic from "@anthropic-ai/sdk";
import type { MindClient } from "./anthropic-client.js";
import type { ContentBlock, ConversationEntry } from "../conversation/types.js";
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
```

- [ ] **Step 4: Run test, verify PASS (3 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/mind/loop.ts server-ts/src/mind/loop.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add single-turn mind loop

runMindTurn converts conversation history + a new user message into an
Anthropic messages.create request, returns the assistant content and
aggregated token usage. Later tasks extend this with tool_use handling,
max_tokens continuation, caching, compaction, and streaming.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: Mind loop — tool-use handling

**Files:**
- Modify: `server-ts/src/mind/loop.ts`
- Modify: `server-ts/src/mind/loop.test.ts`

- [ ] **Step 1: Extend the test**

Append to `server-ts/src/mind/loop.test.ts`:

```typescript
import { runMindWithTools } from "./loop.js";

describe("runMindWithTools — tool-use loop", () => {
  it("executes a tool call and continues to end_turn", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "tool_use",
        content: [
          {
            type: "tool_use",
            id: "toolu_a",
            name: "send_push",
            input: { title: "Q2 review", body: "urgent" },
          },
        ],
        usage: { input_tokens: 100, output_tokens: 10 },
      },
      {
        id: "msg_2",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "sent" }],
        usage: { input_tokens: 120, output_tokens: 5 },
      },
    ]);

    const toolCalls: Array<{ name: string; input: unknown }> = [];
    const result = await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "push me",
      executeTool: async (name, input) => {
        toolCalls.push({ name, input });
        return "ok";
      },
      maxIterations: 5,
    });

    expect(toolCalls).toEqual([
      { name: "send_push", input: { title: "Q2 review", body: "urgent" } },
    ]);
    expect(result.finalText).toBe("sent");
    expect(result.turns).toBe(2);
    expect(result.totalUsage.input_tokens).toBe(220);
    expect(result.totalUsage.output_tokens).toBe(15);
  });

  it("stops with an error if an unknown tool is called", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "tool_use",
        content: [
          {
            type: "tool_use",
            id: "toolu_b",
            name: "does_not_exist",
            input: {},
          },
        ],
        usage: { input_tokens: 100, output_tokens: 10 },
      },
      {
        id: "msg_2",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "caught error" }],
        usage: { input_tokens: 120, output_tokens: 5 },
      },
    ]);

    const result = await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "do it",
      executeTool: async (name) => {
        throw new Error(`unknown tool: ${name}`);
      },
      maxIterations: 5,
    });

    expect(result.finalText).toBe("caught error");
    // Second call should have the is_error tool_result
    const secondCall = client.calls[1];
    const toolResult = secondCall?.messages[secondCall.messages.length - 1];
    expect(toolResult?.role).toBe("user");
    const firstBlock = (toolResult?.content as Array<{ type: string; is_error?: boolean }>)[0];
    expect(firstBlock?.type).toBe("tool_result");
    expect(firstBlock?.is_error).toBe(true);
  });

  it("stops at maxIterations", async () => {
    const toolUseMsg = {
      id: "msg_loop",
      role: "assistant" as const,
      model: "mock",
      stop_reason: "tool_use" as const,
      content: [
        {
          type: "tool_use" as const,
          id: "toolu_x",
          name: "send_push",
          input: { title: "x", body: "y" },
        },
      ],
      usage: { input_tokens: 10, output_tokens: 2 },
    };
    const client = new MockAnthropicClient([toolUseMsg, toolUseMsg, toolUseMsg]);

    const result = await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "push me",
      executeTool: async () => "ok",
      maxIterations: 2,
    });

    expect(result.turns).toBe(2);
    expect(result.hitMaxIterations).toBe(true);
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/mind/loop.test.ts
```

- [ ] **Step 3: Extend the implementation**

Append to `server-ts/src/mind/loop.ts`:

```typescript
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
      { role: "assistant", content: response.content as unknown as Anthropic.Messages.ContentBlockParam[] },
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
```

- [ ] **Step 4: Run test, verify PASS (all test blocks pass including the 3 new ones)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/mind/loop.ts server-ts/src/mind/loop.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add tool-use loop to the mind

runMindWithTools iterates the Messages API, executing custom tool_use
blocks via a supplied callback. Tool errors become is_error: true
tool_result blocks rather than exceptions, so the model can observe
and recover. Bounded by maxIterations.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: Mind loop — max_tokens continuation + prompt caching

**Files:**
- Modify: `server-ts/src/mind/loop.ts`
- Modify: `server-ts/src/mind/loop.test.ts`

- [ ] **Step 1: Extend the test**

Append to `server-ts/src/mind/loop.test.ts`:

```typescript
describe("runMindWithTools — max_tokens continuation", () => {
  it("re-prompts on stop_reason: max_tokens and merges output", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "max_tokens",
        content: [{ type: "text", text: "part one" }],
        usage: { input_tokens: 100, output_tokens: 4096 },
      },
      {
        id: "msg_2",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "part two" }],
        usage: { input_tokens: 200, output_tokens: 100 },
      },
    ]);

    const result = await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "write a long thing",
      executeTool: async () => "unused",
      maxIterations: 5,
    });

    expect(result.finalText).toBe("part two");
    expect(result.turns).toBe(2);
    // Second call should include a continuation nudge as the last user message
    const secondCall = client.calls[1];
    const lastMsg = secondCall?.messages[secondCall.messages.length - 1];
    expect(lastMsg?.role).toBe("user");
    expect(JSON.stringify(lastMsg?.content)).toMatch(/continue/i);
  });
});

describe("runMindWithTools — prompt caching", () => {
  it("passes top-level cache_control: ephemeral by default", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "." }],
        usage: { input_tokens: 10, output_tokens: 1 },
      },
    ]);

    await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "hi",
      executeTool: async () => "unused",
      maxIterations: 5,
    });

    const call = client.calls[0] as unknown as { cache_control?: { type: string } };
    expect(call.cache_control).toEqual({ type: "ephemeral" });
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/mind/loop.test.ts
```

- [ ] **Step 3: Extend the implementation**

In `server-ts/src/mind/loop.ts`, replace the body of `runMindWithTools`'s `for` loop with one that handles `max_tokens` and that passes `cache_control` on every call. Also add the continuation constant.

Find:
```typescript
    if (response.stop_reason !== "tool_use") break;
```
Replace with:
```typescript
    if (response.stop_reason === "max_tokens") {
      messages = [
        ...messages,
        { role: "user", content: CONTINUATION_PROMPT },
      ];
      continue;
    }
    if (response.stop_reason !== "tool_use") break;
```

Find:
```typescript
    const response = await client.messages.create({
      model,
      max_tokens: 4096,
      system: systemPrompt,
      tools: tools as unknown as Anthropic.Messages.ToolUnion[],
      messages,
    });
```
Replace with:
```typescript
    const response = await client.messages.create({
      model,
      max_tokens: 4096,
      system: systemPrompt,
      tools: tools as unknown as Anthropic.Messages.ToolUnion[],
      messages,
      // biome-ignore lint/suspicious/noExplicitAny: top-level cache_control is not in the typed params yet
      cache_control: { type: "ephemeral" },
    } as any);
```

At the top of the file, below `import type { ToolDefinition }`, add:
```typescript
const CONTINUATION_PROMPT =
  "[system: your previous response was cut off due to length. please continue exactly where you left off.]";
```

- [ ] **Step 4: Run test, verify PASS**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/mind/loop.ts server-ts/src/mind/loop.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): handle max_tokens continuation and prompt caching

max_tokens: inject continuation user message and re-prompt.
Top-level cache_control: ephemeral on every request so stable system +
early-conversation prefix caches at ~5-min TTL.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 12: Mind loop — server-side compaction

**Files:**
- Modify: `server-ts/src/mind/loop.ts`
- Modify: `server-ts/src/mind/loop.test.ts`

- [ ] **Step 1: Extend the test**

Append to `server-ts/src/mind/loop.test.ts`:

```typescript
describe("runMindWithTools — compaction", () => {
  it("passes context_management and the compact beta header", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "." }],
        usage: { input_tokens: 10, output_tokens: 1 },
      },
    ]);

    await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "hi",
      executeTool: async () => "unused",
      maxIterations: 5,
    });

    const call = client.calls[0] as unknown as {
      context_management?: { edits: Array<{ type: string }> };
      betas?: string[];
    };
    expect(call.context_management?.edits?.[0]?.type).toBe("compact_20260112");
    expect(call.betas).toContain("compact-2026-01-12");
  });

  it("preserves compaction blocks across turns", async () => {
    // Turn 1: max_tokens (forces a continuation turn) with a compaction block
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "max_tokens",
        content: [
          { type: "compaction", content: "earlier conversation summary" },
          { type: "text", text: "new text after compaction" },
        ],
        usage: { input_tokens: 160_000, output_tokens: 4096 },
      },
      {
        id: "msg_2",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "done" }],
        usage: { input_tokens: 50_000, output_tokens: 100 },
      },
    ]);

    await runMindWithTools({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "long request",
      executeTool: async () => "unused",
      maxIterations: 5,
    });

    // The second call's message history must include the compaction block on
    // the assistant turn from message 1 (not just the text).
    const secondCall = client.calls[1];
    const assistantTurn = secondCall?.messages.find((m) => m.role === "assistant");
    const blocks = assistantTurn?.content as Array<{ type: string }>;
    expect(blocks.find((b) => b.type === "compaction")).toBeDefined();
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/mind/loop.test.ts
```

- [ ] **Step 3: Extend the implementation**

In `server-ts/src/mind/loop.ts`, update the `client.messages.create(...)` call to include `context_management` and `betas`:

```typescript
    const response = await client.messages.create({
      model,
      max_tokens: 4096,
      system: systemPrompt,
      tools: tools as unknown as Anthropic.Messages.ToolUnion[],
      messages,
      cache_control: { type: "ephemeral" },
      context_management: {
        edits: [
          {
            type: "compact_20260112",
            trigger: { type: "input_tokens", value: 150_000 },
          },
        ],
      },
      betas: ["compact-2026-01-12"],
    } as any);
```

The existing logic already appends `response.content as unknown as Anthropic.Messages.ContentBlockParam[]` — that preserves compaction blocks since they stream through untouched.

- [ ] **Step 4: Run test, verify PASS**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/mind/loop.ts server-ts/src/mind/loop.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): enable server-side compaction

Passes context_management with compact_20260112 edit + compact-2026-01-12
beta header on every request. Compaction blocks in the response are
preserved when we append response.content to the message history,
so the API can drop pre-compaction messages on the next request.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 13: Streaming mock + streaming mind loop

**Files:**
- Modify: `server-ts/src/mind/mock-client.ts`
- Modify: `server-ts/src/mind/mock-client.test.ts`
- Modify: `server-ts/src/mind/loop.ts`
- Modify: `server-ts/src/mind/loop.test.ts`

- [ ] **Step 1: Extend the mock client's test**

Append to `server-ts/src/mind/mock-client.test.ts`:

```typescript
describe("MockAnthropicClient.messages.stream", () => {
  it("yields queued delta strings and returns a final message", async () => {
    const client = new MockAnthropicClient([], {
      streams: [
        { deltas: ["Hel", "lo!"], final: TEXT_MESSAGE },
      ],
    });

    const stream = client.messages.stream({
      model: "mock",
      max_tokens: 100,
      messages: [{ role: "user", content: "hi" }],
    });

    const collected: string[] = [];
    for await (const event of stream) {
      if (event.type === "content_block_delta" && event.delta.type === "text_delta") {
        collected.push(event.delta.text);
      }
    }
    expect(collected).toEqual(["Hel", "lo!"]);
    const final = await stream.finalMessage();
    expect(final.content[0]).toEqual({ type: "text", text: "hello" });
  });
});
```

- [ ] **Step 2: Extend the mock client**

Replace `stream: (): never => { throw ... }` in `server-ts/src/mind/mock-client.ts` with a real implementation:

```typescript
export type MockStream = {
  deltas: string[];
  final: MockMessage;
};

export class MockAnthropicClient implements MindClient {
  readonly calls: CreateParams[] = [];
  readonly streamCalls: CreateParams[] = [];
  private readonly queue: MockMessage[];
  private readonly streamQueue: MockStream[];

  constructor(
    responses: MockMessage[],
    opts: { streams?: MockStream[] } = {},
  ) {
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
          yield { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: delta } };
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
```

Run mock tests, verify PASS.

- [ ] **Step 3: Extend the mind loop's test**

Append to `server-ts/src/mind/loop.test.ts`:

```typescript
import { runMindStreaming } from "./loop.js";

describe("runMindStreaming", () => {
  it("emits delta callbacks as text streams in", async () => {
    const client = new MockAnthropicClient(
      [],
      {
        streams: [
          {
            deltas: ["Hi ", "there"],
            final: {
              id: "msg_1",
              role: "assistant",
              model: "mock",
              stop_reason: "end_turn",
              content: [{ type: "text", text: "Hi there" }],
              usage: { input_tokens: 10, output_tokens: 3 },
            },
          },
        ],
      },
    );

    const chunks: string[] = [];
    const result = await runMindStreaming({
      client,
      model: "mock",
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "hi",
      onTextDelta: (text) => chunks.push(text),
    });

    expect(chunks).toEqual(["Hi ", "there"]);
    expect(result.finalText).toBe("Hi there");
  });
});
```

- [ ] **Step 4: Extend the mind loop**

Append to `server-ts/src/mind/loop.ts`:

```typescript
export type MindStreamingInputs = MindTurnInputs & {
  onTextDelta: (text: string) => void;
};

export type MindStreamingResult = {
  finalText: string;
  assistantContent: ContentBlock[];
  totalUsage: MindTurnResult["totalUsage"];
};

export async function runMindStreaming(
  inputs: MindStreamingInputs,
): Promise<MindStreamingResult> {
  const { client, model, systemPrompt, tools, onTextDelta } = inputs;

  const messages: Anthropic.Messages.MessageParam[] = [
    ...conversationToMessages(inputs.conversation),
    { role: "user", content: inputs.userMessage },
  ];

  const stream = client.messages.stream({
    model,
    max_tokens: 4096,
    system: systemPrompt,
    tools: tools as unknown as Anthropic.Messages.ToolUnion[],
    messages,
    cache_control: { type: "ephemeral" },
    context_management: {
      edits: [{ type: "compact_20260112", trigger: { type: "input_tokens", value: 150_000 } }],
    },
    betas: ["compact-2026-01-12"],
  } as any);

  for await (const event of stream) {
    if (event.type === "content_block_delta" && event.delta.type === "text_delta") {
      onTextDelta(event.delta.text);
    }
  }

  const final = await stream.finalMessage();
  return {
    finalText: final.content
      .filter((b: { type: string }) => b.type === "text")
      .map((b: unknown) => (b as { text: string }).text)
      .join(""),
    assistantContent: final.content as unknown as ContentBlock[],
    totalUsage: {
      input_tokens: final.usage.input_tokens,
      output_tokens: final.usage.output_tokens,
      cache_read_input_tokens: final.usage.cache_read_input_tokens ?? 0,
      cache_creation_input_tokens: final.usage.cache_creation_input_tokens ?? 0,
    },
  };
}
```

- [ ] **Step 5: Run tests, verify PASS**

- [ ] **Step 6: `pnpm check`**

- [ ] **Step 7: Commit**

```bash
git add server-ts/src/mind/loop.ts server-ts/src/mind/loop.test.ts server-ts/src/mind/mock-client.ts server-ts/src/mind/mock-client.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add streaming mind loop + streaming mock

runMindStreaming uses messages.stream + finalMessage. Emits text deltas
via a callback so the HTTP layer can broadcast ChatStreamDelta events in
real time. Mock client now supports stream() for unit-testable streaming.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 14: Budget integration

**Files:**
- Create: `server-ts/src/mind/budgeted-mind.ts`
- Create: `server-ts/src/mind/budgeted-mind.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { recordSpend } from "../budget/ledger.js";
import { runBudgetedMind } from "./budgeted-mind.js";
import { MockAnthropicClient } from "./mock-client.js";

describe("runBudgetedMind", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-bm-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("runs the mind and records spend on the ledger", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "." }],
        usage: { input_tokens: 1000, output_tokens: 200 },
      },
    ]);

    const result = await runBudgetedMind({
      client,
      home,
      slug: "alice",
      day: "2026-04-22",
      capUsd: 2.0,
      model: "mock",
      pricePerMTokIn: 3.0,
      pricePerMTokOut: 15.0,
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "hi",
      executeTool: async () => "unused",
      maxIterations: 5,
    });

    expect(result.downgraded).toBeUndefined();
    const { loadDaily } = await import("../budget/ledger.js");
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    // 1000 * 3/1M + 200 * 15/1M = 0.003 + 0.003 = 0.006
    expect(ledger.dollars_spent).toBeCloseTo(0.006, 4);
    expect(ledger.calls).toBe(1);
  });

  it("downgrades when budget is suppressed without invoking the client", async () => {
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 0,
      tokens_out: 0,
      dollars: 2.5,
    });

    const client = new MockAnthropicClient([
      {
        id: "should-not-run",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "." }],
        usage: { input_tokens: 10, output_tokens: 1 },
      },
    ]);

    const result = await runBudgetedMind({
      client,
      home,
      slug: "alice",
      day: "2026-04-22",
      capUsd: 2.0,
      model: "mock",
      pricePerMTokIn: 3.0,
      pricePerMTokOut: 15.0,
      systemPrompt: "SYSTEM",
      tools: [],
      conversation: [],
      userMessage: "hi",
      executeTool: async () => "unused",
      maxIterations: 5,
    });

    expect(result.downgraded).toBe(true);
    expect(client.calls).toHaveLength(0);
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/mind/budgeted-mind.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
import { chargeAndCall } from "../budget/charge-and-call.js";
import type { ConversationEntry } from "../conversation/types.js";
import type { MindClient } from "./anthropic-client.js";
import { runMindWithTools, type MindFullTurnResult } from "./loop.js";
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

export type BudgetedMindResult =
  | ({ downgraded: false } & MindFullTurnResult)
  | { downgraded: true; reason: string };

export async function runBudgetedMind(
  inputs: BudgetedMindInputs,
): Promise<BudgetedMindResult> {
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
  return { downgraded: false, ...outcome.value };
}
```

- [ ] **Step 4: Run test, verify PASS (2 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/mind/budgeted-mind.ts server-ts/src/mind/budgeted-mind.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): wrap mind loop in chargeAndCall

runBudgetedMind gates every tier-3 mind invocation through the daily
budget ledger. On suppressed state, returns a downgraded outcome without
calling Anthropic. Token-to-dollar conversion uses per-million-token
prices passed in by the caller (model-dependent).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 15: ServerEvent types + JSON wire format

**Files:**
- Create: `server-ts/src/events/server-event.ts`
- Create: `server-ts/src/events/server-event.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { describe, expect, it } from "vitest";
import { serializeServerEvent, type ServerEvent } from "./server-event.js";

describe("serializeServerEvent", () => {
  it("ChatMessageCreated matches the Rust wire format shape", () => {
    const event: ServerEvent = {
      type: "chat_message_created",
      instance_slug: "alice",
      chat_id: "default",
      message: {
        id: "msg_1",
        role: "Assistant",
        content: "hello",
        created_at: "1714000000000",
        kind: "Message",
      },
    };
    const json = JSON.parse(serializeServerEvent(event));
    expect(json.type).toBe("chat_message_created");
    expect(json.instance_slug).toBe("alice");
    expect(json.message.role).toBe("Assistant");
  });

  it("ChatStreamDelta carries message_id + delta", () => {
    const event: ServerEvent = {
      type: "chat_stream_delta",
      instance_slug: "alice",
      chat_id: "default",
      message_id: "msg_2",
      delta: "Hi ",
    };
    const json = JSON.parse(serializeServerEvent(event));
    expect(json.type).toBe("chat_stream_delta");
    expect(json.delta).toBe("Hi ");
  });

  it("AgentRunning / AgentStopped are bare signals", () => {
    const running: ServerEvent = {
      type: "agent_running",
      instance_slug: "alice",
      chat_id: "default",
    };
    const json = JSON.parse(serializeServerEvent(running));
    expect(json).toEqual({
      type: "agent_running",
      instance_slug: "alice",
      chat_id: "default",
    });
  });

  it("ContextCompacting reports how many messages were compacted", () => {
    const event: ServerEvent = {
      type: "context_compacting",
      instance_slug: "alice",
      chat_id: "default",
      messages_compacted: 42,
    };
    const json = JSON.parse(serializeServerEvent(event));
    expect(json.messages_compacted).toBe(42);
  });

  it("ChatSnapshot replaces the whole chat state", () => {
    const event: ServerEvent = {
      type: "chat_snapshot",
      instance_slug: "alice",
      chat_id: "default",
      messages: [],
      agent_running: false,
    };
    const json = JSON.parse(serializeServerEvent(event));
    expect(json.agent_running).toBe(false);
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/events/server-event.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
/**
 * ServerEvent — the WebSocket wire format. Byte-compatible with the Rust
 * backend's existing serde(rename_all = "snake_case") tagged enum, so the
 * SvelteKit client runs unmodified.
 *
 * This file intentionally defines only the variants Plan 2 emits. Plans 3
 * (outreach), 4 (cross-instance), and 5 (polish) add the remaining variants
 * (MoodUpdated, DropCreated, HeartbeatThought, McpAppStart, etc.) as their
 * features land.
 */
export type ChatMessage = {
  id: string;
  role: "User" | "Assistant";
  content: string;
  created_at: string;
  kind: "Message" | "ToolCall" | "ToolOutput" | "McpApp" | "Compaction";
  tool_name?: string;
  model?: string;
};

export type ServerEvent =
  | {
      type: "chat_message_created";
      instance_slug: string;
      chat_id: string;
      message: ChatMessage;
    }
  | {
      type: "chat_stream_delta";
      instance_slug: string;
      chat_id: string;
      message_id: string;
      delta: string;
    }
  | {
      type: "agent_running";
      instance_slug: string;
      chat_id: string;
    }
  | {
      type: "agent_stopped";
      instance_slug: string;
      chat_id: string;
    }
  | {
      type: "context_compacting";
      instance_slug: string;
      chat_id: string;
      messages_compacted: number;
    }
  | {
      type: "chat_snapshot";
      instance_slug: string;
      chat_id: string;
      messages: ChatMessage[];
      agent_running: boolean;
    };

export function serializeServerEvent(event: ServerEvent): string {
  return JSON.stringify(event);
}
```

- [ ] **Step 4: Run test, verify PASS (5 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/events/server-event.ts server-ts/src/events/server-event.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add ServerEvent wire format

Plan 2 subset of the Rust ServerEvent enum: chat_message_created,
chat_stream_delta, agent_running/stopped, context_compacting,
chat_snapshot. Byte-compatible with the existing client's JSON shape.
Later plans extend this union.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 16: WebSocket broadcaster

**Files:**
- Create: `server-ts/src/events/broadcaster.ts`
- Create: `server-ts/src/events/broadcaster.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { describe, expect, it, vi } from "vitest";
import { Broadcaster } from "./broadcaster.js";
import type { ServerEvent } from "./server-event.js";

describe("Broadcaster", () => {
  it("delivers an event to every subscriber", () => {
    const b = new Broadcaster();
    const a = vi.fn();
    const c = vi.fn();
    b.subscribe(a);
    b.subscribe(c);

    const event: ServerEvent = {
      type: "agent_running",
      instance_slug: "alice",
      chat_id: "default",
    };
    b.emit(event);

    expect(a).toHaveBeenCalledWith(event);
    expect(c).toHaveBeenCalledWith(event);
  });

  it("unsubscribe stops delivery", () => {
    const b = new Broadcaster();
    const a = vi.fn();
    const unsub = b.subscribe(a);
    unsub();
    b.emit({ type: "agent_running", instance_slug: "x", chat_id: "y" });
    expect(a).not.toHaveBeenCalled();
  });

  it("isolates subscriber exceptions from other subscribers", () => {
    const b = new Broadcaster();
    b.subscribe(() => {
      throw new Error("boom");
    });
    const ok = vi.fn();
    b.subscribe(ok);
    b.emit({ type: "agent_running", instance_slug: "x", chat_id: "y" });
    expect(ok).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/events/broadcaster.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
import type { ServerEvent } from "./server-event.js";

export type Subscriber = (event: ServerEvent) => void;

/**
 * In-process pub-sub for ServerEvents. WebSocket handlers call subscribe()
 * on connect; the mind worker calls emit() as events fire.
 * Subscriber exceptions are caught so one bad client can't break others.
 */
export class Broadcaster {
  private readonly subs = new Set<Subscriber>();

  subscribe(fn: Subscriber): () => void {
    this.subs.add(fn);
    return () => this.subs.delete(fn);
  }

  emit(event: ServerEvent): void {
    for (const fn of this.subs) {
      try {
        fn(event);
      } catch (err) {
        console.error("Broadcaster: subscriber threw", err);
      }
    }
  }
}
```

- [ ] **Step 4: Run test, verify PASS (3 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/events/broadcaster.ts server-ts/src/events/broadcaster.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add in-process ServerEvent broadcaster

Simple pub-sub. WebSocket handlers subscribe on connect; the mind worker
emits as events fire. Subscriber errors are caught and logged, not
propagated, so one bad client never breaks others.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 17: Mind worker

**Files:**
- Create: `server-ts/src/mind/worker.ts`
- Create: `server-ts/src/mind/worker.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { Broadcaster } from "../events/broadcaster.js";
import type { ServerEvent } from "../events/server-event.js";
import { MindWorker } from "./worker.js";
import { MockAnthropicClient } from "./mock-client.js";

describe("MindWorker", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-worker-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("handles a chat message end-to-end and persists both turns", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "hi back" }],
        usage: { input_tokens: 100, output_tokens: 10 },
      },
    ]);

    const broadcaster = new Broadcaster();
    const received: ServerEvent[] = [];
    broadcaster.subscribe((e) => received.push(e));

    const worker = new MindWorker({
      client,
      home,
      slug: "alice",
      chatId: "default",
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3.0,
      pricePerMTokOut: 15.0,
      broadcaster,
    });

    await worker.handleUserMessage("hello");

    // Two broadcasts: AgentRunning (on start) and AgentStopped (on end),
    // plus at least one ChatMessageCreated for the assistant reply.
    const types = received.map((e) => e.type);
    expect(types).toContain("agent_running");
    expect(types).toContain("chat_message_created");
    expect(types).toContain("agent_stopped");

    // Conversation persisted: user turn + assistant turn.
    const { loadConversation } = await import("../conversation/store.js");
    const conv = await loadConversation(home, "alice", "default");
    expect(conv.map((e) => e.role)).toEqual(["user", "assistant"]);
  });

  it("tracks warm state: active after use, teardown-eligible after TTL", async () => {
    const client = new MockAnthropicClient([]);
    const worker = new MindWorker({
      client,
      home,
      slug: "alice",
      chatId: "default",
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3.0,
      pricePerMTokOut: 15.0,
      broadcaster: new Broadcaster(),
      warmTtlMs: 1000,
    });

    worker.touch(1_000_000);
    expect(worker.isStaleAt(1_000_500)).toBe(false);
    expect(worker.isStaleAt(1_002_001)).toBe(true);
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/mind/worker.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
import { loadSkills } from "../skills/loader.js";
import { loadTriageRules } from "../triage/rules.js";
import { loadSettings } from "../settings/reader.js";
import { todayUtc } from "../budget/ledger.js";
import type { Broadcaster } from "../events/broadcaster.js";
import {
  type ChatMessage,
  type ServerEvent,
} from "../events/server-event.js";
import {
  appendConversationEntry,
  loadConversation,
} from "../conversation/store.js";
import type { ConversationEntry } from "../conversation/types.js";
import { instanceDir } from "../paths.js";
import type { MindClient } from "./anthropic-client.js";
import { runBudgetedMind } from "./budgeted-mind.js";
import { buildSystemPrompt } from "./system-prompt.js";
import { builtInTools, skillToTool } from "./skill-tool.js";
import { readFile } from "node:fs/promises";
import { join } from "node:path";

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

      if (outcome.downgraded) {
        broadcaster.emit({ type: "agent_stopped", instance_slug: slug, chat_id: chatId });
        return;
      }

      const assistantEntry: ConversationEntry = {
        id: `msg_${Date.now()}_a`,
        role: "assistant",
        content: outcome.assistantContent,
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
          content: outcome.finalText,
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
```

- [ ] **Step 4: Run test, verify PASS (2 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/mind/worker.ts server-ts/src/mind/worker.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add MindWorker — per-employee chat handler

Assembles system prompt from soul/mood/rhythm/skills/triage, loads
conversation, runs runBudgetedMind, persists both turns, broadcasts
AgentRunning / ChatMessageCreated / AgentStopped. Tracks warm TTL so
the worker pool (Task 18) can tear down idle workers.

Tool execution is stubbed — real outreach + cross-instance wiring lands
in Plans 4 and 5.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 18: Worker pool

**Files:**
- Create: `server-ts/src/mind/pool.ts`
- Create: `server-ts/src/mind/pool.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Broadcaster } from "../events/broadcaster.js";
import { MockAnthropicClient } from "./mock-client.js";
import { WorkerPool } from "./pool.js";

describe("WorkerPool", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-pool-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("creates one worker per (slug, chatId) on demand", () => {
    const pool = new WorkerPool({
      clientFactory: () => new MockAnthropicClient([]),
      home,
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3,
      pricePerMTokOut: 15,
      broadcaster: new Broadcaster(),
    });

    const a = pool.get("alice", "default");
    const a2 = pool.get("alice", "default");
    const b = pool.get("bob", "default");

    expect(a).toBe(a2);
    expect(a).not.toBe(b);
  });

  it("sweepStale removes workers past their TTL", () => {
    const pool = new WorkerPool({
      clientFactory: () => new MockAnthropicClient([]),
      home,
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3,
      pricePerMTokOut: 15,
      broadcaster: new Broadcaster(),
      warmTtlMs: 1000,
    });

    const w = pool.get("alice", "default");
    w.touch(1_000_000);
    pool.sweepStale(1_000_500);
    expect(pool.get("alice", "default")).toBe(w);

    pool.sweepStale(1_100_000);
    expect(pool.get("alice", "default")).not.toBe(w);
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/mind/pool.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
import type { Broadcaster } from "../events/broadcaster.js";
import type { MindClient } from "./anthropic-client.js";
import { MindWorker } from "./worker.js";

export type WorkerPoolOptions = {
  clientFactory: () => MindClient;
  home: string;
  companyName: string;
  model: string;
  pricePerMTokIn: number;
  pricePerMTokOut: number;
  broadcaster: Broadcaster;
  warmTtlMs?: number;
};

export class WorkerPool {
  private readonly workers = new Map<string, MindWorker>();

  constructor(private readonly opts: WorkerPoolOptions) {}

  private key(slug: string, chatId: string): string {
    return `${slug}/${chatId}`;
  }

  get(slug: string, chatId: string): MindWorker {
    const k = this.key(slug, chatId);
    const existing = this.workers.get(k);
    if (existing) return existing;

    const worker = new MindWorker({
      client: this.opts.clientFactory(),
      home: this.opts.home,
      slug,
      chatId,
      companyName: this.opts.companyName,
      model: this.opts.model,
      pricePerMTokIn: this.opts.pricePerMTokIn,
      pricePerMTokOut: this.opts.pricePerMTokOut,
      broadcaster: this.opts.broadcaster,
      ...(this.opts.warmTtlMs !== undefined ? { warmTtlMs: this.opts.warmTtlMs } : {}),
    });
    this.workers.set(k, worker);
    return worker;
  }

  sweepStale(nowMs: number): void {
    for (const [k, w] of this.workers) {
      if (w.isStaleAt(nowMs)) this.workers.delete(k);
    }
  }

  size(): number {
    return this.workers.size;
  }
}
```

- [ ] **Step 4: Run test, verify PASS (2 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/mind/pool.ts server-ts/src/mind/pool.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add WorkerPool

Lazy-start MindWorker per (slug, chat_id), reused for subsequent messages.
sweepStale() is called periodically by the main loop (Task 21) to tear
down idle workers past their warm TTL.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 19: HTTP auth middleware

**Files:**
- Create: `server-ts/src/http/auth.ts`
- Create: `server-ts/src/http/auth.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { Hono } from "hono";
import { describe, expect, it } from "vitest";
import { requireAuth } from "./auth.js";

function app(authToken: string | undefined) {
  const h = new Hono();
  h.use("*", requireAuth(authToken));
  h.get("/ok", (c) => c.text("ok"));
  return h;
}

describe("requireAuth", () => {
  it("passes through when authToken is undefined (no auth configured)", async () => {
    const res = await app(undefined).request("/ok");
    expect(res.status).toBe(200);
  });

  it("accepts a matching Bearer token", async () => {
    const res = await app("secret").request("/ok", {
      headers: { Authorization: "Bearer secret" },
    });
    expect(res.status).toBe(200);
  });

  it("accepts a matching ?token= query param (for WebSocket)", async () => {
    const res = await app("secret").request("/ok?token=secret");
    expect(res.status).toBe(200);
  });

  it("returns 401 with no token", async () => {
    const res = await app("secret").request("/ok");
    expect(res.status).toBe(401);
  });

  it("returns 401 with a wrong token", async () => {
    const res = await app("secret").request("/ok", {
      headers: { Authorization: "Bearer wrong" },
    });
    expect(res.status).toBe(401);
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/http/auth.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
import type { MiddlewareHandler } from "hono";

/**
 * Bearer-token or ?token= query-param auth.
 * - If authToken is undefined, auth is disabled (dev).
 * - Otherwise, requests must supply the token via either channel.
 */
export function requireAuth(authToken: string | undefined): MiddlewareHandler {
  return async (c, next) => {
    if (!authToken) return next();

    const header = c.req.header("authorization") ?? "";
    const bearer = header.startsWith("Bearer ") ? header.slice(7) : "";
    const query = c.req.query("token") ?? "";

    if (bearer === authToken || query === authToken) {
      return next();
    }
    return c.text("Unauthorized", 401);
  };
}
```

- [ ] **Step 4: Run test, verify PASS (5 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/http/auth.ts server-ts/src/http/auth.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add bearer-token auth middleware

Mirrors the Rust backend: Authorization: Bearer <token> or ?token=<...>
query param. When authToken is undefined, auth is disabled (dev mode).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 20: Hono server — chat + WebSocket

**Files:**
- Create: `server-ts/src/http/server.ts`
- Create: `server-ts/src/http/server.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { Broadcaster } from "../events/broadcaster.js";
import { MockAnthropicClient } from "../mind/mock-client.js";
import { WorkerPool } from "../mind/pool.js";
import { createApp } from "./server.js";

describe("createApp", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-http-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("POST /api/chat returns 202 and triggers the mind worker", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_1",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "ack" }],
        usage: { input_tokens: 10, output_tokens: 1 },
      },
    ]);
    const broadcaster = new Broadcaster();
    const pool = new WorkerPool({
      clientFactory: () => client,
      home,
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3,
      pricePerMTokOut: 15,
      broadcaster,
    });
    const app = createApp({ authToken: undefined, pool, broadcaster });

    const res = await app.request("/api/chat", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ instance_slug: "alice", chat_id: "default", content: "hi" }),
    });

    expect(res.status).toBe(202);
  });

  it("POST /api/chat requires auth when configured", async () => {
    const pool = new WorkerPool({
      clientFactory: () => new MockAnthropicClient([]),
      home,
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3,
      pricePerMTokOut: 15,
      broadcaster: new Broadcaster(),
    });
    const app = createApp({ authToken: "secret", pool, broadcaster: new Broadcaster() });

    const res = await app.request("/api/chat", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ instance_slug: "x", chat_id: "y", content: "hi" }),
    });
    expect(res.status).toBe(401);
  });

  it("GET /api/health returns 200", async () => {
    const pool = new WorkerPool({
      clientFactory: () => new MockAnthropicClient([]),
      home,
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3,
      pricePerMTokOut: 15,
      broadcaster: new Broadcaster(),
    });
    const app = createApp({ authToken: undefined, pool, broadcaster: new Broadcaster() });

    const res = await app.request("/api/health");
    expect(res.status).toBe(200);
  });
});
```

- [ ] **Step 2: Run test, verify FAIL**

```bash
pnpm test src/http/server.test.ts
```

- [ ] **Step 3: Write implementation**

```typescript
import { Hono } from "hono";
import type { Broadcaster } from "../events/broadcaster.js";
import type { WorkerPool } from "../mind/pool.js";
import { requireAuth } from "./auth.js";

export type AppOptions = {
  authToken: string | undefined;
  pool: WorkerPool;
  broadcaster: Broadcaster;
};

export function createApp(opts: AppOptions) {
  const app = new Hono();

  app.get("/api/health", (c) => c.json({ ok: true }));

  app.use("/api/*", requireAuth(opts.authToken));

  app.post("/api/chat", async (c) => {
    const body = await c.req.json<{
      instance_slug: string;
      chat_id: string;
      content: string;
    }>();

    const worker = opts.pool.get(body.instance_slug, body.chat_id);
    // Fire-and-forget: the mind turn runs asynchronously and broadcasts
    // progress over WebSocket. The HTTP caller just gets a 202.
    void worker.handleUserMessage(body.content).catch((err) => {
      console.error("mind worker error", err);
    });
    return c.body(null, 202);
  });

  return app;
}
```

- [ ] **Step 4: Run test, verify PASS (3 tests)**

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/http/server.ts server-ts/src/http/server.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add Hono app with /api/chat and /api/health

POST /api/chat is fire-and-forget: returns 202, the MindWorker runs
asynchronously and broadcasts ChatStreamDelta + ChatMessageCreated over
the WebSocket channel. GET /api/health is an unauthed liveness probe.

Auth middleware applies to all /api/* except /api/health.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 21: WebSocket endpoint

**Files:**
- Modify: `server-ts/src/http/server.ts`
- Create: `server-ts/src/http/ws.ts`

- [ ] **Step 1: Write the implementation**

Create `server-ts/src/http/ws.ts`:

```typescript
import type { Broadcaster } from "../events/broadcaster.js";
import { serializeServerEvent } from "../events/server-event.js";

/**
 * Wire a Hono WebSocket to the Broadcaster. Returns a handler usable with
 * @hono/node-ws's upgradeWebSocket.
 *
 * Auth is handled by the upgrade path (middleware + ?token= query param).
 */
export function wsHandler(broadcaster: Broadcaster) {
  return () => ({
    onOpen(_evt: unknown, ws: { send: (data: string) => void; close: () => void }) {
      const unsub = broadcaster.subscribe((event) => {
        try {
          ws.send(serializeServerEvent(event));
        } catch {
          unsub();
          ws.close();
        }
      });
      (ws as unknown as { _unsub?: () => void })._unsub = unsub;
    },
    onClose(_evt: unknown, ws: { _unsub?: () => void }) {
      ws._unsub?.();
    },
    onError(_evt: unknown, ws: { _unsub?: () => void }) {
      ws._unsub?.();
    },
    onMessage: () => {
      // Clients don't need to send messages over WS — chat goes through POST /api/chat
    },
  });
}
```

- [ ] **Step 2: Wire into the Hono app**

Modify `server-ts/src/http/server.ts` to register the WebSocket route. Replace the current `createApp` with:

```typescript
import { Hono } from "hono";
import { createNodeWebSocket } from "@hono/node-ws";
import type { Broadcaster } from "../events/broadcaster.js";
import type { WorkerPool } from "../mind/pool.js";
import { requireAuth } from "./auth.js";
import { wsHandler } from "./ws.js";

export type AppOptions = {
  authToken: string | undefined;
  pool: WorkerPool;
  broadcaster: Broadcaster;
};

export function createApp(opts: AppOptions) {
  const app = new Hono();
  const { injectWebSocket, upgradeWebSocket } = createNodeWebSocket({ app });

  app.get("/api/health", (c) => c.json({ ok: true }));

  app.use("/api/*", requireAuth(opts.authToken));

  app.post("/api/chat", async (c) => {
    const body = await c.req.json<{
      instance_slug: string;
      chat_id: string;
      content: string;
    }>();

    const worker = opts.pool.get(body.instance_slug, body.chat_id);
    void worker.handleUserMessage(body.content).catch((err) => {
      console.error("mind worker error", err);
    });
    return c.body(null, 202);
  });

  app.get("/api/ws", upgradeWebSocket(wsHandler(opts.broadcaster)));

  return { app, injectWebSocket };
}
```

Update the existing tests in `server-ts/src/http/server.test.ts` to destructure: replace `const app = createApp(...)` with `const { app } = createApp(...)` everywhere.

- [ ] **Step 3: Run tests, verify PASS**

Run `pnpm test src/http/` — both `auth.test.ts` and `server.test.ts` should pass.

- [ ] **Step 4: `pnpm check`**

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/http/server.ts server-ts/src/http/server.test.ts server-ts/src/http/ws.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add WebSocket endpoint at /api/ws

Wires the in-process Broadcaster to a @hono/node-ws upgrade handler.
Every ServerEvent emitted by the worker pool gets serialized to JSON
and pushed to every connected client. Clients don't send messages over
WS — chat goes through POST /api/chat.

Auth is enforced via the standard auth middleware using ?token= on the
upgrade request.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 22: main.ts — boot + shutdown

**Files:**
- Create: `server-ts/src/main.ts`

- [ ] **Step 1: Write the file**

```typescript
import { serve } from "@hono/node-server";
import { Broadcaster } from "./events/broadcaster.js";
import { createAnthropicClient } from "./mind/anthropic-client.js";
import { WorkerPool } from "./mind/pool.js";
import { createApp } from "./http/server.js";
import { loadConfig } from "./config.js";

const DEFAULT_MODEL = "claude-sonnet-4-6";
const SONNET_PRICE_IN = 3.0;   // $/M input tokens
const SONNET_PRICE_OUT = 15.0; // $/M output tokens
const STALE_SWEEP_MS = 60_000; // once a minute

function getCompanyName(env: NodeJS.ProcessEnv): string {
  return env.BOLLY_COMPANY_NAME ?? "your company";
}

async function main() {
  const config = loadConfig(process.env as Record<string, string | undefined>);

  const broadcaster = new Broadcaster();
  const pool = new WorkerPool({
    clientFactory: () => createAnthropicClient(config.anthropicApiKey),
    home: config.bollyHome,
    companyName: getCompanyName(process.env),
    model: DEFAULT_MODEL,
    pricePerMTokIn: SONNET_PRICE_IN,
    pricePerMTokOut: SONNET_PRICE_OUT,
    broadcaster,
  });

  const { app, injectWebSocket } = createApp({
    authToken: config.authToken,
    pool,
    broadcaster,
  });

  const server = serve({
    fetch: app.fetch,
    port: config.httpPort,
  });
  injectWebSocket(server);

  const sweepInterval = setInterval(() => pool.sweepStale(Date.now()), STALE_SWEEP_MS);

  const shutdown = (signal: string) => {
    console.log(`received ${signal}, shutting down`);
    clearInterval(sweepInterval);
    server.close(() => {
      process.exit(0);
    });
    setTimeout(() => process.exit(1), 10_000).unref();
  };

  process.on("SIGTERM", () => shutdown("SIGTERM"));
  process.on("SIGINT", () => shutdown("SIGINT"));

  console.log(`Bolly v1.0 listening on port ${config.httpPort}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
```

- [ ] **Step 2: Add a start script**

Edit `server-ts/package.json` to add:

```json
    "start": "node dist/main.js",
    "dev": "tsc --watch & node --enable-source-maps --watch dist/main.js"
```

(Keep the existing `build`, `test`, `check` scripts.)

- [ ] **Step 3: Verify it builds**

```bash
pnpm build
```

Expected: `dist/main.js` is produced.

**Note:** actually starting the server requires real `ANTHROPIC_API_KEY` + `BOLLY_HOME`. This task doesn't run it — only verifies compilation. The E2E test in Task 24 starts the server with a mock client.

- [ ] **Step 4: `pnpm check`**

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/main.ts server-ts/package.json
git commit -m "$(cat <<'EOF'
feat(server-ts): add main entry point

Wires Broadcaster → WorkerPool → Hono app → @hono/node-server.
Stale workers are swept every minute. SIGTERM/SIGINT flush and exit;
a 10-second timeout forces exit if anything hangs.

Adds pnpm start and pnpm dev scripts.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 23: Public API re-exports

**Files:**
- Modify: `server-ts/src/index.ts`
- Modify: `server-ts/src/index.test.ts`

- [ ] **Step 1: Extend the test**

Append to `server-ts/src/index.test.ts`:

```typescript
describe("Plan 2 public API", () => {
  it("re-exports runtime config, conversation store, mind, events, http", () => {
    expect(api.loadConfig).toBeTypeOf("function");
    expect(api.loadConversation).toBeTypeOf("function");
    expect(api.appendConversationEntry).toBeTypeOf("function");
    expect(api.buildSystemPrompt).toBeTypeOf("function");
    expect(api.skillToTool).toBeTypeOf("function");
    expect(api.builtInTools).toBeTypeOf("function");
    expect(api.createAnthropicClient).toBeTypeOf("function");
    expect(api.MockAnthropicClient).toBeTypeOf("function");
    expect(api.runMindTurn).toBeTypeOf("function");
    expect(api.runMindWithTools).toBeTypeOf("function");
    expect(api.runMindStreaming).toBeTypeOf("function");
    expect(api.runBudgetedMind).toBeTypeOf("function");
    expect(api.MindWorker).toBeTypeOf("function");
    expect(api.WorkerPool).toBeTypeOf("function");
    expect(api.Broadcaster).toBeTypeOf("function");
    expect(api.serializeServerEvent).toBeTypeOf("function");
    expect(api.createApp).toBeTypeOf("function");
    expect(api.requireAuth).toBeTypeOf("function");
  });
});
```

- [ ] **Step 2: Extend index.ts**

Append to `server-ts/src/index.ts`:

```typescript
// Plan 2 — Mind runtime
export { loadConfig, type RuntimeConfig } from "./config.js";

export {
  loadConversation,
  saveConversation,
  appendConversationEntry,
} from "./conversation/store.js";
export {
  ContentBlockSchema,
  ConversationEntrySchema,
  ConversationSchema,
  type ContentBlock,
  type ConversationEntry,
  type Conversation,
} from "./conversation/types.js";

export { buildSystemPrompt, type SystemPromptInputs } from "./mind/system-prompt.js";
export {
  skillToTool,
  skillToToolName,
  builtInTools,
  type ToolDefinition,
} from "./mind/skill-tool.js";
export { createAnthropicClient, type MindClient } from "./mind/anthropic-client.js";
export { MockAnthropicClient, type MockMessage, type MockStream } from "./mind/mock-client.js";
export {
  runMindTurn,
  runMindWithTools,
  runMindStreaming,
  type MindTurnInputs,
  type MindTurnResult,
  type MindFullTurnInputs,
  type MindFullTurnResult,
  type MindStreamingInputs,
  type MindStreamingResult,
} from "./mind/loop.js";
export { runBudgetedMind, type BudgetedMindResult } from "./mind/budgeted-mind.js";
export { MindWorker, type MindWorkerOptions } from "./mind/worker.js";
export { WorkerPool, type WorkerPoolOptions } from "./mind/pool.js";

export { Broadcaster, type Subscriber } from "./events/broadcaster.js";
export {
  serializeServerEvent,
  type ServerEvent,
  type ChatMessage,
} from "./events/server-event.js";

export { createApp, type AppOptions } from "./http/server.js";
export { requireAuth } from "./http/auth.js";
```

- [ ] **Step 3: Run index test, verify PASS**

```bash
pnpm test src/index.test.ts
```

- [ ] **Step 4: Run full suite**

```bash
pnpm test
```

Expected: all tests pass (should be 140+ tests across ~30 files).

- [ ] **Step 5: `pnpm check`**

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/index.ts server-ts/src/index.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): extend public API with Plan 2 surface

Re-exports the mind runtime (loop, worker, pool, client factory, mock),
conversation store, system prompt assembler, skill-tool mapping, events
(broadcaster, ServerEvent), HTTP app + auth middleware, and runtime config.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 24: End-to-end integration test

**Files:**
- Create: `server-ts/src/e2e.test.ts`

- [ ] **Step 1: Write the test**

```typescript
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { Broadcaster } from "./events/broadcaster.js";
import type { ServerEvent } from "./events/server-event.js";
import { MockAnthropicClient } from "./mind/mock-client.js";
import { WorkerPool } from "./mind/pool.js";
import { createApp } from "./http/server.js";

describe("E2E: HTTP chat → mind worker → WebSocket-style broadcast", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-e2e-"));
    // Seed a minimal instance with soul.md
    const instDir = join(home, "instances", "alice");
    await mkdir(instDir, { recursive: true });
    await writeFile(join(instDir, "soul.md"), "Alice's Bolly is helpful.");
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("POST /api/chat produces a full event sequence matching the Rust wire format", async () => {
    const client = new MockAnthropicClient([
      {
        id: "msg_assist",
        role: "assistant",
        model: "mock",
        stop_reason: "end_turn",
        content: [{ type: "text", text: "Hi Alice, what's up?" }],
        usage: { input_tokens: 200, output_tokens: 50 },
      },
    ]);

    const broadcaster = new Broadcaster();
    const received: ServerEvent[] = [];
    broadcaster.subscribe((e) => received.push(e));

    const pool = new WorkerPool({
      clientFactory: () => client,
      home,
      companyName: "Acme",
      model: "mock",
      pricePerMTokIn: 3,
      pricePerMTokOut: 15,
      broadcaster,
    });

    const { app } = createApp({ authToken: undefined, pool, broadcaster });

    const res = await app.request("/api/chat", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        instance_slug: "alice",
        chat_id: "default",
        content: "hey",
      }),
    });
    expect(res.status).toBe(202);

    // handleUserMessage is fire-and-forget; wait for the worker to finish
    // by polling for the AgentStopped event.
    for (let i = 0; i < 100 && !received.some((e) => e.type === "agent_stopped"); i++) {
      await new Promise((r) => setTimeout(r, 10));
    }

    const types = received.map((e) => e.type);
    expect(types).toContain("agent_running");
    expect(types.filter((t) => t === "chat_message_created")).toHaveLength(2); // user + assistant
    expect(types).toContain("agent_stopped");

    // Verify the wire format is valid JSON the client expects
    const assistantMsg = received.find(
      (e): e is Extract<ServerEvent, { type: "chat_message_created" }> =>
        e.type === "chat_message_created" && e.message.role === "Assistant",
    );
    expect(assistantMsg?.message.content).toBe("Hi Alice, what's up?");
    expect(assistantMsg?.message.kind).toBe("Message");
    expect(assistantMsg?.message.model).toBe("mock");
  });
});
```

- [ ] **Step 2: Run the E2E test**

```bash
pnpm test src/e2e.test.ts
```

Expected: PASS.

- [ ] **Step 3: Run the full suite**

```bash
pnpm test
```

Expected: all tests pass across the whole codebase (~150+ tests).

- [ ] **Step 4: `pnpm check`**

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/e2e.test.ts
git commit -m "$(cat <<'EOF'
test(server-ts): add E2E integration test for chat → mind → broadcast

Seeds a minimal instance (soul.md only), posts to /api/chat, waits for
AgentStopped, verifies the exact event sequence and that the ChatMessageCreated
wire format is compatible with the SvelteKit client.

Covers everything from Plan 2 end-to-end with the mock Anthropic client.
Does not run against real Claude — that's Plan 6's cost-validation test.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 25: Milestone tag + README note

**Files:**
- Create: `server-ts/README.md`

- [ ] **Step 1: Write a minimal README**

```markdown
# @bolly/server

The Bolly v1.0 server runtime. Self-hosted AI coworker for teams.

## Status

**Plan 1** ✅ Foundations (types, paths, budget ledger, skills parser, triage helpers, settings reader) — 82 tests.
**Plan 2** ✅ Mind runtime (Messages API loop with tool use, caching, compaction, streaming; HTTP server; WebSocket broadcast) — 150+ tests.
**Plans 3–6** pending: event queue + triage execution, outreach delivery, cross-instance, polish + cost validation.

## Development

```bash
pnpm install
pnpm check      # tsc --noEmit + biome check
pnpm test       # vitest run
pnpm test:watch # vitest watch
pnpm build      # → dist/
pnpm start      # node dist/main.js (requires ANTHROPIC_API_KEY + BOLLY_HOME)
```

## Configuration (env)

| Variable             | Required | Default | Purpose                                           |
|----------------------|----------|---------|---------------------------------------------------|
| `ANTHROPIC_API_KEY`  | yes      |         | Anthropic API key (company or BYOK per-employee) |
| `BOLLY_HOME`         | yes      |         | Path to workspace root (`instances/`, `shared/`) |
| `BOLLY_HTTP_PORT`    | no       | `4242`  | HTTP listen port                                  |
| `BOLLY_AUTH_TOKEN`   | no       |         | Bearer token for `/api/*` (disabled when unset)   |
| `BOLLY_COMPANY_NAME` | no       | `your company` | Rendered in the system prompt            |

## Architecture

See `docs/superpowers/specs/2026-04-22-bolly-v1-redesign-design.md`.
```

- [ ] **Step 2: Commit and tag**

```bash
git add server-ts/README.md
git commit -m "$(cat <<'EOF'
docs(server-ts): add README with run / test / env reference

Documents where Plan 1 and Plan 2 land in the roadmap and how to run the
server in development.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"

git tag -a mind-runtime-complete -m "Plan 2 complete: mind runtime, HTTP + WebSocket, budget-gated Anthropic loop"
```

Do not push the tag — the user reviews before pushing.

---

## Self-Review Checklist

Before declaring Plan 2 done, the executor should confirm:

- [ ] All 25 tasks committed (25 feat/test/chore commits on `bolly-v1-redesign`).
- [ ] `pnpm test` reports 150+ tests passing.
- [ ] `pnpm check` has no errors.
- [ ] `pnpm build` produces `dist/main.js`.
- [ ] `git log --oneline main..HEAD` shows a clean sequence with no TODO/WIP commits.
- [ ] `server-ts/src/index.ts` exports every module Plan 3 will consume.
- [ ] The `foundations-complete` tag still points at Plan 1's head; `mind-runtime-complete` is at Plan 2's.

## What Plan 2 does NOT do (Plan 3 picks it up)

- No event queue — user messages currently enter via `POST /api/chat` directly, not through a normalized event envelope. Plan 3 adds the queue + Tier 2 Haiku triage.
- No scheduled jobs — the scheduler service that enqueues `source: scheduled` events lands in Plan 3.
- No idle timer — the "user has been quiet for N minutes" source also lands in Plan 3.
- No real outreach — `send_push`, `send_email`, `defer_for_digest` are declared as tools, but Plan 2's `executeTool` returns a placeholder string. Plan 4 wires real delivery.
- No cross-instance — inter-Bolly events via `shared/channel/` land in Plan 5.
- No end-to-end cost validation — the 24h-simulation gate is Plan 6.

Plan 3 (Events & Triage, ~2 weeks) consumes this surface: `WorkerPool`, `Broadcaster`, `runBudgetedMind`, and the `Event` type from Plan 1's foundations. Expected ship: event queue with dedupe + backpressure, Haiku-backed triage gate, scheduled-job runner, cross-instance channel watcher (stub).
