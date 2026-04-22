# Bolly v1 — Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the isolated, unit-tested foundation modules of the new TS backend — filesystem helpers, budget ledger, triage rules parser, skills parser, and core types — so that Plan 2 (Mind runtime) can compose them into a running server.

**Architecture:** A single standalone `server-ts/` pnpm package. Pure TypeScript modules with no runtime server, no Claude SDK calls, no HTTP. Everything testable in isolation with vitest. Modules grouped by responsibility (storage, budget, skills, triage, outreach-audit).

**Tech Stack:** Node.js 22+, TypeScript 5.x, pnpm, vitest, zod (schema validation), gray-matter (frontmatter), smol-toml (TOML), ulid (ids), tinyglobby (fs glob).

**Scope (what this plan ships):**
- `server-ts/` scaffolded with build + test commands
- `src/paths.ts` — filesystem path helpers
- `src/fs-atomic.ts` — atomic file write utilities
- `src/json-file.ts` and `src/toml-file.ts` — typed read/write helpers
- `src/types.ts` — core type definitions (Event, Skill, BudgetState, etc.)
- `src/budget/state.ts` — pure state calculator
- `src/budget/ledger.ts` — daily ledger load/save/recordSpend
- `src/budget/charge-and-call.ts` — LLM call wrapper with enforcement
- `src/budget/throttle.ts` — per-minute rolling window throttle
- `src/skills/parse.ts` — frontmatter + body parser
- `src/skills/loader.ts` — directory scanner
- `src/triage/rules.ts` — triage.md reader
- `src/triage/prompt.ts` — Haiku prompt builder (returns string, no API call)
- `src/outreach/audit.ts` — outreach.jsonl appender
- `src/settings/reader.ts` — settings.toml parser with defaults

**Out of scope (later plans):**
- Claude Agent SDK integration (Plan 2)
- HTTP/WebSocket server (Plan 2)
- Event queue, triage execution, scheduled jobs (Plan 3)
- Outreach delivery (Plan 4)
- Cross-instance (Plan 5)

**Filesystem paths used in tests:** tests use `tmp/` directories from `node:os.tmpdir()` via `fs.mkdtemp`, cleaned up in `afterEach`. No writes to real `$BOLLY_HOME`.

**Commit convention:** One commit per task. Messages use `feat:`, `test:`, `chore:` prefixes. All commits include the trailer `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`.

---

## File Structure

```
server-ts/
├── package.json
├── tsconfig.json
├── vitest.config.ts
├── biome.json
├── .gitignore
└── src/
    ├── index.ts                  re-exports public API
    ├── types.ts                  core type definitions
    ├── paths.ts                  filesystem path helpers
    ├── fs-atomic.ts              write-tmp-then-rename
    ├── json-file.ts              typed JSON read/write
    ├── toml-file.ts              typed TOML read/write
    ├── budget/
    │   ├── state.ts              pure state calculator
    │   ├── state.test.ts
    │   ├── ledger.ts             daily ledger module
    │   ├── ledger.test.ts
    │   ├── charge-and-call.ts    LLM wrapper
    │   ├── charge-and-call.test.ts
    │   ├── throttle.ts           per-minute throttle
    │   └── throttle.test.ts
    ├── skills/
    │   ├── parse.ts              frontmatter + body parser
    │   ├── parse.test.ts
    │   ├── loader.ts             dir scanner
    │   └── loader.test.ts
    ├── triage/
    │   ├── rules.ts              triage.md reader
    │   ├── rules.test.ts
    │   ├── prompt.ts             prompt string builder
    │   └── prompt.test.ts
    ├── outreach/
    │   ├── audit.ts              jsonl appender
    │   └── audit.test.ts
    └── settings/
        ├── reader.ts             settings.toml parser
        └── reader.test.ts
```

---

## Task 1: Scaffold server-ts package

**Files:**
- Create: `server-ts/package.json`
- Create: `server-ts/tsconfig.json`
- Create: `server-ts/vitest.config.ts`
- Create: `server-ts/biome.json`
- Create: `server-ts/.gitignore`
- Create: `server-ts/src/index.ts`

- [ ] **Step 1: Create directory and package.json**

```bash
mkdir -p server-ts/src
```

Create `server-ts/package.json`:

```json
{
  "name": "@bolly/server",
  "version": "1.0.0-alpha.0",
  "private": true,
  "type": "module",
  "engines": {
    "node": ">=22"
  },
  "scripts": {
    "build": "tsc",
    "test": "vitest run",
    "test:watch": "vitest",
    "check": "tsc --noEmit && biome check src"
  },
  "dependencies": {
    "gray-matter": "^4.0.3",
    "smol-toml": "^1.3.1",
    "ulid": "^2.3.0",
    "zod": "^3.23.8"
  },
  "devDependencies": {
    "@biomejs/biome": "^1.9.4",
    "@types/node": "^22.10.0",
    "typescript": "^5.7.0",
    "vitest": "^2.1.8"
  }
}
```

- [ ] **Step 2: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2023",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "lib": ["ES2023"],
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "exactOptionalPropertyTypes": true,
    "noImplicitOverride": true,
    "declaration": true,
    "sourceMap": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "resolveJsonModule": true
  },
  "include": ["src/**/*"],
  "exclude": ["src/**/*.test.ts", "dist", "node_modules"]
}
```

- [ ] **Step 3: Create vitest.config.ts**

```typescript
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: false,
    include: ["src/**/*.test.ts"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.ts"],
      exclude: ["src/**/*.test.ts", "src/index.ts"],
    },
  },
});
```

- [ ] **Step 4: Create biome.json**

```json
{
  "$schema": "https://biomejs.dev/schemas/1.9.4/schema.json",
  "organizeImports": { "enabled": true },
  "linter": {
    "enabled": true,
    "rules": {
      "recommended": true,
      "style": { "noNonNullAssertion": "off" }
    }
  },
  "formatter": {
    "enabled": true,
    "indentStyle": "space",
    "indentWidth": 2,
    "lineWidth": 100
  }
}
```

- [ ] **Step 5: Create .gitignore and initial index.ts**

`server-ts/.gitignore`:

```
node_modules
dist
coverage
*.log
.env
.DS_Store
```

`server-ts/src/index.ts`:

```typescript
export const VERSION = "1.0.0-alpha.0";
```

- [ ] **Step 6: Install dependencies and verify build**

Run from `server-ts/`:

```bash
pnpm install
pnpm check
pnpm test
```

Expected: install succeeds; `check` passes; `test` reports "No test files found" (exit code 0 because vitest treats empty as non-failure with `run`).

- [ ] **Step 7: Commit**

```bash
git add server-ts
git commit -m "$(cat <<'EOF'
chore(server-ts): scaffold TypeScript package for v1 backend

Adds empty pnpm package with TypeScript, vitest, and biome configured.
No runtime code yet — subsequent tasks fill in foundation modules.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Core type definitions

**Files:**
- Create: `server-ts/src/types.ts`
- Create: `server-ts/src/types.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/types.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { EventSourceSchema, TriageDecisionSchema, BudgetStateSchema } from "./types.js";

describe("EventSourceSchema", () => {
  it("accepts the eight known sources", () => {
    const sources = [
      "user_msg",
      "user_activity",
      "email",
      "calendar",
      "scheduled",
      "idle",
      "skill_emit",
      "instance_emit",
    ];
    for (const s of sources) {
      expect(EventSourceSchema.parse(s)).toBe(s);
    }
  });

  it("rejects an unknown source", () => {
    expect(() => EventSourceSchema.parse("bogus")).toThrow();
  });
});

describe("TriageDecisionSchema", () => {
  it("accepts ignore, digest, escalate", () => {
    for (const d of ["ignore", "digest", "escalate"]) {
      expect(TriageDecisionSchema.parse(d)).toBe(d);
    }
  });

  it("rejects any other value", () => {
    expect(() => TriageDecisionSchema.parse("skip")).toThrow();
  });
});

describe("BudgetStateSchema", () => {
  it("accepts ok, tight, suppressed", () => {
    for (const s of ["ok", "tight", "suppressed"]) {
      expect(BudgetStateSchema.parse(s)).toBe(s);
    }
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run from `server-ts/`:

```bash
pnpm test src/types.test.ts
```

Expected: FAIL. vitest cannot find `./types.js` exports.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/types.ts`:

