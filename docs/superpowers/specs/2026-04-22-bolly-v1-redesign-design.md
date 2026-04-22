# Bolly v1.0 — Runtime & Multi-Agent Redesign

- **Date:** 2026-04-22
- **Status:** Draft for review
- **Authors:** Timur Turatbekov (w/ Claude)
- **Scope:** Complete rewrite of the Bolly server runtime; new agent model; cost-optimized event-driven heartbeat; cross-instance support

## TL;DR

Bolly repositions as a **self-hosted AI coworker for companies**. The Rust backend is replaced by a TypeScript service that drives the **Claude Messages API** directly with custom tools, server-side compaction, and prompt caching. The 10-minute polling heartbeat (costing ~$20/user/day) is replaced by an event-driven three-tier architecture: pure-code event sources → cheap Haiku triage → Messages-API mind loop on escalation only. `ChildAgentConfig` TOML files become NL-configured **skills** (markdown with YAML frontmatter). Each company deploys one `$BOLLY_HOME` with `instances/{employee}/` per user and a shared `shared/` directory for company-wide memory and inter-Bolly coordination. Target: $20–50/employee/month economics, under 4,000 LoC, 10-week timeline, shipped as v1.0 with no backward compatibility.

## Context

### Current state

- **Backend:** Rust/Axum single-binary server with embedded SvelteKit client (rust-embed), running an Agent loop over LLM providers (Anthropic + OpenAI).
- **Agent model:** `ChildAgentConfig` TOML files define "child agents" — each with its own hourly-cadence heartbeat loop (`server/src/services/heartbeat.rs`).
- **Cost:** measured at ~$20/user/day under the 10-minute default heartbeat. Unsustainable at any pricing model.
- **Single-provider friction:** OpenAI branch measured to be weak at computer use; the dual-provider surface doubles maintenance for little benefit.
- **Product positioning:** today sold/used as a personal AI companion; v1 repositions to a team-deployed AI coworker that lives inside a company's context.

### Why redesign

Four pains drove this work:

