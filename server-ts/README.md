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