```typescript
import { z } from "zod";

export const EventSourceSchema = z.enum([
  "user_msg",
  "user_activity",
  "email",
  "calendar",
  "scheduled",
  "idle",
  "skill_emit",
  "instance_emit",
]);
export type EventSource = z.infer<typeof EventSourceSchema>;

export const EventSchema = z.object({
  id: z.string().min(1),
  user_id: z.string().min(1),
  source: EventSourceSchema,
  ts: z.number().int().nonnegative(),
  payload: z.record(z.unknown()).default({}),
  skill_hint: z.string().optional(),
});
export type Event = z.infer<typeof EventSchema>;

export const TriageDecisionSchema = z.enum(["ignore", "digest", "escalate"]);
export type TriageDecision = z.infer<typeof TriageDecisionSchema>;

export const TriageOutcomeSchema = z.object({
  decision: TriageDecisionSchema,
  reason: z.string(),
});
export type TriageOutcome = z.infer<typeof TriageOutcomeSchema>;

export const BudgetStateSchema = z.enum(["ok", "tight", "suppressed"]);
export type BudgetState = z.infer<typeof BudgetStateSchema>;

export const BudgetDailySchema = z.object({
  day: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
  calls: z.number().int().nonnegative().default(0),
  tokens_in: z.number().int().nonnegative().default(0),
  tokens_out: z.number().int().nonnegative().default(0),
  dollars_spent: z.number().nonnegative().default(0),
  cap_usd: z.number().positive(),
  state: BudgetStateSchema.default("ok"),
});
export type BudgetDaily = z.infer<typeof BudgetDailySchema>;

export const SkillFrontmatterSchema = z.object({
  name: z.string().min(1),
  created: z.string().optional(),
  triggers: z.array(z.record(z.unknown())).default([]),
  tools: z.array(z.string()).default([]),
  enabled: z.boolean().default(true),
});
export type SkillFrontmatter = z.infer<typeof SkillFrontmatterSchema>;

export const SkillSchema = z.object({
  frontmatter: SkillFrontmatterSchema,
  body: z.string(),
  path: z.string(),
});
export type Skill = z.infer<typeof SkillSchema>;

export const OutreachChannelSchema = z.enum(["push", "email", "digest"]);
export type OutreachChannel = z.infer<typeof OutreachChannelSchema>;

export const OutreachEntrySchema = z.object({
  id: z.string().min(1),
  ts: z.number().int().nonnegative(),
  channel: OutreachChannelSchema,
  title: z.string(),
  body: z.string().optional(),
  urgency: z.enum(["low", "medium", "high"]).default("medium"),
  delivered: z.boolean(),
  dedup_suppressed: z.boolean().default(false),
});
export type OutreachEntry = z.infer<typeof OutreachEntrySchema>;
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/types.test.ts
```

Expected: PASS — 3 test blocks, all green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/types.ts server-ts/src/types.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add core type schemas

Defines Event, TriageOutcome, BudgetDaily, Skill, OutreachEntry
using zod for runtime validation and type inference.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Filesystem path helpers

**Files:**
- Create: `server-ts/src/paths.ts`
- Create: `server-ts/src/paths.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/paths.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import {
  bollyHome,
  instanceDir,
  chatDir,
  conversationFile,
  skillsDir,
  skillFile,
  triageFile,
  budgetDir,
  budgetDailyFile,
  outreachFile,
  sharedDir,
  sharedChannelDir,
  sharedInstancesFile,
} from "./paths.js";

describe("paths", () => {
  const home = "/tmp/bolly-test";

  it("bollyHome returns the passed root", () => {
    expect(bollyHome(home)).toBe(home);
  });

  it("instanceDir joins home + instances + slug", () => {
    expect(instanceDir(home, "alice")).toBe("/tmp/bolly-test/instances/alice");
  });

  it("chatDir nests under instance/chats/chatId", () => {
    expect(chatDir(home, "alice", "default")).toBe(
      "/tmp/bolly-test/instances/alice/chats/default",
    );
  });

  it("conversationFile lives inside chatDir", () => {
    expect(conversationFile(home, "alice", "default")).toBe(
      "/tmp/bolly-test/instances/alice/chats/default/conversation.json",
    );
  });

  it("skillsDir is instances/{slug}/skills", () => {
    expect(skillsDir(home, "alice")).toBe(
      "/tmp/bolly-test/instances/alice/skills",
    );
  });

  it("skillFile suffixes .md", () => {
    expect(skillFile(home, "alice", "email-check")).toBe(
      "/tmp/bolly-test/instances/alice/skills/email-check.md",
    );
  });

  it("triageFile is instances/{slug}/triage.md", () => {
    expect(triageFile(home, "alice")).toBe(
      "/tmp/bolly-test/instances/alice/triage.md",
    );
  });

  it("budgetDir is instances/{slug}/budget", () => {
    expect(budgetDir(home, "alice")).toBe(
      "/tmp/bolly-test/instances/alice/budget",
    );
  });

  it("budgetDailyFile uses YYYY-MM-DD.json", () => {
    expect(budgetDailyFile(home, "alice", "2026-04-22")).toBe(
      "/tmp/bolly-test/instances/alice/budget/2026-04-22.json",
    );
  });

  it("outreachFile is instances/{slug}/outreach.jsonl", () => {
    expect(outreachFile(home, "alice")).toBe(
      "/tmp/bolly-test/instances/alice/outreach.jsonl",
    );
  });

  it("sharedDir is home/shared", () => {
    expect(sharedDir(home)).toBe("/tmp/bolly-test/shared");
  });

  it("sharedChannelDir is home/shared/channel", () => {
    expect(sharedChannelDir(home)).toBe("/tmp/bolly-test/shared/channel");
  });

  it("sharedInstancesFile is home/shared/instances.json", () => {
    expect(sharedInstancesFile(home)).toBe(
      "/tmp/bolly-test/shared/instances.json",
    );
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/paths.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/paths.ts`:

```typescript
import { join } from "node:path";

export function bollyHome(home: string): string {
  return home;
}

export function instanceDir(home: string, slug: string): string {
  return join(home, "instances", slug);
}

export function chatDir(home: string, slug: string, chatId: string): string {
  return join(instanceDir(home, slug), "chats", chatId);
}

export function conversationFile(home: string, slug: string, chatId: string): string {
  return join(chatDir(home, slug, chatId), "conversation.json");
}

export function skillsDir(home: string, slug: string): string {
  return join(instanceDir(home, slug), "skills");
}

export function skillFile(home: string, slug: string, name: string): string {
  return join(skillsDir(home, slug), `${name}.md`);
}

export function triageFile(home: string, slug: string): string {
  return join(instanceDir(home, slug), "triage.md");
}

export function budgetDir(home: string, slug: string): string {
  return join(instanceDir(home, slug), "budget");
}

export function budgetDailyFile(home: string, slug: string, day: string): string {
  return join(budgetDir(home, slug), `${day}.json`);
}

export function outreachFile(home: string, slug: string): string {
  return join(instanceDir(home, slug), "outreach.jsonl");
}

export function settingsFile(home: string, slug: string): string {
  return join(instanceDir(home, slug), "settings.toml");
}

export function sharedDir(home: string): string {
  return join(home, "shared");
}

export function sharedChannelDir(home: string): string {
  return join(sharedDir(home), "channel");
}

export function sharedInstancesFile(home: string): string {
  return join(sharedDir(home), "instances.json");
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/paths.test.ts
```

