# Bolly

> A self-hosted AI companion platform — persistent memory, mood, creative output, and full system access. Runs as a single binary or managed SaaS.

![Rust](https://img.shields.io/badge/rust-2024-orange?logo=rust)
![SvelteKit](https://img.shields.io/badge/sveltekit-5-red?logo=svelte)
![License](https://img.shields.io/badge/license-proprietary-red)

---

## What is Bolly?

A companion that lives on your server, remembers everything, and acts on its own — writes to you first, generates ideas while you sleep, sends emails, browses the web, and manages projects. It has its own personality, mood, and creative energy.

---

## Architecture

```
server/     Rust (Axum) — single binary with embedded client
client/     SvelteKit 5 — dark theme, oklch colors
landing/    SvelteKit — marketing site, dashboard, Stripe billing (Vercel)
```

### Stack

| Layer | Technology |
|-------|-----------|
| Server | Rust, Axum, Tokio (single binary via rust-embed) |
| LLM | Anthropic (Claude), OpenAI, OpenRouter — smart model routing |
| Frontend | SvelteKit 5, Tailwind CSS |
| Memory | File-based library (BM25 search, LLM-driven extraction) |
| Email | lettre (SMTP), async-imap (IMAP), Gmail (OAuth) |
| Calendar | Google Calendar API |
| Storage | Google Drive API |
| Browser | Headless Chromium (companion+ plans) |
| Deploy | GitHub Releases (binary), Fly.io (managed), Docker |
| Landing | SvelteKit, Drizzle ORM, PostgreSQL, Stripe |

### Data layout

Everything is a file. No black boxes.

```
~/.bolly/
├── config.toml
└── instances/
    └── {slug}/
        ├── soul.md              personality definition
        ├── heartbeat.md         customizable heartbeat behavior
        ├── autonomy.md          autonomous action rules
        ├── mood.json            emotional state
        ├── instance.toml        per-instance config (github, etc.)
        ├── memory/              file-based memory library
        │   ├── about/           facts about the user
        │   ├── preferences/     user preferences
        │   └── moments/         shared experiences
        ├── drops/               autonomous creative artifacts (JSON)
        ├── stats/               daily usage stats (JSON)
        ├── uploads/             user-uploaded files
        ├── skills/
        │   └── {skill_id}/
        │       ├── SKILL.md     skill definition
        │       └── references/  bundled docs
        └── chats/
            └── {chat_id}/
                ├── rig_history.json   unified message history
                └── meta.json
```

---

## Features

### Memory
- File-based memory library with BM25 search
- LLM-driven memory extraction after each conversation
- Organized by topic: `about/`, `preferences/`, `moments/`, `projects/`
- Memory tools: `memory_write`, `memory_read`, `memory_search`, `memory_list`, `memory_forget`

### Mood & Personality
- `soul.md` — defines voice, personality, style (editable by the companion)
- Mood system — shifts based on conversation (calm, focused, playful, loving, warm, reflective, curious, excited, melancholy)
- Sentiment tracking on every message

### Creative Output
- **Drops** — autonomous creative artifacts during heartbeat cycles
- Ideas, poems, observations, reflections, stories
- Browsable gallery in the UI

### Tools (46+)

| Category | Tools |
|----------|-------|
| Files | `read_file`, `write_file`, `edit_file`, `list_files`, `search_code`, `explore_code` |
| Shell | `run_command`, `interactive_session` |
| Web | `web_search`, `web_fetch`, `browse` (headless Chromium), `view_image` |
| Media | `watch_video`, `listen_music` |
| Email | `send_email`, `read_email` (SMTP/IMAP + Gmail OAuth) |
| Google | `list_events`, `create_event`, `list_drive_files`, `read_drive_file`, `upload_drive_file` |
| Memory | `memory_write`, `memory_read`, `memory_search`, `memory_list`, `memory_forget` |
| Self | `edit_soul`, `get_settings`, `update_config`, `clear_context` |
| Creative | `create_drop`, `create_view`, `export_to_excalidraw` |
| Project | `schedule_message`, `deep_research` |
| GitHub | `github_clone`, `github_branch`, `github_commit_push`, `github_create_pr` (via `run_command`) |
| Skills | `list_skills`, `activate_skill`, `read_skill_reference` |
| Security | `request_secret` (masked input — never exposed in chat) |
| Profile | `export_profile`, `import_profile` |
| State | `save_checkpoint`, `read_checkpoint` |

### Autonomy
- **Heartbeat** — wakes every 45 minutes to reflect, update mood, create drops
- **Scheduled messages** — can set reminders and reach out on its own
- **Agent loop** — multi-turn tool use with auto-continuation
- **Customizable** — `heartbeat.md` and `autonomy.md` control autonomous behavior

### Smart Model Routing
- **Auto mode** — Haiku classifier decides per-message: fast (cheap) or heavy (powerful)
- **Fast mode** — always use lightweight model (saves budget)
- **Heavy mode** — always use powerful model (10x budget cost)
- Switchable via settings UI or chat input toggle (A/F/H button)

### Integrations
- **Google** — Gmail, Calendar, Drive (OAuth)
- **GitHub** — clone, branch, commit, PR (token-based)
- **Email** — SMTP/IMAP accounts
- **MCP** — extensible via Model Context Protocol servers
- **Skills** — installable from [registry](https://github.com/triangle-int/bolly-skills)

---

## Configuration

Config lives at `~/.bolly/config.toml` (or `/data/config.toml` in Docker).

```toml
host = "0.0.0.0"
port = 8080
auth_token = ""

[llm]
provider = "anthropic"    # or "openai", "openrouter"
model = "claude-sonnet-4-6"
model_mode = "auto"       # "auto", "fast", "heavy"

[llm.tokens]
ANTHROPIC = "sk-ant-..."
OPEN_AI = ""
OPENROUTER = ""
BRAVE_SEARCH = ""
```

### Environment variables

| Variable | Description |
|----------|-------------|
| `BOLLY_HOME` | Workspace directory (default `~/.bolly`) |
| `BOLLY_AUTH_TOKEN` | Auth token override |
| `BOLLY_PUBLIC_URL` | Public URL for the instance |
| `BOLLY_LLM_PROVIDER` | LLM provider override |
| `BOLLY_LLM_MODEL` | Model override |
| `BOLLY_MODEL_MODE` | Model routing mode override |
| `RUST_LOG` | Logging level (default `info`) |

---

## Deployment

### Self-hosted (binary)

```bash
curl -sSL https://raw.githubusercontent.com/triangle-int/bolly/main/scripts/install.sh | bash
```

Creates a systemd service. Updates via settings UI.

### Docker

```bash
docker run -d \
  -p 8080:8080 \
  -v bolly-data:/data \
  -e BOLLY_HOME=/data \
  ubuntu:24.04 /opt/bolly/scripts/entrypoint.sh
```

### Managed (bollyai.dev)

Fully managed instances on Fly.io with persistent storage, automatic updates, and Stripe billing.

---

## Auth

Set `auth_token` in config.toml to require Bearer token auth. WebSocket and file URLs use `?token=` query parameter. Leave empty for local use.

---

## Development

```bash
# Server
cd server && cargo run

# Client (dev mode)
cd client && pnpm install && pnpm dev

# Landing
cd landing && pnpm install && pnpm dev
```

Use `pnpm` (not npm) for client and landing.

### Versioning

```bash
./scripts/bump-version.sh 0.15.0
```

---

## License

Copyright (c) 2026 Bolly. All rights reserved. See [LICENSE](LICENSE).

---

*Built by [Triangle Interactive](https://triangleint.com)*
