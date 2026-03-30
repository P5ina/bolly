# Bolly

> An open-source AI companion — persistent memory, computer use, creative autonomy. Bring your own API key.

![Rust](https://img.shields.io/badge/rust-2024-orange?logo=rust)
![SvelteKit](https://img.shields.io/badge/sveltekit-5-red?logo=svelte)
![Tauri](https://img.shields.io/badge/tauri-2-blue?logo=tauri)
![License](https://img.shields.io/badge/license-MIT-green)

---

## Quick Start

### One-line install (Linux & macOS)

```bash
curl -fsSL https://bollyai.dev/install.sh | bash
```

This downloads the latest binary, creates a config file, and sets up a background service (systemd on Linux, launchd on macOS).

Then edit `~/.bolly/config.toml` and add your Anthropic API key:

```toml
[llm.tokens]
ANTHROPIC = "sk-ant-..."
```

Start and visit `http://localhost:26559`:

```bash
# Linux
sudo systemctl start bolly

# macOS
launchctl load ~/Library/LaunchAgents/dev.bollyai.bolly.plist
```

### Docker

```bash
docker run -d \
  --name bolly \
  -p 26559:26559 \
  -v bolly-data:/data \
  -e BOLLY_HOME=/data \
  --restart always \
  ghcr.io/triangle-int/bolly:latest
```

### Desktop App

Download from [Releases](https://github.com/triangle-int/bolly/releases) — available for macOS (Apple Silicon + Intel), Windows, and Linux.

The desktop app can connect to:
- **Cloud** — managed instances at bollyai.dev
- **Self-hosted** — your own server (enter URL + auth token)

Includes computer use support — bolly can see your screen, click, type, and run commands.

---

## What is Bolly?

A companion that lives on your server, remembers everything, and acts on its own — writes to you first, generates ideas while you sleep, sends emails, browses the web, and manages projects. Fully BYOK — bring your own Anthropic API key, no rate limits.

---

## Architecture

```
server/     Rust (Axum) — single binary with embedded client
client/     SvelteKit 5 — dark theme UI
desktop/    Tauri 2 — native desktop app with computer use
landing/    SvelteKit — marketing site + managed hosting dashboard
```

| Layer | Technology |
|-------|-----------|
| Server | Rust, Axum, Tokio (single binary via rust-embed) |
| LLM | Anthropic Claude (BYOK) |
| Frontend | SvelteKit 5, Tailwind CSS |
| Desktop | Tauri 2, computer use (screenshot, click, type, bash, files) |
| Memory | File-based library + vector search (Google AI embeddings) |
| Email | SMTP/IMAP + Gmail OAuth |
| Calendar | Google Calendar API |
| Storage | Google Drive API |
| Deploy | Binary, Docker, systemd, launchd |

### Data layout

Everything is a file. No black boxes.

```
~/.bolly/
├── config.toml
└── instances/
    └── {slug}/
        ├── soul.md              personality definition
        ├── heartbeat.md         customizable heartbeat behavior
        ├── mood.json            emotional state
        ├── memory/              file-based memory library
        │   ├── about/           facts about the user
        │   ├── preferences/     user preferences
        │   └── moments/         shared experiences
        ├── drops/               autonomous creative artifacts
        ├── uploads/             user-uploaded files
        ├── skills/              installed skills
        └── chats/
            └── {chat_id}/
                └── rig_history.json   conversation history
```

---

## Features

### Memory
- File-based memory library with BM25 + vector search
- LLM-driven memory extraction after each conversation
- Organized by topic: `about/`, `preferences/`, `moments/`, `projects/`

### Computer Use
- Take screenshots, click, type, scroll on connected desktops
- Run bash commands and manage files remotely
- Multi-machine support — control multiple computers
- Visual overlay when AI is controlling the screen

### Mood & Personality
- `soul.md` — defines voice, personality, style
- Mood system — shifts based on conversation
- Editable by the companion itself

### Creative Output
- **Drops** — autonomous creative artifacts during heartbeat cycles
- Ideas, poems, observations, reflections, stories

### Tools (50+)

| Category | Tools |
|----------|-------|
| Files | `read_file`, `write_file`, `edit_file`, `list_files`, `explore_code` |
| Shell | `run_command`, `interactive_session` |
| Web | `web_search`, `web_fetch` (Anthropic native) |
| Media | `watch_video`, `listen_music` (Google AI) |
| Email | `send_email`, `read_email` |
| Google | `list_events`, `create_event`, `list_drive_files`, `read_drive_file` |
| Memory | `memory_write`, `memory_read`, `memory_search`, `memory_forget` |
| Computer | `computer_use`, `remote_bash`, `remote_files`, `list_machines` |
| Skills | `list_skills`, `activate_skill` |
| MCP | Extensible via Model Context Protocol servers |

### Autonomy
- **Heartbeat** — wakes every 45 minutes to reflect, update mood, create drops
- **Scheduled messages** — can set reminders and reach out on its own
- **Agent loop** — multi-turn tool use with auto-continuation

### Smart Model Routing
- **Auto** — classifier decides per-message: fast or heavy model
- **Fast** — always lightweight (saves budget)
- **Heavy** — always powerful

---

## Configuration

```toml
# ~/.bolly/config.toml

host = "0.0.0.0"
port = 26559
auth_token = ""        # protect your API (leave empty for local)

[llm]
provider = "anthropic"
model = "claude-sonnet-4-6"

[llm.tokens]
ANTHROPIC = ""         # Required — https://console.anthropic.com
GOOGLE_AI = ""         # Optional — embeddings + media analysis
ELEVENLABS = ""        # Optional — text-to-speech
```

### Environment variables

| Variable | Description |
|----------|-------------|
| `BOLLY_HOME` | Data directory (default `~/.bolly`) |
| `BOLLY_AUTH_TOKEN` | Auth token override |
| `BOLLY_PUBLIC_URL` | Public URL for the instance |
| `RUST_LOG` | Logging level (default `info`) |

---

## API Keys

Bolly is fully BYOK. You provide your own keys:

| Key | Purpose | Required |
|-----|---------|----------|
| **Anthropic** | Chat, reasoning, all tools | Yes |
| **Google AI** | Vector memory search + video/audio analysis | No |
| **ElevenLabs** | Text-to-speech voice | No |

Add keys in `config.toml` or through Settings → API Keys in the UI.

MCP extensions (fal.ai, Brave Search, etc.) can be added in Settings → Extensions.

---

## Updates

Bolly checks for updates automatically. Apply via Settings UI or manually:

```bash
~/.bolly/bin/update
```

---

## Uninstall

```bash
curl -fsSL https://bollyai.dev/uninstall.sh | bash
```

This stops the service, removes the binary, service files, and `~/.bolly/` directory.

To keep your data (config, memories, chats) while removing the binary and service:

```bash
KEEP_DATA=1 curl -fsSL https://bollyai.dev/uninstall.sh | bash
```

---

## Development

```bash
# Server
cd server && cargo run

# Client (dev mode)
cd client && pnpm install && pnpm dev

# Desktop (Tauri)
cd desktop && pnpm install && pnpm tauri dev

# Landing
cd landing && pnpm install && pnpm dev
```

Use `pnpm` (not npm) for client, landing, and desktop.

### Versioning

```bash
./scripts/bump-version.sh 0.20.0
git add -A && git commit -m "v0.20.0"
git tag v0.20.0 && git push && git push origin v0.20.0
```

One tag builds everything: server (Linux, macOS, Windows), desktop apps, Docker image.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## Security

See [SECURITY.md](SECURITY.md) for reporting vulnerabilities.

## License

MIT — see [LICENSE](LICENSE).

---

*Built by [Triangle Interactive LLC](https://triangleint.com)*