Expected: PASS — 13 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/paths.ts server-ts/src/paths.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add filesystem path helpers

Pure path-join helpers for every file shape documented in the spec:
instance/, chats/, skills/, budget/, triage.md, outreach.jsonl, shared/.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Atomic file write utility

**Files:**
- Create: `server-ts/src/fs-atomic.ts`
- Create: `server-ts/src/fs-atomic.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/fs-atomic.test.ts`:

```typescript
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { atomicWrite } from "./fs-atomic.js";

describe("atomicWrite", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "bolly-atomic-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("writes content to the target path", async () => {
    const target = join(dir, "hello.txt");
    await atomicWrite(target, "hello world");
    const contents = await readFile(target, "utf8");
    expect(contents).toBe("hello world");
  });

  it("creates parent directories if missing", async () => {
    const target = join(dir, "nested", "deep", "file.txt");
    await atomicWrite(target, "ok");
    const contents = await readFile(target, "utf8");
    expect(contents).toBe("ok");
  });

  it("overwrites an existing file", async () => {
    const target = join(dir, "overwrite.txt");
    await writeFile(target, "old");
    await atomicWrite(target, "new");
    const contents = await readFile(target, "utf8");
    expect(contents).toBe("new");
  });

  it("cleans up its .tmp file on success", async () => {
    const target = join(dir, "cleanup.txt");
    await atomicWrite(target, "body");
    const { readdir } = await import("node:fs/promises");
    const entries = await readdir(dir);
    expect(entries).toEqual(["cleanup.txt"]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/fs-atomic.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/fs-atomic.ts`:

```typescript
import { mkdir, rename, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

/**
 * Write body to path atomically by writing to a .tmp sibling and renaming.
 * Mirrors the Rust backend's write-tmp-then-rename pattern.
 */
export async function atomicWrite(
  path: string,
  body: string | Uint8Array,
): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  const tmp = `${path}.tmp`;
  await writeFile(tmp, body);
  await rename(tmp, path);
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/fs-atomic.test.ts
```

Expected: PASS — 4 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/fs-atomic.ts server-ts/src/fs-atomic.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add atomic file write helper

write-tmp-then-rename, creates parent dirs, overwrites safely.
Matches the existing Rust backend pattern.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Typed JSON file helpers

**Files:**
- Create: `server-ts/src/json-file.ts`
- Create: `server-ts/src/json-file.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/json-file.test.ts`:

```typescript
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { z } from "zod";
import { readJson, writeJson } from "./json-file.js";

const PersonSchema = z.object({ name: z.string(), age: z.number() });

describe("readJson", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "bolly-json-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("parses a valid JSON file through the schema", async () => {
    const f = join(dir, "person.json");
    await writeFile(f, JSON.stringify({ name: "alice", age: 30 }));
    const result = await readJson(f, PersonSchema);
    expect(result).toEqual({ name: "alice", age: 30 });
  });

  it("returns null when the file is missing", async () => {
    const f = join(dir, "missing.json");
    const result = await readJson(f, PersonSchema);
    expect(result).toBeNull();
  });

  it("throws when the JSON is malformed", async () => {
    const f = join(dir, "broken.json");
    await writeFile(f, "{ not valid");
    await expect(readJson(f, PersonSchema)).rejects.toThrow(/parse/i);
  });

  it("throws when the shape fails schema validation", async () => {
    const f = join(dir, "wrong.json");
    await writeFile(f, JSON.stringify({ name: "alice", age: "thirty" }));
    await expect(readJson(f, PersonSchema)).rejects.toThrow();
  });
});

describe("writeJson", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "bolly-json-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("writes a pretty-printed JSON file that round-trips", async () => {
    const f = join(dir, "round.json");
    await writeJson(f, { name: "bob", age: 25 });
    const roundTripped = await readJson(f, PersonSchema);
    expect(roundTripped).toEqual({ name: "bob", age: 25 });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/json-file.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/json-file.ts`:

```typescript
import { readFile } from "node:fs/promises";
import type { ZodSchema } from "zod";
import { atomicWrite } from "./fs-atomic.js";

/**
 * Read a JSON file and validate it against a zod schema.
 * Returns null if the file does not exist.
 * Throws on parse errors or schema failures.
 */
export async function readJson<T>(
  path: string,
  schema: ZodSchema<T>,
): Promise<T | null> {
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw err;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    throw new Error(`failed to parse JSON at ${path}: ${(err as Error).message}`);
  }

  return schema.parse(parsed);
}

/**
 * Write a value as pretty JSON to path, atomically.
 */
export async function writeJson<T>(path: string, value: T): Promise<void> {
  await atomicWrite(path, JSON.stringify(value, null, 2));
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/json-file.test.ts
```

Expected: PASS — 5 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/json-file.ts server-ts/src/json-file.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add schema-validated JSON file helpers

readJson returns null for missing files, throws on parse/schema errors.
writeJson writes atomically via fs-atomic.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: TOML file helpers

**Files:**
- Create: `server-ts/src/toml-file.ts`
- Create: `server-ts/src/toml-file.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/toml-file.test.ts`:

```typescript
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { z } from "zod";
import { readToml } from "./toml-file.js";

const ConfigSchema = z.object({
  name: z.string(),
  nested: z.object({ value: z.number() }),
});

describe("readToml", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "bolly-toml-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("parses a valid TOML file through the schema", async () => {
    const f = join(dir, "config.toml");
    await writeFile(f, 'name = "alice"\n[nested]\nvalue = 42\n');
    const result = await readToml(f, ConfigSchema);
    expect(result).toEqual({ name: "alice", nested: { value: 42 } });
  });

  it("returns null when the file is missing", async () => {
    const f = join(dir, "missing.toml");
    const result = await readToml(f, ConfigSchema);
    expect(result).toBeNull();
  });

  it("throws when the TOML is malformed", async () => {
    const f = join(dir, "broken.toml");
    await writeFile(f, "not = [ valid");
    await expect(readToml(f, ConfigSchema)).rejects.toThrow(/parse/i);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/toml-file.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/toml-file.ts`:

```typescript
import { readFile } from "node:fs/promises";
import { parse as parseToml } from "smol-toml";
import type { ZodSchema } from "zod";

/**
 * Read a TOML file and validate it against a zod schema.
 * Returns null if the file does not exist.
 */
export async function readToml<T>(
  path: string,
  schema: ZodSchema<T>,
): Promise<T | null> {
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw err;
  }

  let parsed: unknown;
  try {
    parsed = parseToml(raw);
  } catch (err) {
    throw new Error(`failed to parse TOML at ${path}: ${(err as Error).message}`);
  }

  return schema.parse(parsed);
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/toml-file.test.ts
```

Expected: PASS — 3 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/toml-file.ts server-ts/src/toml-file.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add TOML read helper with schema validation

Read-only for now; write-side not needed yet (skills use markdown,
settings are infrequently written and formatting is not critical).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Budget state calculator

