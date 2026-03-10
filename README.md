# Bolly

> A self-hosted AI companion that lives on your server — not a chatbot, but a persistent being with memory, mood, creative output, and full system access.

![Rust](https://img.shields.io/badge/rust-2024-orange?logo=rust)
![SvelteKit](https://img.shields.io/badge/sveltekit-5-red?logo=svelte)
![License](https://img.shields.io/badge/license-MIT-blue)
![Docker](https://img.shields.io/docker/v/p5ina/bolly?sort=semver&label=docker)

---

## What is Bolly?

Most AI assistants wait for you to ask something. Bolly is different.

It lives on your server, remembers everything about you, and acts like a real companion — it writes to you first, generates ideas while you sleep, and drops creative artifacts with thoughts it had about your projects. It has its own character, mood, and creative energy.

It also has full system access — it can read and write files, run commands, install packages, send emails, and see images and documents you share.

---

## Quick Start

### Docker

```bash
docker run -d \
  -p 8080:8080 \
  -v bolly-data:/data \
  p5ina/bolly:latest
```

Open `http://localhost:8080`, configure your LLM provider, and create your first companion.

### From Source

```bash
# Server
cd server && cargo run

# Client (dev mode, in another terminal)
cd client && pnpm install && pnpm dev
```

The dev server proxies API requests to `localhost:8080`.

---

## Features

### Knows you
- **Semantic memory** — extracts and recalls facts from every conversation (SQLite + vector embeddings)
- **soul.md** — defines your companion's personality, voice, and character (editable by the companion itself)
- **Journal** — private daily reflections your companion keeps for continuity
- **Context compaction** — automatically summarizes older messages to stay within context limits without losing history

### Feels
- **Mood system** — shifts naturally based on conversation context (calm, focused, playful, loving, warm, reflective, curious, excited, melancholy)
- **Sentiment tracking** — reads your emotional state and responds to it
- **Living blob** — 3D tamagotchi rendered as ASCII art, visual mood indicator

### Creates
- **Drops** — autonomous creative artifacts generated during heartbeat cycles
- Ideas, poems, observations, reflections, stories — whatever comes naturally
- Browsable gallery in the UI, real-time WebSocket notifications

### Sees
- **Image uploads** — attach images directly in chat, analyzed with vision (Anthropic/OpenAI multimodal)
- **PDF documents** — send PDFs for extraction and understanding
- **Code & text files** — attach any text-based file (`.py`, `.rs`, `.json`, etc.) — inlined automatically
- No tool calls needed — your companion sees attachments immediately in the conversation

### Acts
- **26 tools** with full system access:

| Category | Tools |
|----------|-------|
| Filesystem | `read_file`, `write_file`, `list_files`, `search_code` |
| Shell | `run_command` |
| System | `install_package` (auto-detects apt/dnf/brew/pacman/apk) |
| Email | `send_email` (SMTP), `read_email` (IMAP) |
| Memory | `remember`, `recall` |
| Self | `edit_soul`, `set_mood`, `get_mood`, `journal`, `read_journal` |
| Creative | `create_drop` |
| Project | `get_project_state`, `update_project_state`, `create_task`, `update_task`, `list_tasks` |
| Chat | `clear_context`, `schedule_message` |
| Other | `web_search`, `current_time`, `update_config` |

### Thinks autonomously
- **Heartbeat** — wakes every 45 minutes to reflect, journal, update mood, and create drops
- **Customizable heartbeat** — edit `heartbeat.md` to control what happens between conversations (the companion can edit this itself)
- **Agent loop** — multi-turn tool use with up to 20 iterations per request, 8 internal sub-turns each
- **Auto-continuation** — detects when a task isn't done and keeps going
- **Scheduled messages** — can set reminders and reach out on its own

### Multiple companions
- One server, multiple instances — each with its own soul, memory, mood, and drops
- Independent heartbeats, separate file storage, individual chat histories

---

## Configuration

Config lives at `~/.bolly/config.toml` (or `/data/config.toml` in Docker).

```toml
host = "0.0.0.0"
port = 8080
auth_token = ""          # set a token to protect your API
static_dir = ""          # path to built client files (set automatically in Docker)

[llm]
provider = "anthropic"   # or "openai"
model = "claude-sonnet-4-6"

[llm.tokens]
ANTHROPIC = "sk-ant-..."
OPEN_AI = ""
BRAVE_SEARCH = ""        # for web_search tool

[email]
smtp_host = "smtp.gmail.com"
smtp_port = 587
smtp_user = ""
smtp_password = ""
smtp_from = ""
imap_host = "imap.gmail.com"
imap_port = 993
imap_user = ""
imap_password = ""
```

LLM provider and API key can also be configured through the web UI on first launch.

### Environment variables

- `BOLLY_HOME` — override workspace directory (default `~/.bolly`)
- `RUST_LOG` — logging level (default `info`)

---

## Architecture

```
server/     Rust (Axum) — API, WebSocket, LLM integration, tools, heartbeat
client/     SvelteKit 5 — static SPA, dark organic theme
```

### Data layout

Everything is a file. No black boxes.

```
~/.bolly/
├── config.toml
├── instances/
│   └── {slug}/
│       ├── soul.md              personality definition
│       ├── heartbeat.md         customizable heartbeat behavior
│       ├── mood.json            emotional state
│       ├── project_state.json   companion's self-managed context
│       ├── tasks.json           task board
│       ├── memory/
│       │   ├── facts.md         human-readable memory
│       │   └── memory.db        vector store (sqlite-vec)
│       ├── journal/             daily reflections (YYYY-MM-DD.md)
│       ├── drops/               creative artifacts (JSON)
│       ├── uploads/             user-uploaded files + metadata
│       └── chats/
│           └── {chat_id}/
│               ├── messages.json
│               ├── meta.json
│               └── compact.md   compressed older context
└── skills/
```

### Stack

| Layer | Technology |
|-------|-----------|
| Server | Rust, Axum, Tokio |
| LLM | Rig (Anthropic + OpenAI) |
| Vision | Multimodal API (images, PDFs, documents) |
| Memory | SQLite + sqlite-vec embeddings |
| Frontend | SvelteKit 5, Tailwind CSS |
| 3D | Three.js → ASCII rendering |
| Email | lettre (SMTP), async-imap (IMAP) |
| Deploy | Docker (multi-arch: amd64 + arm64) |

### Real-time events

WebSocket at `/api/ws` broadcasts:
- `chat_message_created` — new message
- `mood_updated` — mood change
- `agent_running` / `agent_stopped` — thinking state
- `tool_activity` — tool execution with summary
- `drop_created` — new autonomous drop
- `instance_discovered` — new companion found

---

## Auth

Set `auth_token` in config.toml to require a Bearer token on all API requests. The web UI prompts for the token on first visit. WebSocket connections and file URLs pass the token as `?token=` query parameter.

Leave `auth_token` empty to disable auth (fine for local use).

---

## Deployment

### Docker Compose

```yaml
services:
  bolly:
    image: p5ina/bolly:latest
    ports:
      - "8080:8080"
    volumes:
      - bolly-data:/data
    environment:
      - RUST_LOG=info
    restart: unless-stopped

volumes:
  bolly-data:
```

### Behind a reverse proxy (Caddy)

```
companion.example.com {
    reverse_proxy localhost:8080
}
```

Set `auth_token` when exposing to the internet.

---

## API

All endpoints require `Authorization: Bearer {token}` when `auth_token` is configured.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/meta` | Server info (version, commit, LLM status) |
| `GET` | `/api/instances` | List companions |
| `POST` | `/api/chat` | Send message (starts agent loop) |
| `GET` | `/api/chat/{slug}/chats` | List chat threads |
| `GET` | `/api/chat/{slug}/{chat_id}/messages` | Get messages |
| `POST` | `/api/chat/{slug}/{chat_id}/stop` | Stop agent |
| `DELETE` | `/api/chat/{slug}/{chat_id}/context` | Clear context |
| `GET/PUT` | `/api/instances/{slug}/soul` | Read/update personality |
| `GET` | `/api/instances/{slug}/mood` | Get mood state |
| `GET/PUT` | `/api/instances/{slug}/companion-name` | Get/set name |
| `GET` | `/api/instances/{slug}/drops` | List drops |
| `POST` | `/api/instances/{slug}/uploads` | Upload file (multipart) |
| `GET` | `/api/instances/{slug}/uploads/{id}/file` | Serve uploaded file |
| `PUT` | `/api/config/llm` | Update LLM configuration |
| `GET` | `/api/ws` | WebSocket (real-time events) |

---

## Roadmap

- [x] Core chat with persistent history
- [x] Soul + personality system with self-editing
- [x] Semantic memory (extract, store, recall)
- [x] Mood system with visual feedback
- [x] Heartbeat — autonomous reflection and journaling
- [x] Customizable heartbeat prompt (heartbeat.md)
- [x] 26 LLM tools (filesystem, shell, memory, project management)
- [x] Multi-chat support per instance
- [x] Streaming activity UI (real-time tool visibility)
- [x] Drops engine (autonomous creative output)
- [x] Email tools (SMTP send, IMAP read)
- [x] System package installation
- [x] File uploads with vision (images, PDFs, code files)
- [x] Context auto-compaction (infinite conversation history)
- [x] Auth (Bearer token)
- [x] Docker deployment (multi-arch)
- [x] Static file serving from Axum
- [x] Version + commit hash display
- [ ] PWA + push notifications
- [ ] Tamagotchi polish (richer mood-driven visuals)
- [ ] Skins system (.glb custom models)
- [ ] Skills marketplace (OpenClaw compatible)

---

## Contributing

Pull requests welcome.

---

## License

MIT

---

*Built by [Triangle Interactive](https://triangleint.com)*