1. **The heartbeat is a cron, not a heartbeat.** It fires on a timer regardless of context, burning tokens whether anything is happening or not. Bolly needs to feel alive, not mechanical.
2. **Multi-agent orchestration is too rigid.** `ChildAgentConfig` is hand-edited TOML with hourly intervals. There is no runtime coordination between agents, and no way for a user to shape an agent through natural language.
3. **The Rust agent loop is doing work the Messages API does better.** Stop-reason handling, tool dispatch, and compaction are all first-class features of `/v1/messages` now (`context_management` for compaction, native tool use, prompt caching). Porting to a thin TypeScript loop on the Messages API cuts ~2,000 LoC of Rust and gets us ZDR, caching, and compaction for free. Competitors like [NanoClaw](https://github.com/qwibitai/nanoclaw) demonstrate that small, focused TS agent codebases can reach adoption quickly.
4. **The consumer positioning is too crowded.** Replika, Nomi, Kindroid own persona-first companions. Bolly's differentiator — autonomous heartbeat plus agentic capability — lands harder in a business context where persistent memory, team coordination, and proactive outreach translate directly to value.

### What's changing and what isn't

**Redesigned:**
- Server runtime (Rust → TypeScript, driving the Claude Messages API directly)
- Agent model (`ChildAgentConfig` → NL-configured skills)
- Heartbeat (polling → event-driven with triage gate)
- Cost model (uncapped → tiered with hard per-employee daily budget)
- Provider surface (Anthropic + OpenAI → Anthropic only, single API-key auth)
- Positioning (personal companion → self-hosted team coworker)

**Preserved (migrated unchanged):**
- SvelteKit client and onboarding flow (`client/src/lib/components/onboarding/InstanceOnboarding.svelte`)
- Vector memory with Gemini embeddings (`gemini-embedding-2-preview`)
- Soul / mood / rhythm persona layer
- Drops, memories, thoughts directories
- WebSocket event wire format (same `ServerEvent` variants; clients don't change)

**Out of scope (future specs):**
- Managed cloud / SaaS hosting (v1 is self-hosted only)
- Messaging-app outreach channels (Slack as the top priority, then Teams; iMessage/Telegram deprioritized for business context)
- Ambient features (audio, richer screen observation, location)
- Skills marketplace
- Persona-first onboarding redesign
- Team-level config layer on top of per-employee settings (v1.1)
- SSO / SAML / fine-grained data governance (v1.1, SOC2 path)

## Goals & success criteria

**Goals:**
1. Cost feasibility — sustainable per-employee economics at small-team scale
2. "Actual heartbeat" — Bolly feels present and reactive, not scheduled
3. User-friendly configuration — no TOML editing; everything through chat
4. **Team coworker model** — multiple Bollys in one company share docs and coordinate through a `shared/` directory
5. Self-hosted deployment — `docker compose up` and a company has Bolly running on their own infrastructure

**Success criteria — measurable:**

| Criterion | Target |
|---|---|
| Per-employee monthly cost (engaged employee) | $20–50/employee/month (vs ~$600 at today's burn rate) |
| Proactive outreach cadence (engaged employee) | ≥ 1/day without being spammy (rate-limited, dedup-aware) |
| TS backend size | < 4,000 LoC (NanoClaw-scale) |
| Deployment | Single `docker compose up` from a released image; admin flow under 10 minutes |
| WebSocket wire format | Client runs unmodified against new backend |
| Cross-instance | alice's Bolly can poke bob's Bolly in the same deployment in < 2s end-to-end |

## Design

### Architecture overview

Three tiers, budget-gated:

```
[events]  →  [queue]  →  [triage (Haiku)]  →  [mind (Messages API + custom tools)]  →  [outreach / act / note]
 user              dedupe       ignore                stateless loop in Node         push (web-push)
 email             backpressure digest                system: soul + skills          email (SMTP)
 calendar                       escalate              prompt-cached prefix           drop / memory
 idle                                                 server-side compaction
 scheduled                                            custom tools run in-process
 instance_emit
 skill_emit
                              ↑ budget ledger (daily cap) ↑
```

**Tier 0 — Event sources.** Every wake-up signal produces a normalized event envelope. No LLM calls.

**Tier 1 — Event queue.** In-process FIFO per user. Dedupes bursts (5 emails in 10s → 1 triage call). Provides backpressure when the mind is busy.

**Tier 2 — Triage gate (Haiku).** ~500 input / ~50 output tokens per call (~$0.001). Inputs: event, persona snippet, current mood, budget state, user's NL triage rules. Output: one of `ignore`, `digest`, or `escalate`.

**Tier 3 — The mind (Messages API loop).** Only invoked on escalation. We run a minimal agent loop in Node: system prompt (soul + enabled skills) is cached; user message carries the event context; custom tools fire `tool_use` blocks that we execute in-process and feed back as `tool_result`. Server-side compaction (`context_management`) handles long histories. Stable system + stable history prefix + prompt caching → low per-call cost on repeat escalations.

**Budget ledger.** Every tier-2 and tier-3 call records spend to `instances/{slug}/budget/{YYYY-MM-DD}.json`. The triage prompt receives current budget state; at 70% cap it tightens escalation threshold; at 100% all escalations downgrade to `digest` until midnight.

**Behavioral property:** the mind never polls. If events stop, no sessions are created. Employee returns → user-activity event → triage → session opens → mind catches up, closes. This is where the $20/user/day → ~$1.50/employee/day reduction lives.

### Skills model

`ChildAgentConfig` is removed entirely. A **skill** is a markdown file with YAML frontmatter at `instances/{slug}/skills/{name}.md`:

```markdown
---
name: email-morning-check
created: 2026-04-22
triggers:
  - scheduled: "every weekday at 8am"
  - event: "email arrives with 'urgent' in subject"
tools: [read_email, send_push, write_drop]
enabled: true
---

When triggered, read unread emails since last check.
Summarize anything the user would care about.
If something needs a response before noon, send a push notification —
otherwise save as a drop for when they open the app.
Don't mention newsletter emails — user finds that noisy.
```

The body *is* the skill's prompt. The mind is permitted to read, write, and edit its own skill files.

**Lifecycle:**
- **Create:** user chats "can you check my email each morning?" → mind writes a new skill file and patches `triage.md` with the trigger rule.
- **Edit:** mind updates the body as it learns preferences ("don't wake me before 9" appends to working notes).
- **Disable:** user says "stop checking email" → mind sets `enabled: false`. No hard-deletes; re-enabling restores learned context.

**Execution:**
- Every enabled skill becomes a **custom tool** in the `tools` array of each Messages API call. The tool name matches the skill name; the JSON-schema `input_schema` is derived from the skill file.
- On trigger escalation, the mind receives a `user` message naming the skill to run and the trigger context. The system prompt describes each available skill and when to invoke it.
- Skills that perform sensitive or boundary-crossing actions (e.g., cross-instance event emission, shared-memory writes) are gated by a **user-in-loop confirmation** our code enforces before executing the tool — even though Messages API has no permission policy primitive, we can intercept `tool_use` blocks and prompt the user before running them.
- The skill body (markdown) is surfaced to the mind through the cached system prompt for stable skills; recently edited skills get injected as inline guidance in the triggering `user` message to preserve cache hits on the system prefix.

**Baseline skills shipped by default:**
- `companion` — rhythm, mood, drops; always enabled; triggered by idle + meaningful events.
- `reflection` — periodic or on-request reflections; writes drops.

Everything else (email, calendar, specialized checks) is user-added via chat.

### Mind runtime

The mind runs directly on the **Anthropic Messages API** (`/v1/messages`). We implement a minimal TypeScript agent loop: send messages with custom tools, handle `tool_use` content blocks as they arrive, feed `tool_result` blocks back, repeat until `stop_reason: "end_turn"`.

**Why Messages API (not Managed Agents):**
- Bolly uses only **custom tools** (outreach, memory, drops, cross-instance). The Managed Agents sandbox container (`agent_toolset_20260401` with bash/read/write/etc.) is overhead we would pay for and never use.
- **ZDR is confirmed for Messages API.** Managed Agents is not in the ZDR feature eligibility table; coverage is unclear.
- Simpler surface — no Agent/Environment/Session/Event/Vault resources to manage, no beta API stability risk for the core path.
- Messages API still gives us everything we need: tool use, streaming, server-side compaction (`context_management`), and prompt caching (`cache_control`).

**The loop:**

```typescript
async function runMind(employeeSlug: string, trigger: TriggerContext) {
  const messages = loadRecentConversation(employeeSlug);
  messages.push({ role: "user", content: buildMindPrompt(trigger) });

  while (true) {
    const stream = anthropic.messages.stream({
      model: "claude-sonnet-4-6",
      max_tokens: 4096,
      system: [
        { type: "text", text: buildSystemPrompt(soul, mood, enabledSkills),
          cache_control: { type: "ephemeral" } },
      ],
      tools: buildCustomTools(enabledSkills),        // JSON-schema defs
      messages,
      context_management: {
        edits: [{ type: "compact_20260112", trigger: { type: "input_tokens", value: 150000 } }],
      },
      betas: ["compact-2026-01-12"],
    });

    // Stream assistant text → WebSocket ChatStreamDelta broadcasts
    for await (const event of stream) {
      if (event.type === "content_block_delta" && event.delta.type === "text_delta") {
        broadcastStreamDelta(employeeSlug, event.delta.text);
      }
    }

    const response = await stream.finalMessage();
    messages.push({ role: "assistant", content: response.content });
    persistConversation(employeeSlug, response);

    if (response.stop_reason === "end_turn") break;

    if (response.stop_reason === "max_tokens") {
      messages.push({ role: "user", content: CONTINUATION_PROMPT });
      continue;
    }

    if (response.stop_reason === "tool_use") {
      const toolResults = [];
      for (const block of response.content) {
        if (block.type !== "tool_use") continue;
        const { content, isError } = await executeSkillOrOutreach(
          employeeSlug, block.name, block.input,
        );
        toolResults.push({
          type: "tool_result",
          tool_use_id: block.id,
          content,
          is_error: isError,
        });
      }
      messages.push({ role: "user", content: toolResults });
      continue;
    }

    // refusal, stop_sequence, pause_turn — log and break
    break;
  }
}
```

~100 LoC of well-tested code. No managed surface, no beta runtime dependency for the core path (only the `compact-2026-01-12` beta header, which is a request-level feature flag, not a separate API).

**Process model — one Node worker per active employee, lazy-started:**
- First event for a dormant employee → spawn the worker (loads persona, triage.md, conversation, skills → ready).
- Warm TTL: 10 min of empty queue → teardown. State is on disk; next event cold-starts.
- Every company deployment is a single Node process with a pool of lightweight per-employee workers sharing the Anthropic client. Employee count per deployment is expected in the 10s for v1; no distributed sharding needed.

**Authentication — one path:**
- `ANTHROPIC_API_KEY` env var (company-provided at deploy, or BYOK per employee via `settings.toml`).
- No Managed Agents, no Claude Max — Messages API with a standard API key is the only auth path.

**State loaded at mind-worker boot (local):**
- `soul.md`, `mood.md`, `rhythm.json` — composed into system prompt
- Last 50 `conversation.json` entries
- Enabled skills — each becomes a custom tool definition in the `tools` array; body is injected into the system prompt as guidance
- `triage.md` — needed by the triage gate, also passed to the mind as context

**System prompt shape (single cached text block):**
```
You are Bolly, the AI coworker for {employee_name} at {company_name}.

<persona>{soul.md}</persona>
<mood>{mood summary}</mood>
<team>{employees in shared/instances.json}</team>

<skills>
For each enabled skill, a block with:
  name: email-morning-check
  when-to-use: {the skill's "triggers" frontmatter, paraphrased}
  body: {full skill markdown body}
  tool: run_email_morning_check
</skills>

Use the custom tools available to you. You may reach out to {employee_name}
via send_push, send_email, or defer_for_digest.
```

Cached with `cache_control: { type: "ephemeral" }` — 5-minute TTL, 10% read cost on cache hits. For a stable system + growing conversation, cache hit rate ≥ 80% is achievable.

**Compaction:** enabled via `context_management: { edits: [{ type: "compact_20260112", trigger: { value: 150000 } }] }` plus `compact-2026-01-12` beta header. When conversation crosses 150k input tokens, Anthropic inserts a `compaction` content block summarizing prior turns; on the next request we echo the full response back and Anthropic automatically drops pre-compaction messages. We record the compacted summary in `conversation.json` and broadcast a `ContextCompacting` event to the client.

**Streaming** uses the standard Messages streaming format (`content_block_start`, `content_block_delta`, `content_block_stop`, `message_delta`). We map `text_delta` → `ChatStreamDelta` and `content_block_start` for `tool_use` blocks → `ToolOutputChunk` for client-side preview.

**Graceful shutdown:** SIGTERM → finish any in-flight turn (bounded by `max_tokens`), flush `conversation.json`, broadcast a final `ChatSnapshot`.

### Events & triage

**Event envelope** — every source produces the same shape:

```typescript
type Event = {
  id: string;              // ULID for idempotency
  user_id: string;
  source:
    | "user_msg" | "user_activity"
    | "email" | "calendar"
    | "scheduled" | "idle"
    | "skill_emit" | "instance_emit";
  ts: number;              // unix ms
  payload: Record<string, unknown>;   // source-specific
  skill_hint?: string;     // skill name if source = scheduled from a skill
};
```

**Event queue.** In-memory FIFO on the user's mind process. Dedupe window is source-specific (email: 30s, calendar: 10s, user_activity: 2s). Not durable; durability lives at the source (webhooks retry, scheduled jobs persist in files). Mind process crash → events replay on wake.

**Triage prompt shape (Haiku):**

```
You are the triage layer for Bolly. Decide: ignore | digest | escalate.

<soul>{first 200 chars of soul.md}</soul>
<mood>{current mood}</mood>
<budget_state>{spent}/{cap} — {ok|tight|suppressed}</budget_state>

<triage_rules>{full triage.md}</triage_rules>
<recent_outreach>{last 10 outreach.jsonl entries}</recent_outreach>

<event>{event JSON, truncated to 1KB}</event>

Respond exactly one line: DECISION=<ignore|digest|escalate> REASON=<short>
```

Cost per call: ~$0.001. Even 500 events/day per user = $0.50.

**Triage outputs:**
- `ignore` — drop. Only a metric increment recorded.
- `digest` — append to `digest.jsonl`. Surfaced to mind on user's next return ("while you were away…").
- `escalate` — passed to the mind with triage's `reason`.

**`triage.md` — the rules file.** Written by the mind itself based on user conversation. Human-readable, NL. Example:

```markdown
# Triage rules

Default: unless matched below, ignore.

## Always escalate
- User sent a message in the app
- Email from my boss or containing "urgent"
- Calendar event starting in <5 min I haven't been reminded of
- A skill I installed explicitly asks for escalation

## Always digest (never wake the user)
- Newsletter emails
- GitHub notification emails
- Calendar changes in the past

## Quiet hours
- Between 22:00 and 07:00: only escalate if "emergency" in subject
```

**Budget-gate interaction:** triage prompt includes current budget state. On `tight` (70–99% of cap), an appended directive: *"Only escalate if truly urgent."* On `suppressed` (≥100%), all escalate decisions silently become digests; triage still runs (cheap).

**Scheduled events.** NL-defined schedules ("every weekday at 8am") are compiled to cron expressions and stored as `instances/{slug}/scheduled/{id}.json`. One in-process scheduler (a single setInterval in the main Node process) polls every 30s across all instances in `$BOLLY_HOME/instances/`, enqueues due jobs, and wakes dormant employee mind workers.

### Data layout

**No database.** All state is filesystem-based. Access patterns are single-writer per user (one mind process = no contention) and cross-user queries are offline-batch (analytics, billing) — not runtime-critical.

**Layout:**

```
$BOLLY_HOME/
├── instances/{slug}/
│   ├── soul.md, mood.md, rhythm.json         persona
│   ├── chats/{chat_id}/conversation.json     was rig_history.json
│   ├── skills/{name}.md                      NL-configured capabilities
│   ├── triage.md                             triage rules
│   ├── drops/, memories/, thoughts/          unchanged
│   ├── vector/                               Gemini-embedded, unchanged
│   ├── budget/{YYYY-MM-DD}.json              daily spend ledger
│   ├── scheduled/{id}.json                   NL-defined schedules
│   ├── mcp/{name}.json                       MCP server configs
│   ├── outreach.jsonl                        append-only audit
│   └── settings.toml                         user preferences
├── shared/                                   cross-instance (see next section)
└── config.toml                               global config
```

**Budget ledger file format:**

```json
{
  "day": "2026-04-22",
  "calls": 142,
  "tokens_in": 45621,
  "tokens_out": 3210,
  "dollars_spent": 0.84,
  "cap_usd": 2.00,
  "state": "ok"
}
```

Written atomically via write-tmp-then-rename. Read on every LLM call via `chargeAndCall()` wrapper.

**`chargeAndCall()` enforcement:**

```typescript
async function chargeAndCall(userId, tier, fn) {
  const day = today(userId.timezone);
  const state = await budgetState(userId, day);   // ok | tight | suppressed

  if (tier === 3 && state === "suppressed") {
    return { downgraded: true, reason: "budget_cap" };
  }

  const result = await fn(state);                  // fn receives state
  await recordSpend(userId, day, result.usage);
  return result;
}
```

Thresholds: `ok` < 70%, `tight` 70–99%, `suppressed` ≥ 100%. Default `daily_budget_usd` = $2/employee/day (≈ $60/month; success criterion is total per-employee spend in $20–50/month band for typical usage, so a $2 hard cap gives comfortable headroom).

**Per-minute throttle** (defense-in-depth against runaway skills): max 5 tier-3 calls per minute per employee, independent of daily cap.

### Cross-instance (the team model)

This is the core v1 use case, not a side feature. Every Bolly deployment is a team: one `$BOLLY_HOME` hosts one employee per `instances/{slug}/` directory, plus a `shared/` directory that makes Bolly a team member rather than a solo assistant.

**`$BOLLY_HOME/shared/` layout:**

```
shared/
├── docs/                             company documents (shared via MCP)
├── memory/                           shared notes, facts
├── channel/{event_id}.json           inter-instance events
├── channel/.consumed/                processed events (audit)
├── instances.json                    registry + HMAC keys
└── triage.md                         optional shared triage policy
```

**Three mechanisms — all reuse existing primitives:**

1. **Shared docs and memory** — exposed as a built-in MCP filesystem server with ACL. No new Bolly concept; it's an MCP server the mind has access to. Company onboards by dropping files in `shared/docs/`.

2. **Cross-instance events** — an instance writes:

   ```json
   {
     "id": "01JKM...",
     "from": "alice",
     "to": "bob",
     "source": "instance_emit",
     "payload": { "message": "can you review the Q2 memo?" },
     "signature": "<hmac-sha256>"
   }
   ```

   to `shared/channel/{id}.json`. Each instance's event loop watches the directory (`fs.watch` / `inotify`). Events addressed to it enter that instance's triage queue as `source: instance_emit`. Receiver verifies HMAC against sender's key in `instances.json`. Processed events move to `shared/channel/.consumed/`.

3. **Instance registry** — `shared/instances.json` lists slugs, display names, last-seen timestamps, and per-instance HMAC public keys. Updated on boot and periodic heartbeat (hourly, not per event). Compromised instance cannot impersonate another.

**Example skill — Alice's Bolly can poke Bob's:**

```markdown
---
name: contact-colleague
tools: [emit_instance_event, read_instances, read_shared_memory]
---
When I ask you to send something to a colleague, look them up in instances.json,
then emit an instance_emit event. Default: their quiet hours respected unless I mark urgent.
```

Bob's triage decides whether to escalate. Bob's mind might respond via another cross-instance event, or write to `shared/memory/` where Alice's Bolly can find it. The "swarm" emerges from shared files plus file-drop events — no central orchestration service.

**Threat model for v1:** self-hosting trusts the filesystem. `shared/instances.json` HMAC keys are stored plaintext. Anyone with filesystem access has full control. Future: OS keychain or per-user passphrase encryption.

### Outreach

**Three tools on the mind's belt:**
- `send_push(title, body, urgency)` — interrupting, short
- `send_email(subject, body_markdown)` — async, detail
- `defer_for_digest(summary)` — surfaces on user's next app open

**Delivery (self-hosted, single path per channel):**

| Channel | Delivery |
|---|---|
| push | Web-push via VAPID — PWA's existing service worker. Company hosts no push relay; VAPID is browser-native. |
| email | SMTP to a provider the company configures at deploy time (Postmark, SES, Mailgun, or the company's own SMTP server). Falls back to `defer_for_digest` if SMTP is not configured. |
| digest | File-only at `instances/{slug}/digest.jsonl`; served to the client on next `ChatSnapshot`. |

Slack as a future outreach channel is scheduled for v1.1 — it's the highest-value next channel in a business context, but v1 proves the core with push + email.

**Channel selection** — the mind's system prompt includes:
- push: time-sensitive, <100 chars, worth interrupting
- email: detail, async, morning briefs, summaries
- digest (default): anything not pressing

**Guardrails — enforced outside the mind:**
- Max 5 pushes/day unless `urgency: high`
- Max 2 emails/day unless a skill explicitly raises the cap (e.g., daily brief)
- Quiet hours (default 22:00–07:00 local, NL-editable): no push; urgent-only email
- Dedup via fuzzy content hash — if a similar outreach went out in the last 60 min, skip

**Audit trail** — `instances/{slug}/outreach.jsonl`:

```json
{"id":"01JK...","ts":1714...,"channel":"push","title":"Q2 memo review",
 "urgency":"medium","delivered":true,"dedup_suppressed":false}
```

Last 10 entries feed back into the triage prompt's context → Bolly self-regulates chattiness.

**User preferences — `settings.toml`:**

```toml
quiet_hours = { start = "22:00", end = "07:00" }
push  = { enabled = true, daily_max = 5 }
email = { enabled = true, address = "user@...", daily_max = 2 }
```

NL-editable through chat ("don't email me, just push" → mind rewrites the file).

## Launch strategy

**Clean-slate v1.0.** No users today, so no migration, no backward compatibility, no parallel running. Build v1.0 in TypeScript on a branch; when it passes internal QA, cut over and delete Rust.

**10-week plan:**

| Week | Phase | Ships |
|---|---|---|
| 1–2 | Foundations | Repo scaffold (TS + pnpm), storage module, budget ledger, triage module — all isolated + unit-tested |
| 3–5 | Mind runtime | Anthropic Messages client wrapper, system-prompt assembly from skills, agent loop (tool use + compaction + caching + streaming), custom-tool handlers, WebSocket broadcast matching wire format |
| 6–7 | Events & triage | Event queue, triage gate (Messages API + Haiku), scheduled jobs, cross-instance channel watcher |
| 8 | Outreach | Push (VAPID) + email (SMTP) + digest + guardrails |
| 9 | Cross-instance | Shared-fs MCP, instance registry, HMAC signing |
| 10 | Polish + QA | End-to-end tests, 24h cost validation, `docker compose` image, v1.0 release |

**Client work (~1 week, parallel with backend phases):**
- New endpoints: `GET/PUT /api/skills`, `GET/PUT /api/settings`, `GET/PUT /api/triage`, `GET /api/outreach`
- One new WebSocket variant: `OutreachSent`
- Settings page: quiet hours, push/email prefs, daily budget cap, API key
- Skills management page (list, enable/disable, view body) — ~300 LoC Svelte
- Core chat, drops, moods, WebSocket handling: unchanged

**Deployment artifact:** a published Docker image (or multi-arch `docker compose` bundle with the Node server, Svelte static build, and any sidecars). Company admin:

1. Pulls the image
2. Sets `ANTHROPIC_API_KEY`, `BOLLY_HOME`, SMTP config in `.env`
3. `docker compose up`
4. Opens the web UI, creates admin account, invites employees

Full admin flow under 10 minutes (success criterion).

**Clean-break declaration** (README):

> Bolly v1.0 is a full rewrite with a different product shape — a self-hosted team coworker instead of a personal companion. It is **not compatible** with v0.x workspace directories. Fresh install required. v0.x binaries remain available for individual users who prefer the prior model.

**Version jump:** `v0.32.0` → `v1.0.0`. Honest break.

## Testing strategy

**TDD-first.** The Rust codebase's behaviors are *reference material* for correct semantics (max_tokens continuation pattern, pause_turn handling, atomic write-then-rename), not compatibility targets.

**Layered test strategy:**

| Layer | Test approach |
|---|---|
| Budget ledger | Unit; pure functions; state transitions, UPSERT, midnight rollover |
| Triage gate | Fixture-based (event + triage.md → decision). Mock Haiku for unit; real Haiku in integration |
| Event queue | Unit; dedupe window, backpressure, ordering |
| Mind runtime | Unit (mock Anthropic client — in-memory fake that returns canned `MessageStreamEvent` sequences, including tool-use loops and compaction blocks); integration (real `/v1/messages` calls, `INTEGRATION=1`, ~10 tests, ~$0.50 nightly) |
| Skills | Unit; frontmatter parse, tool allowlist, Agent definition assembly |
| Cross-instance | Unit; file-watch triggers, HMAC verify (valid/tampered/missing), quiet hours respected |
| Outreach | Unit; dedup hash, rate limits, quiet hours, audit feedback |

**E2E flows (Playwright or equivalent against a running dev server):**
- Chat round-trip
- Skill install via NL
- Scheduled task (test clock advances time)
- Cross-instance event (alice → bob round-trip)
- Budget cap (synthesize 500 events, assert progressive downgrade)

**Cost validation — non-functional, critical:**

Single 24h simulation test. Replay a realistic event stream end-to-end with real `/v1/messages` calls. Assertions:

- Total spend ≤ $2/employee/day (≈ $60/employee/month worst case)
- Triage escalation rate 5–30% (below 5% feels dead; above 30% is wasteful)
- P99 outreach latency < 3s
- Prompt cache hit rate > 80% on `cache_read_input_tokens` after the first mind call per employee per day
- No mind turn takes > 30s wall clock for typical triage-triggered engagement

Runs pre-release and weekly in CI; ~$1 per run.

**CI tiers:**
- PR (~2 min): unit + integration with mocked API
- Nightly (~10 min, ~$0.50): full suite with real Claude/Haiku
- Pre-release (~15 min, ~$2): includes 24h simulation

## Alternatives considered

### Topology

- **A — Peer agents on a bus.** Each agent self-contained, event-subscribed. Rejected: doesn't solve "feels mechanical" — you end up with N event-driven crons instead of one cron, no coherent voice, hard to configure via NL because the user has to grasp multiple peer lifecycles.
- **C — Front-stage mind + background peers.** Hybrid with autonomous peers publishing into a shared thought stream. Rejected: two runtime modes (mind and peer) = more complexity for a marginal gain over B, and Bolly is one character not a cast.
- **B — One mind with specialists (chosen).** One mind-loop per employee driving the Messages API; skills are custom tools the mind can call. NL config maps cleanly to "add a skill to the mind"; the triage-gated event-driven flow *is* the heartbeat, which solves the "feels mechanical" problem by construction.

### Runtime substrate

- **Claude Agent SDK (local loop).** Initial v1 draft assumed this. Rejected: Anthropic deprecated the SDK in favor of Managed Agents.
- **Claude Managed Agents.** Considered after Agent SDK was removed. Rejected after reading the Managed Agents feature surface: Bolly uses **only custom tools** (outreach, memory, drops, cross-instance), so the sandbox container that drives Managed Agents' value is overhead we pay for and never use. ZDR coverage for Managed Agents is also unclear (not in the feature eligibility table), which is unacceptable for the business pivot.
- **Claude Messages API, hand-rolled loop (chosen).** Tier 2 triage is a single stateless call; Tier 3 mind is a ~100-LoC TypeScript loop using `/v1/messages` with tool use, server-side compaction (`context_management` + `compact-2026-01-12` beta), and automatic prompt caching (`cache_control`). ZDR is confirmed. We get the features that matter (tool use, compaction, caching, streaming) without the surface area we don't need.

### Scope

- **#1 — Minimal runtime-only rewrite.** Migrate 1:1 to TS + Agent SDK, replace polling with events; keep `ChildAgentConfig`. Rejected: keeps the original "multi-agent too rigid" problem.
- **#3 — Full product reset.** Everything in #2 plus persona-first onboarding rebuild, messaging app integrations, ambient features. Rejected: 18-month scope against competitors moving in 4 months. Defer.
- **#2 — Runtime + heartbeat + skills (chosen).** Runtime, skills model, and heartbeat are load-bearing for each other; changing one without the others is incoherent. ~10–12 weeks; this spec.

### Storage

- **Postgres for cross-employee metadata.** Rejected: access pattern is single-writer-per-employee (one worker per active employee); cross-employee queries are offline-batch analytics only. Filesystem suffices and keeps the self-hosted deployment artifact a single container with no stateful dependency.

## Open questions

1. **Prompt cache hit rate under compaction.** After a `compact_20260112` event, the message prefix changes — cache entries for prior turns are invalidated. The system prompt stays cached separately if marked with its own `cache_control` breakpoint (Anthropic docs recommend exactly this). The cost-validation test in Section 9 measures hit rate directly; if below 80% after compaction, we experiment with explicit breakpoints on stable content.
2. **Tool system-prompt overhead (~346 tokens per request).** Fixed cost added by declaring tools. Cacheable with the system prompt. Not a problem for our budget; noted for transparency.
3. **Skill edit concurrency.** If the mind is mid-turn when the user disables a skill, the current turn finishes with the old tools list; the disable applies next invocation. Acceptable for v1.
4. **Instance registry bootstrap.** Instances auto-register on boot (write own entry, read others). Open: slug collision handling — for v1, refuse to start and prompt admin to resolve.
5. **Quiet-hours timezone.** Per-employee (read from `settings.toml`), fall back to the server's system timezone. Confirmed.
6. **Admin bootstrap.** First-time `docker compose up` — how does the admin account get created? `BOLLY_ADMIN_EMAIL` env var seeds a first-login token; admin clicks through a one-time setup wizard that creates their instance.
7. **Prompt leak prevention validation.** Adversarial test suite (Section 9) runs ~50 attacks against every mind-runtime build. If a single one returns `soul.md` verbatim or another employee's content, build fails. Specific attack list to be written in Plan 2.

## Risks & mitigations

| Risk | Mitigation |
|---|---|
| NanoClaw ships a team edition and beats us to market | Book persona-first onboarding + Slack outreach for v1.1 immediately after v1 |
| Messages API changes (e.g., compaction GA breaking current `compact-2026-01-12` beta shape) | Beta header is pinned; we track release notes. Messages API itself is stable (GA) — only the compaction header is beta. If compaction API changes, we update one call site. |
| Prompt cache hit rate below 80% → cost model breaks | Cost-validation test fails closed in pre-release CI; if caching underperforms after compaction events, we add explicit `cache_control` breakpoints on stable content (system prompt, early conversation) to get ≥ 3 cache levels |
| Prompt leak of soul/skills/cross-employee content | Adversarial prompt-leak test suite in CI; output filter on assistant messages before persistence/broadcast; permission-gated tools for cross-instance reads |
| Triage makes bad decisions (misses / spams) | Observable triage log; user-facing "why did Bolly do that?" explainer reading `reason` field; iterate prompt on real data |
| Runaway skill burns budget before daily cap triggers | Per-minute throttle (5 tier-3 calls/min/employee) alongside daily cap |
| Cross-instance HMAC keys in plaintext on filesystem | v1 documents the threat model ("self-hosting trusts the filesystem"); v1.x adds OS keychain / passphrase encryption |
| Employee mind worker hangs → event backlog | Health ping every 30s; kill + restart on no pong. Node's single-threaded model makes this rare but possible. |
| Self-hosted deployments hit cross-employee privacy issues | Document the threat model per deployment: `shared/` is by-definition shared, per-instance dirs are not. Tools that cross the boundary require explicit user-in-loop confirmation in v1. |
| Scope creep in 10-week plan | Strict phase gates; non-critical deferred to v1.1 |

## Appendix A — Competitive landscape

Business-AI-assistant field (our new neighborhood):

| Product | Position | Relevance to Bolly |
|---|---|---|
| Microsoft Copilot for M365 | Embedded AI in Office, Teams, Outlook. Single-user scope; no cross-employee awareness. | Bolly's cross-instance layer is precisely what Copilot can't do — it's scoped to one user's context by design. |
| [Glean](https://www.glean.com/) | Enterprise search + AI over company docs and apps. Passive, query-response. | Glean retrieves; Bolly acts. Users invoke Glean; Bolly initiates. The triage + outreach pattern is our clearest differentiation. |
| Notion AI | AI over team knowledge base. Lives inside Notion. | Bolly is not bound to a single surface. Lives in its own UI and reaches out through push/email. |
| Slack AI | AI summaries and search within Slack. Slack-bound. | Slack is a future outreach channel for us, not a home. Bolly has memory and initiative Slack AI lacks. |
| [NanoClaw](https://github.com/qwibitai/nanoclaw) | Lightweight, container-isolated, ~3,900 LoC on Claude Agent SDK. Agent-first, no persona layer. | Most direct technical threat. They could ship a team edition. Our moat is **persistent character + autonomous heartbeat + team coordination**, not any individual feature. |
| [OpenClaw](https://openclaw.ai/) | Runs inside messaging apps. Personal, not team-oriented. Has documented security issues. | Different market; we note the security lessons (per-instance isolation, HMAC signing) as guardrails we don't want to skip. |

Persona companions (former neighborhood, reference only):

| Product | Position | Relevance |
|---|---|---|
| [Kindroid](https://scribehow.com/page/Kindroid_Review_2026_The_Personality-First_AI_Companion_Tested__eqIQ8k4zRc6ciUhoEAuOHw), [Replika, Nomi](https://nomi.ai/ai-today/replika-vs-nomi-2026-finding-enduring-ai-companionship/) | Consumer persona companions. Layered onboarding, no agentic capability. | Reference for any future persona-first onboarding work. Not competitors in the business lane. |

**Strategic position:** Bolly is an AI coworker with persistent character and initiative, self-hosted inside a company's environment. Copilot is the assistant embedded in your Office app; Glean is the retrieval layer for your docs; Notion AI is the summarizer in your wiki. Bolly is the entity that *remembers*, *notices*, and *acts* on your team's behalf — using those other tools as context or channels when appropriate.

**Industry alignment:**
- Centralized orchestrator-worker (chosen topology B) matches consensus guidance to "start centralized, decentralize only when hitting concrete bottlenecks."
- Direct Messages API is the ZDR-eligible path for building agents that don't need the managed sandbox — the natural home for persona + custom-tool products like Bolly.
- Skills-as-markdown matches OpenClaw's markdown-memory pattern and is trivially reviewable by a company admin.
- MCP as the tool-server primitive lets companies plug in their own internal tools (JIRA, Linear, internal docs) without waiting for us to build integrations.

## Appendix B — Current Rust system reference

Key behaviors to preserve (as TDD test cases, not code to port):

- **Agent loop stop-reason handling** (`server/src/services/llm/agent_loop.rs`):
  - `max_tokens` → inject specific continuation user message, re-loop
  - `pause_turn` → re-loop without message injection
  - `compaction` → broadcast `ContextCompacting`, persist, broadcast `ChatSnapshot`
- **Atomic `conversation.json` writes** — write `.tmp`, then rename (`chat.rs:1642`).
- **Compaction load-time trimming** — drop messages before the last compaction entry (`chat.rs:1613`).
- **Tool execution error format** — `"error: {e}"` and `"error: unknown tool 'X'"`.
- **Scheduled task file deletion** — delete *before* injecting message (prevents re-trigger on overlapping tick).
- **Message ID format** — `msg_{unix_millis}_{counter}`, monotonic within a process.
- **WebSocket lag resync hint** — exact string `{"type":"resync","reason":"lagged"}`.

These are behaviors the new TS implementation should replicate; the tests live in the new codebase, not ported from Rust.