**Files:**
- Create: `server-ts/src/budget/state.ts`
- Create: `server-ts/src/budget/state.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/budget/state.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { computeState, TIGHT_THRESHOLD } from "./state.js";

describe("computeState", () => {
  it("returns ok when spend is below 70% of cap", () => {
    expect(computeState(0.5, 2.0)).toBe("ok");
    expect(computeState(1.39, 2.0)).toBe("ok");
    expect(computeState(0, 2.0)).toBe("ok");
  });

  it("returns tight at exactly 70% of cap", () => {
    expect(computeState(1.4, 2.0)).toBe("tight");
  });

  it("returns tight when spend is in [70%, 100%)", () => {
    expect(computeState(1.5, 2.0)).toBe("tight");
    expect(computeState(1.999, 2.0)).toBe("tight");
  });

  it("returns suppressed at exactly the cap", () => {
    expect(computeState(2.0, 2.0)).toBe("suppressed");
  });

  it("returns suppressed above the cap", () => {
    expect(computeState(3.0, 2.0)).toBe("suppressed");
  });

  it("exposes TIGHT_THRESHOLD as 0.7", () => {
    expect(TIGHT_THRESHOLD).toBe(0.7);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/budget/state.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/budget/state.ts`:

```typescript
import type { BudgetState } from "../types.js";

export const TIGHT_THRESHOLD = 0.7;

/**
 * Classify current spend vs cap into one of three budget states.
 * Pure function; no side effects.
 */
export function computeState(dollarsSpent: number, capUsd: number): BudgetState {
  if (capUsd <= 0) return "suppressed";
  const ratio = dollarsSpent / capUsd;
  if (ratio >= 1) return "suppressed";
  if (ratio >= TIGHT_THRESHOLD) return "tight";
  return "ok";
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/budget/state.test.ts
```

Expected: PASS — 6 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/budget/state.ts server-ts/src/budget/state.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add budget state calculator

Pure classifier: ok < 70%, tight 70-99%, suppressed at/above 100%.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Budget daily ledger

**Files:**
- Create: `server-ts/src/budget/ledger.ts`
- Create: `server-ts/src/budget/ledger.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/budget/ledger.test.ts`:

```typescript
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadDaily, recordSpend, todayUtc } from "./ledger.js";

describe("todayUtc", () => {
  it("formats a Date as YYYY-MM-DD in UTC", () => {
    const d = new Date(Date.UTC(2026, 3, 22, 5, 30)); // month is 0-indexed
    expect(todayUtc(d)).toBe("2026-04-22");
  });

  it("rolls over at UTC midnight, not local", () => {
    const d = new Date(Date.UTC(2026, 3, 22, 23, 59));
    expect(todayUtc(d)).toBe("2026-04-22");
    const after = new Date(Date.UTC(2026, 3, 23, 0, 1));
    expect(todayUtc(after)).toBe("2026-04-23");
  });
});

describe("loadDaily", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-ledger-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("returns a fresh ledger with defaults when file missing", async () => {
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger).toEqual({
      day: "2026-04-22",
      calls: 0,
      tokens_in: 0,
      tokens_out: 0,
      dollars_spent: 0,
      cap_usd: 2.0,
      state: "ok",
    });
  });

  it("returns the persisted ledger when file exists", async () => {
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 100,
      tokens_out: 20,
      dollars: 0.5,
    });
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger.calls).toBe(1);
    expect(ledger.tokens_in).toBe(100);
    expect(ledger.tokens_out).toBe(20);
    expect(ledger.dollars_spent).toBeCloseTo(0.5);
  });
});

describe("recordSpend", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-ledger-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("accumulates spend across calls", async () => {
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 100,
      tokens_out: 10,
      dollars: 0.3,
    });
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 200,
      tokens_out: 30,
      dollars: 0.5,
    });
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger.calls).toBe(2);
    expect(ledger.tokens_in).toBe(300);
    expect(ledger.tokens_out).toBe(40);
    expect(ledger.dollars_spent).toBeCloseTo(0.8);
  });

  it("updates state when spend crosses tight threshold", async () => {
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 0,
      tokens_out: 0,
      dollars: 1.5,
    });
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger.state).toBe("tight");
  });

  it("updates state to suppressed when spend reaches cap", async () => {
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 0,
      tokens_out: 0,
      dollars: 2.0,
    });
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger.state).toBe("suppressed");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/budget/ledger.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/budget/ledger.ts`:

```typescript
import { budgetDailyFile } from "../paths.js";
import { readJson, writeJson } from "../json-file.js";
import { BudgetDailySchema, type BudgetDaily } from "../types.js";
import { computeState } from "./state.js";

export function todayUtc(d: Date = new Date()): string {
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export async function loadDaily(
  home: string,
  slug: string,
  day: string,
  capUsd: number,
): Promise<BudgetDaily> {
  const path = budgetDailyFile(home, slug, day);
  const existing = await readJson(path, BudgetDailySchema);
  if (existing) return existing;

  return {
    day,
    calls: 0,
    tokens_in: 0,
    tokens_out: 0,
    dollars_spent: 0,
    cap_usd: capUsd,
    state: "ok",
  };
}

export type SpendDelta = {
  tokens_in: number;
  tokens_out: number;
  dollars: number;
};

export async function recordSpend(
  home: string,
  slug: string,
  day: string,
  capUsd: number,
  delta: SpendDelta,
): Promise<BudgetDaily> {
  const current = await loadDaily(home, slug, day, capUsd);
  const next: BudgetDaily = {
    day,
    calls: current.calls + 1,
    tokens_in: current.tokens_in + delta.tokens_in,
    tokens_out: current.tokens_out + delta.tokens_out,
    dollars_spent: current.dollars_spent + delta.dollars,
    cap_usd: capUsd,
    state: computeState(current.dollars_spent + delta.dollars, capUsd),
  };
  await writeJson(budgetDailyFile(home, slug, day), next);
  return next;
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/budget/ledger.test.ts
```

Expected: PASS — 7 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/budget/ledger.ts server-ts/src/budget/ledger.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add budget daily ledger

loadDaily reads or initializes the day's ledger; recordSpend accumulates
deltas and recomputes state. All writes atomic via json-file.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: chargeAndCall LLM wrapper

**Files:**
- Create: `server-ts/src/budget/charge-and-call.ts`
- Create: `server-ts/src/budget/charge-and-call.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/budget/charge-and-call.test.ts`:

```typescript
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { chargeAndCall, type CallOutcome } from "./charge-and-call.js";

describe("chargeAndCall", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-cac-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("invokes fn with state ok when ledger is empty", async () => {
    let seenState: string | null = null;
    const result = await chargeAndCall(
      { home, slug: "alice", day: "2026-04-22", capUsd: 2.0, tier: 3 },
      async (state) => {
        seenState = state;
        return { ok: true, usage: { tokens_in: 10, tokens_out: 5, dollars: 0.01 } };
      },
    );
    expect(seenState).toBe("ok");
    expect((result as CallOutcome<unknown>).downgraded).toBeUndefined();
  });

  it("records spend after the call", async () => {
    await chargeAndCall(
      { home, slug: "alice", day: "2026-04-22", capUsd: 2.0, tier: 2 },
      async () => ({ ok: true, usage: { tokens_in: 100, tokens_out: 20, dollars: 0.05 } }),
    );
    const { loadDaily } = await import("./ledger.js");
    const ledger = await loadDaily(home, "alice", "2026-04-22", 2.0);
    expect(ledger.calls).toBe(1);
    expect(ledger.dollars_spent).toBeCloseTo(0.05);
  });

  it("downgrades tier 3 when state is suppressed without calling fn", async () => {
    // Seed ledger to suppressed
    const { recordSpend } = await import("./ledger.js");
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 0,
      tokens_out: 0,
      dollars: 2.5,
    });

    let fnCalled = false;
    const result = await chargeAndCall(
      { home, slug: "alice", day: "2026-04-22", capUsd: 2.0, tier: 3 },
      async () => {
        fnCalled = true;
        return { ok: true, usage: { tokens_in: 0, tokens_out: 0, dollars: 0 } };
      },
    );
    expect(fnCalled).toBe(false);
    expect(result).toEqual({ downgraded: true, reason: "budget_cap" });
  });

  it("allows tier 2 calls even when suppressed (triage is cheap)", async () => {
    const { recordSpend } = await import("./ledger.js");
    await recordSpend(home, "alice", "2026-04-22", 2.0, {
      tokens_in: 0,
      tokens_out: 0,
      dollars: 2.5,
    });

    let fnCalled = false;
    await chargeAndCall(
      { home, slug: "alice", day: "2026-04-22", capUsd: 2.0, tier: 2 },
      async () => {
        fnCalled = true;
        return { ok: true, usage: { tokens_in: 10, tokens_out: 5, dollars: 0.001 } };
      },
    );
    expect(fnCalled).toBe(true);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/budget/charge-and-call.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/budget/charge-and-call.ts`:

```typescript
import type { BudgetState } from "../types.js";
import { loadDaily, recordSpend, type SpendDelta } from "./ledger.js";

export type ChargeContext = {
  home: string;
  slug: string;
  day: string;
  capUsd: number;
  tier: 2 | 3;
};

export type CallSuccess<T> = {
  ok: true;
  value?: T;
  usage: SpendDelta;
};

export type CallDowngraded = {
  downgraded: true;
  reason: string;
};

export type CallOutcome<T> = CallSuccess<T> | CallDowngraded;

/**
 * Wraps an LLM call with budget enforcement.
 * - Loads today's ledger.
 * - For tier 3 when state is suppressed, returns a downgraded outcome without invoking fn.
 * - Otherwise calls fn, records spend, returns success.
 */
export async function chargeAndCall<T>(
  ctx: ChargeContext,
  fn: (state: BudgetState) => Promise<CallSuccess<T>>,
): Promise<CallOutcome<T>> {
  const ledger = await loadDaily(ctx.home, ctx.slug, ctx.day, ctx.capUsd);

  if (ctx.tier === 3 && ledger.state === "suppressed") {
    return { downgraded: true, reason: "budget_cap" };
  }

  const result = await fn(ledger.state);
  await recordSpend(ctx.home, ctx.slug, ctx.day, ctx.capUsd, result.usage);
  return result;
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/budget/charge-and-call.test.ts
```

Expected: PASS — 4 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/budget/charge-and-call.ts server-ts/src/budget/charge-and-call.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add chargeAndCall LLM wrapper

Gates LLM invocations against the daily budget ledger.
Tier 3 + suppressed = downgraded outcome, fn not invoked.
Tier 2 always runs (triage is cheap and needed even under budget pressure).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: Per-minute throttle

**Files:**
- Create: `server-ts/src/budget/throttle.ts`
- Create: `server-ts/src/budget/throttle.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/budget/throttle.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { Throttle } from "./throttle.js";

describe("Throttle", () => {
  it("allows the first N calls within a window", () => {
    const t = new Throttle({ maxCalls: 5, windowMs: 60_000 });
    const start = 1_000_000;
    for (let i = 0; i < 5; i++) {
      expect(t.check("alice", start + i * 100)).toBe(true);
    }
  });

  it("rejects the N+1st call within the window", () => {
    const t = new Throttle({ maxCalls: 3, windowMs: 60_000 });
    const start = 1_000_000;
    t.check("alice", start);
    t.check("alice", start + 1);
    t.check("alice", start + 2);
    expect(t.check("alice", start + 3)).toBe(false);
  });

  it("tracks quota independently per user", () => {
    const t = new Throttle({ maxCalls: 2, windowMs: 60_000 });
    const now = 1_000_000;
    t.check("alice", now);
    t.check("alice", now + 1);
    expect(t.check("alice", now + 2)).toBe(false);
    expect(t.check("bob", now + 2)).toBe(true);
  });

  it("decays entries outside the window", () => {
    const t = new Throttle({ maxCalls: 2, windowMs: 60_000 });
    const now = 1_000_000;
    t.check("alice", now);
    t.check("alice", now + 30_000);
    // Old entry still counts — third call blocked
    expect(t.check("alice", now + 40_000)).toBe(false);
    // After window passes, first entry drops out
    expect(t.check("alice", now + 65_000)).toBe(true);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/budget/throttle.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/budget/throttle.ts`:

```typescript
export type ThrottleConfig = {
  maxCalls: number;
  windowMs: number;
};

/**
 * Rolling-window per-user throttle. In-memory only — suitable for the
 * single-mind-per-user model where there's one process per user.
 */
export class Throttle {
  private readonly buckets = new Map<string, number[]>();

  constructor(private readonly config: ThrottleConfig) {}

  /**
   * Returns true if the call is allowed (and records it).
   * Returns false if the call would exceed the window's max.
   */
  check(userId: string, nowMs: number): boolean {
    const cutoff = nowMs - this.config.windowMs;
    const existing = this.buckets.get(userId) ?? [];
    const pruned = existing.filter((ts) => ts > cutoff);

    if (pruned.length >= this.config.maxCalls) {
      this.buckets.set(userId, pruned);
      return false;
    }

    pruned.push(nowMs);
    this.buckets.set(userId, pruned);
    return true;
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/budget/throttle.test.ts
```

Expected: PASS — 4 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/budget/throttle.ts server-ts/src/budget/throttle.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add rolling-window per-user throttle

In-memory defense against runaway skills. 5 tier-3 calls per minute
per user is the intended default; values are passed in at construction.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: Skill frontmatter parser

**Files:**
- Create: `server-ts/src/skills/parse.ts`
- Create: `server-ts/src/skills/parse.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/skills/parse.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { parseSkill } from "./parse.js";

const VALID_SKILL = `---
name: email-morning-check
created: 2026-04-22
triggers:
  - scheduled: "every weekday at 8am"
  - event: "email arrives with 'urgent' in subject"
tools:
  - read_email
  - send_push
enabled: true
---

When triggered, read unread emails since last check.
Summarize anything the user would care about.
`;

describe("parseSkill", () => {
  it("parses a well-formed skill into frontmatter + body", () => {
    const skill = parseSkill(VALID_SKILL, "/instances/alice/skills/email.md");
    expect(skill.frontmatter.name).toBe("email-morning-check");
    expect(skill.frontmatter.tools).toEqual(["read_email", "send_push"]);
    expect(skill.frontmatter.enabled).toBe(true);
    expect(skill.frontmatter.triggers).toHaveLength(2);
    expect(skill.body).toContain("When triggered");
    expect(skill.path).toBe("/instances/alice/skills/email.md");
  });

  it("applies defaults for missing optional fields", () => {
    const minimal = `---
name: minimal
---

body here
`;
    const skill = parseSkill(minimal, "/x.md");
    expect(skill.frontmatter.enabled).toBe(true);
    expect(skill.frontmatter.tools).toEqual([]);
    expect(skill.frontmatter.triggers).toEqual([]);
  });

  it("throws when frontmatter is absent", () => {
    expect(() => parseSkill("no frontmatter here", "/x.md")).toThrow(/frontmatter/i);
  });

  it("throws when the required name field is missing", () => {
    const bad = `---
description: anonymous skill
---

body
`;
    expect(() => parseSkill(bad, "/x.md")).toThrow();
  });

  it("respects enabled: false", () => {
    const disabled = `---
name: sleeping
enabled: false
---

not right now
`;
    const skill = parseSkill(disabled, "/x.md");
    expect(skill.frontmatter.enabled).toBe(false);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/skills/parse.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/skills/parse.ts`:

```typescript
import matter from "gray-matter";
import { SkillFrontmatterSchema, type Skill } from "../types.js";

/**
 * Parse a skill file (YAML frontmatter + markdown body) into a Skill.
 * Throws if frontmatter is missing or does not match the schema.
 */
export function parseSkill(contents: string, path: string): Skill {
  if (!contents.trimStart().startsWith("---")) {
    throw new Error(`skill at ${path} is missing YAML frontmatter`);
  }

  const { data, content } = matter(contents);
  const frontmatter = SkillFrontmatterSchema.parse(data);

  return {
    frontmatter,
    body: content.trimStart(),
    path,
  };
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/skills/parse.test.ts
```

Expected: PASS — 5 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/skills/parse.ts server-ts/src/skills/parse.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add skill frontmatter parser

Uses gray-matter for YAML + body split, zod for schema validation.
Rejects files without frontmatter or without a name.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 12: Skill directory loader

**Files:**
- Create: `server-ts/src/skills/loader.ts`
- Create: `server-ts/src/skills/loader.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/skills/loader.test.ts`:

```typescript
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadSkills } from "./loader.js";

async function seed(home: string, slug: string, name: string, body: string): Promise<void> {
  const dir = join(home, "instances", slug, "skills");
  await mkdir(dir, { recursive: true });
  await writeFile(join(dir, `${name}.md`), body);
}

describe("loadSkills", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-skills-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("returns an empty array when no skills directory exists", async () => {
    const skills = await loadSkills(home, "alice");
    expect(skills).toEqual([]);
  });

  it("loads all .md files from the skills directory", async () => {
    await seed(home, "alice", "one", "---\nname: one\n---\n\nbody one");
    await seed(home, "alice", "two", "---\nname: two\n---\n\nbody two");
    const skills = await loadSkills(home, "alice");
    const names = skills.map((s) => s.frontmatter.name).sort();
    expect(names).toEqual(["one", "two"]);
  });

  it("skips non-.md files", async () => {
    await seed(home, "alice", "real", "---\nname: real\n---\n\nbody");
    await writeFile(
      join(home, "instances", "alice", "skills", "readme.txt"),
      "not a skill",
    );
    const skills = await loadSkills(home, "alice");
    expect(skills).toHaveLength(1);
    expect(skills[0]?.frontmatter.name).toBe("real");
  });

  it("filters out disabled skills when enabledOnly=true", async () => {
    await seed(home, "alice", "on", "---\nname: on\nenabled: true\n---\n\nbody");
    await seed(home, "alice", "off", "---\nname: off\nenabled: false\n---\n\nbody");
    const enabled = await loadSkills(home, "alice", { enabledOnly: true });
    expect(enabled.map((s) => s.frontmatter.name)).toEqual(["on"]);
  });

  it("returns all skills when enabledOnly omitted", async () => {
    await seed(home, "alice", "on", "---\nname: on\nenabled: true\n---\n\nbody");
    await seed(home, "alice", "off", "---\nname: off\nenabled: false\n---\n\nbody");
    const all = await loadSkills(home, "alice");
    expect(all).toHaveLength(2);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/skills/loader.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/skills/loader.ts`:

```typescript
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { skillsDir } from "../paths.js";
import type { Skill } from "../types.js";
import { parseSkill } from "./parse.js";

export type LoadSkillsOptions = {
  enabledOnly?: boolean;
};

/**
 * Load all skill .md files from an instance's skills directory.
 * Returns [] if the directory does not exist.
 */
export async function loadSkills(
  home: string,
  slug: string,
  opts: LoadSkillsOptions = {},
): Promise<Skill[]> {
  const dir = skillsDir(home, slug);
  let entries: string[];
  try {
    entries = await readdir(dir);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw err;
  }

  const results: Skill[] = [];
  for (const name of entries) {
    if (!name.endsWith(".md")) continue;
    const full = join(dir, name);
    const raw = await readFile(full, "utf8");
    const skill = parseSkill(raw, full);
    if (opts.enabledOnly && !skill.frontmatter.enabled) continue;
    results.push(skill);
  }
  return results;
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/skills/loader.test.ts
```

Expected: PASS — 5 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/skills/loader.ts server-ts/src/skills/loader.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add skill directory loader

Reads all .md files from instances/{slug}/skills/.
Optional enabledOnly flag filters out disabled skills.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 13: Triage rules reader

**Files:**
- Create: `server-ts/src/triage/rules.ts`
- Create: `server-ts/src/triage/rules.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/triage/rules.test.ts`:

```typescript
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadTriageRules, DEFAULT_TRIAGE_TEMPLATE } from "./rules.js";

describe("loadTriageRules", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-triage-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("returns the default template when triage.md is missing", async () => {
    const rules = await loadTriageRules(home, "alice");
    expect(rules).toBe(DEFAULT_TRIAGE_TEMPLATE);
  });

  it("returns the raw file contents when triage.md exists", async () => {
    const dir = join(home, "instances", "alice");
    await mkdir(dir, { recursive: true });
    const body = "# My rules\n\nAlways digest newsletters.";
    await writeFile(join(dir, "triage.md"), body);
    const rules = await loadTriageRules(home, "alice");
    expect(rules).toBe(body);
  });

  it("DEFAULT_TRIAGE_TEMPLATE contains the word Default", () => {
    expect(DEFAULT_TRIAGE_TEMPLATE).toMatch(/default/i);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/triage/rules.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/triage/rules.ts`:

```typescript
import { readFile } from "node:fs/promises";
import { triageFile } from "../paths.js";

export const DEFAULT_TRIAGE_TEMPLATE = `# Triage rules

Default: unless matched below, ignore.

## Always escalate
- User sent a message in the app
- Email or event marked urgent
- A skill I installed explicitly asks for escalation

## Always digest
- Newsletter emails
- Notification-only emails
- Calendar changes in the past

## Quiet hours
- Between 22:00 and 07:00: only escalate if "emergency" in subject
`;

/**
 * Read the user's triage rules file. Returns the default template when
 * the file is missing — the mind will overwrite it once the user expresses
 * preferences.
 */
export async function loadTriageRules(home: string, slug: string): Promise<string> {
  try {
    return await readFile(triageFile(home, slug), "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      return DEFAULT_TRIAGE_TEMPLATE;
    }
    throw err;
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/triage/rules.test.ts
```

Expected: PASS — 3 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/triage/rules.ts server-ts/src/triage/rules.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add triage rules reader

Returns the DEFAULT_TRIAGE_TEMPLATE when the user has not yet customized
their rules. The mind rewrites triage.md in response to user chat.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 14: Triage prompt builder

**Files:**
- Create: `server-ts/src/triage/prompt.ts`
- Create: `server-ts/src/triage/prompt.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/triage/prompt.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import type { Event } from "../types.js";
import { buildTriagePrompt } from "./prompt.js";

const EVENT: Event = {
  id: "01JKM000000000000000000000",
  user_id: "alice",
  source: "email",
  ts: 1714000000000,
  payload: { subject: "Q2 review", from: "boss@corp.com" },
};

describe("buildTriagePrompt", () => {
  it("includes soul, mood, budget, triage rules, and the event", () => {
    const prompt = buildTriagePrompt({
      soulSnippet: "Bolly is calm and attentive.",
      mood: "focused",
      budgetState: "ok",
      dollarsSpent: 0.42,
      capUsd: 2.0,
      triageRules: "# rules\n- urgent email -> escalate",
      event: EVENT,
      recentOutreach: [],
    });

    expect(prompt).toContain("<soul>Bolly is calm and attentive.</soul>");
    expect(prompt).toContain("<mood>focused</mood>");
    expect(prompt).toContain("0.42/2.00");
    expect(prompt).toContain("ok");
    expect(prompt).toContain("urgent email -> escalate");
    expect(prompt).toContain("Q2 review");
  });

  it("adds tightening directive when state is tight", () => {
    const prompt = buildTriagePrompt({
      soulSnippet: "",
      mood: "",
      budgetState: "tight",
      dollarsSpent: 1.5,
      capUsd: 2.0,
      triageRules: "",
      event: EVENT,
      recentOutreach: [],
    });
    expect(prompt).toContain("Only escalate if truly urgent");
  });

  it("adds suppression directive when state is suppressed", () => {
    const prompt = buildTriagePrompt({
      soulSnippet: "",
      mood: "",
      budgetState: "suppressed",
      dollarsSpent: 2.0,
      capUsd: 2.0,
      triageRules: "",
      event: EVENT,
      recentOutreach: [],
    });
    expect(prompt).toContain("budget cap reached");
  });

  it("includes the last N outreach entries for self-regulation context", () => {
    const prompt = buildTriagePrompt({
      soulSnippet: "",
      mood: "",
      budgetState: "ok",
      dollarsSpent: 0,
      capUsd: 2.0,
      triageRules: "",
      event: EVENT,
      recentOutreach: [
        { channel: "push", title: "reminder", ts: 1713999999000, urgency: "medium" },
        { channel: "email", title: "digest", ts: 1713999998000, urgency: "low" },
      ],
    });
    expect(prompt).toContain("push: reminder");
    expect(prompt).toContain("email: digest");
  });

  it("asks for the single-line response format", () => {
    const prompt = buildTriagePrompt({
      soulSnippet: "",
      mood: "",
      budgetState: "ok",
      dollarsSpent: 0,
      capUsd: 2.0,
      triageRules: "",
      event: EVENT,
      recentOutreach: [],
    });
    expect(prompt).toMatch(/DECISION=.*REASON=/);
  });

  it("truncates large event payloads", () => {
    const bigEvent: Event = {
      ...EVENT,
      payload: { body: "x".repeat(5000) },
    };
    const prompt = buildTriagePrompt({
      soulSnippet: "",
      mood: "",
      budgetState: "ok",
      dollarsSpent: 0,
      capUsd: 2.0,
      triageRules: "",
      event: bigEvent,
      recentOutreach: [],
    });
    expect(prompt.length).toBeLessThan(6000);
    expect(prompt).toContain("[truncated]");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/triage/prompt.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/triage/prompt.ts`:

```typescript
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
${directive ? `<directive>${directive}</directive>\n` : ""}
<triage_rules>
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
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/triage/prompt.test.ts
```

Expected: PASS — 6 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/triage/prompt.ts server-ts/src/triage/prompt.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add triage prompt string builder

Assembles the Haiku prompt from soul/mood/budget/rules/event/recent-outreach.
Adds directive line when budget state is tight or suppressed.
Truncates large event payloads to keep the prompt under 2KB.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 15: Outreach audit appender

**Files:**
- Create: `server-ts/src/outreach/audit.ts`
- Create: `server-ts/src/outreach/audit.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/outreach/audit.test.ts`:

```typescript
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { appendOutreach, readRecentOutreach } from "./audit.js";
import type { OutreachEntry } from "../types.js";

function entry(id: string, ts: number, title = `event-${id}`): OutreachEntry {
  return {
    id,
    ts,
    channel: "push",
    title,
    urgency: "medium",
    delivered: true,
    dedup_suppressed: false,
  };
}

describe("appendOutreach + readRecentOutreach", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-outreach-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("returns an empty list when the file is absent", async () => {
    const entries = await readRecentOutreach(home, "alice", 10);
    expect(entries).toEqual([]);
  });

  it("appends entries as JSON lines", async () => {
    await appendOutreach(home, "alice", entry("a", 100));
    await appendOutreach(home, "alice", entry("b", 200));
    const entries = await readRecentOutreach(home, "alice", 10);
    expect(entries.map((e) => e.id)).toEqual(["a", "b"]);
  });

  it("returns only the last N entries, newest last", async () => {
    for (let i = 0; i < 5; i++) {
      await appendOutreach(home, "alice", entry(`e${i}`, 1000 + i));
    }
    const entries = await readRecentOutreach(home, "alice", 3);
    expect(entries.map((e) => e.id)).toEqual(["e2", "e3", "e4"]);
  });

  it("tolerates and skips malformed lines", async () => {
    // Write a broken line directly, then a good one through the API
    const { appendFile, mkdir } = await import("node:fs/promises");
    const dir = join(home, "instances", "alice");
    await mkdir(dir, { recursive: true });
    await appendFile(join(dir, "outreach.jsonl"), "{ not valid json\n");
    await appendOutreach(home, "alice", entry("good", 500));
    const entries = await readRecentOutreach(home, "alice", 10);
    expect(entries.map((e) => e.id)).toEqual(["good"]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/outreach/audit.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/outreach/audit.ts`:

```typescript
import { appendFile, mkdir, readFile } from "node:fs/promises";
import { dirname } from "node:path";
import { OutreachEntrySchema, type OutreachEntry } from "../types.js";
import { outreachFile } from "../paths.js";

export async function appendOutreach(
  home: string,
  slug: string,
  entry: OutreachEntry,
): Promise<void> {
  const path = outreachFile(home, slug);
  await mkdir(dirname(path), { recursive: true });
  await appendFile(path, `${JSON.stringify(entry)}\n`);
}

/**
 * Read the last N outreach entries, oldest-first within the returned slice.
 * Returns [] if the file does not exist. Malformed lines are skipped silently.
 */
export async function readRecentOutreach(
  home: string,
  slug: string,
  limit: number,
): Promise<OutreachEntry[]> {
  const path = outreachFile(home, slug);
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw err;
  }

  const lines = raw.split("\n").filter((l) => l.length > 0);
  const start = Math.max(0, lines.length - limit);
  const slice = lines.slice(start);

  const result: OutreachEntry[] = [];
  for (const line of slice) {
    try {
      const parsed = JSON.parse(line);
      result.push(OutreachEntrySchema.parse(parsed));
    } catch {
      // malformed line — skip
    }
  }
  return result;
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/outreach/audit.test.ts
```

Expected: PASS — 4 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/outreach/audit.ts server-ts/src/outreach/audit.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add outreach.jsonl audit appender

Append-only per-instance log. Read helper returns the last N entries;
malformed lines are silently skipped so a bad write never corrupts reads.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 16: Settings.toml reader

**Files:**
- Create: `server-ts/src/settings/reader.ts`
- Create: `server-ts/src/settings/reader.test.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/settings/reader.test.ts`:

```typescript
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, loadSettings } from "./reader.js";

describe("loadSettings", () => {
  let home: string;

  beforeEach(async () => {
    home = await mkdtemp(join(tmpdir(), "bolly-settings-"));
  });

  afterEach(async () => {
    await rm(home, { recursive: true, force: true });
  });

  it("returns DEFAULT_SETTINGS when settings.toml is missing", async () => {
    const s = await loadSettings(home, "alice");
    expect(s).toEqual(DEFAULT_SETTINGS);
  });

  it("merges user-provided fields over defaults", async () => {
    const dir = join(home, "instances", "alice");
    await mkdir(dir, { recursive: true });
    await writeFile(
      join(dir, "settings.toml"),
      `daily_budget_usd = 5.0

[quiet_hours]
start = "23:00"
end = "08:00"

[push]
daily_max = 10
`,
    );
    const s = await loadSettings(home, "alice");
    expect(s.daily_budget_usd).toBe(5.0);
    expect(s.quiet_hours.start).toBe("23:00");
    expect(s.quiet_hours.end).toBe("08:00");
    expect(s.push.daily_max).toBe(10);
    // Defaults preserved for unspecified fields
    expect(s.push.enabled).toBe(true);
    expect(s.email.enabled).toBe(DEFAULT_SETTINGS.email.enabled);
  });

  it("DEFAULT_SETTINGS has a 2.00 daily budget", () => {
    expect(DEFAULT_SETTINGS.daily_budget_usd).toBe(2.0);
  });

  it("DEFAULT_SETTINGS has 22:00-07:00 quiet hours", () => {
    expect(DEFAULT_SETTINGS.quiet_hours.start).toBe("22:00");
    expect(DEFAULT_SETTINGS.quiet_hours.end).toBe("07:00");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/settings/reader.test.ts
```

Expected: FAIL. Module not found.

- [ ] **Step 3: Write minimal implementation**

Create `server-ts/src/settings/reader.ts`:

```typescript
import { z } from "zod";
import { settingsFile } from "../paths.js";
import { readToml } from "../toml-file.js";

const SettingsSchema = z.object({
  daily_budget_usd: z.number().positive().default(2.0),
  quiet_hours: z
    .object({
      start: z.string().default("22:00"),
      end: z.string().default("07:00"),
    })
    .default({}),
  push: z
    .object({
      enabled: z.boolean().default(true),
      daily_max: z.number().int().positive().default(5),
    })
    .default({}),
  email: z
    .object({
      enabled: z.boolean().default(true),
      address: z.string().optional(),
      daily_max: z.number().int().positive().default(2),
    })
    .default({}),
});

export type Settings = z.infer<typeof SettingsSchema>;

export const DEFAULT_SETTINGS: Settings = SettingsSchema.parse({});

/**
 * Read the instance's settings.toml; fall back to DEFAULT_SETTINGS if
 * the file does not exist. User-provided fields are merged over defaults
 * by zod's schema default propagation.
 */
export async function loadSettings(home: string, slug: string): Promise<Settings> {
  const result = await readToml(settingsFile(home, slug), SettingsSchema);
  return result ?? DEFAULT_SETTINGS;
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/settings/reader.test.ts
```

Expected: PASS — 4 tests green.

- [ ] **Step 5: Commit**

```bash
git add server-ts/src/settings/reader.ts server-ts/src/settings/reader.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): add settings.toml reader with defaults

$2 daily budget, 22:00-07:00 quiet hours, push + email enabled by default.
User-provided fields merge over defaults via zod schema propagation.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 17: Public API re-exports

**Files:**
- Modify: `server-ts/src/index.ts`

- [ ] **Step 1: Write the failing test**

Create `server-ts/src/index.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import * as api from "./index.js";

describe("public API", () => {
  it("re-exports the foundational modules", () => {
    expect(api.atomicWrite).toBeTypeOf("function");
    expect(api.readJson).toBeTypeOf("function");
    expect(api.writeJson).toBeTypeOf("function");
    expect(api.readToml).toBeTypeOf("function");
    expect(api.parseSkill).toBeTypeOf("function");
    expect(api.loadSkills).toBeTypeOf("function");
    expect(api.loadTriageRules).toBeTypeOf("function");
    expect(api.buildTriagePrompt).toBeTypeOf("function");
    expect(api.loadSettings).toBeTypeOf("function");
    expect(api.appendOutreach).toBeTypeOf("function");
    expect(api.readRecentOutreach).toBeTypeOf("function");
    expect(api.computeState).toBeTypeOf("function");
    expect(api.loadDaily).toBeTypeOf("function");
    expect(api.recordSpend).toBeTypeOf("function");
    expect(api.chargeAndCall).toBeTypeOf("function");
    expect(api.Throttle).toBeTypeOf("function");
    expect(api.todayUtc).toBeTypeOf("function");
  });

  it("re-exports path helpers", () => {
    expect(api.instanceDir).toBeTypeOf("function");
    expect(api.conversationFile).toBeTypeOf("function");
  });

  it("exposes DEFAULT_SETTINGS and DEFAULT_TRIAGE_TEMPLATE", () => {
    expect(api.DEFAULT_SETTINGS).toBeDefined();
    expect(api.DEFAULT_TRIAGE_TEMPLATE).toBeDefined();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm test src/index.test.ts
```

Expected: FAIL — exports missing on api.

- [ ] **Step 3: Update index.ts**

Replace `server-ts/src/index.ts`:

```typescript
export const VERSION = "1.0.0-alpha.0";

export * from "./types.js";
export * from "./paths.js";
export { atomicWrite } from "./fs-atomic.js";
export { readJson, writeJson } from "./json-file.js";
export { readToml } from "./toml-file.js";

export { parseSkill } from "./skills/parse.js";
export { loadSkills, type LoadSkillsOptions } from "./skills/loader.js";

export { loadTriageRules, DEFAULT_TRIAGE_TEMPLATE } from "./triage/rules.js";
export {
  buildTriagePrompt,
  type OutreachHint,
  type TriagePromptInputs,
} from "./triage/prompt.js";

export { loadSettings, DEFAULT_SETTINGS, type Settings } from "./settings/reader.js";

export { appendOutreach, readRecentOutreach } from "./outreach/audit.js";

export { computeState, TIGHT_THRESHOLD } from "./budget/state.js";
export {
  loadDaily,
  recordSpend,
  todayUtc,
  type SpendDelta,
} from "./budget/ledger.js";
export {
  chargeAndCall,
  type ChargeContext,
  type CallSuccess,
  type CallDowngraded,
  type CallOutcome,
} from "./budget/charge-and-call.js";
export { Throttle, type ThrottleConfig } from "./budget/throttle.js";
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm test src/index.test.ts
```

Expected: PASS — all 3 blocks green.

- [ ] **Step 5: Run the full suite**

```bash
pnpm test
```

Expected: all 17-task tests pass together (approximately 65+ tests).

- [ ] **Step 6: Commit**

```bash
git add server-ts/src/index.ts server-ts/src/index.test.ts
git commit -m "$(cat <<'EOF'
feat(server-ts): surface foundation modules via public API

index.ts re-exports the stable surface Plan 2 (Mind runtime) will consume:
types, paths, fs helpers, skills, triage, settings, outreach, budget.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 18: Typecheck and lint pass

- [ ] **Step 1: Run typecheck across the whole package**

```bash
cd server-ts && pnpm check
```

Expected: `tsc --noEmit` passes with no errors, `biome check` reports no problems.

If there are issues: fix them before continuing. Common issues:
- Missing `type` imports for zod inferred types → add `import type`
- `exactOptionalPropertyTypes` complaints → explicitly declare `| undefined`

- [ ] **Step 2: Run the full test suite once more**

```bash
pnpm test
```

Expected: all tests pass (green summary showing the total count).

- [ ] **Step 3: Commit any fixes (if step 1 required changes)**

```bash
git add -A
git commit -m "$(cat <<'EOF'
chore(server-ts): typecheck and lint cleanup

Final pass after all foundation modules land.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

If no fixes were needed: skip this commit.

- [ ] **Step 4: Tag the foundations milestone**

```bash
git tag -a foundations-complete -m "Plan 1 complete: server-ts foundation modules"
```

Do not push the tag yet; the user reviews and pushes in their own cadence.

---

## Self-Review Checklist

Before handing off, the executor should confirm:

- [ ] All 17 task files exist with the specified contents
- [ ] `pnpm test` reports all green with >60 tests
- [ ] `pnpm check` has no errors
- [ ] `git log --oneline` shows one commit per task (17 commits plus optional cleanup)
- [ ] `server-ts/src/index.ts` exports every foundation module
- [ ] No TODOs, TBDs, or incomplete sections in any `.ts` file

## What this plan does NOT do (Plan 2 picks it up)

- No Claude Agent SDK integration
- No HTTP server, no WebSocket
- No event queue, triage execution, or scheduled jobs
- No mind runtime / session lifecycle
- No outreach delivery (push/email/digest) — only the audit log
- No cross-instance / shared directory

Plan 2 (Mind runtime, weeks 3-5) consumes this surface and produces the first runnable Bolly v1 server.
