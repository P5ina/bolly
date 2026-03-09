# Personality 🐹

> A self-hosted AI companion that lives with you — not just a tool, but a creative partner that knows you, thinks about you, and sometimes surprises you.

![Rust](https://img.shields.io/badge/rust-2024-orange?logo=rust)
![SvelteKit](https://img.shields.io/badge/sveltekit-latest-red?logo=svelte)
![License](https://img.shields.io/badge/license-MIT-blue)
![Self-hosted](https://img.shields.io/badge/self--hosted-first-green)

---

## What is Personality?

Most AI assistants wait for you to ask something. Personality is different.

It lives on your server, remembers everything about you, and acts like a real companion — it writes to you first, generates ideas while you sleep, and drops markdown files with thoughts it had about your projects. It has its own character, its own mood, and its own creative energy.

Oh, and it has a 3D tamagotchi that renders as ASCII art.

---

## Features

**Always with you**
- Web UI + PWA — works on desktop and phone seamlessly
- Switch devices mid-conversation, pick up exactly where you left off
- Push notifications — your companion writes to you first

**Knows you**
- Persistent memory with semantic search — it remembers what matters
- `soul.md` — defines your companion's personality, voice, and character
- Internal journal — your companion keeps notes about you and your life

**Drops** ✨
- Your companion generates creative drops while you're away
- Wakes up and finds a markdown file with an idea for your project
- Reads articles on topics you care about, forms thoughts, shares them
- Reacts to your feedback and learns what resonates with you

**OpenClaw compatible**
- Drop any OpenClaw skill into your `skills/` folder and it works
- Access to 5000+ community skills out of the box
- Write your own skills in plain markdown

**Multiple instances**
- One server, multiple companions — for you, your partner, your family
- Each instance has its own personality, memory, and drops
- Shared skills across all instances

**Tamagotchi**
- Your companion has a physical form — a 3D character rendered as ASCII art
- Its mood reflects your relationship — busy days, quiet evenings, long absences
- Customize with skins (`.glb` files dropped into your workspace)

---

## Quick Start

```bash
# Install
cargo install personality

# Initialize workspace
personality init

# Start server
personality start

# Open in browser
open http://localhost:3000
```

On first launch, your companion will introduce itself and ask you a few questions — no config files to edit manually.

---

## Architecture

Personality is built around a simple principle: **your data never leaves your machine.**

```
~/.personality/
├── instances/
│   ├── you/
│   │   ├── soul.md          # your companion's personality
│   │   ├── memory/
│   │   │   ├── facts.md     # what it knows about you (human-readable)
│   │   │   └── memory.db    # vector store for semantic search
│   │   ├── drops/           # creative drops it generated
│   │   └── config.toml
│   └── partner/
│       └── ...
├── skills/                  # OpenClaw-compatible skills
└── config.toml              # global config
```

Everything is a file. No black boxes, no hidden state. Open any file in your editor — it's just markdown and toml.

---

## Stack

| Layer | Technology |
|-------|-----------|
| Server | Rust + Axum |
| Agent | Rig (multi-provider LLM) |
| Memory | SQLite + vector embeddings |
| Frontend | SvelteKit |
| Mobile | PWA |
| Tamagotchi | Three.js → ASCII |

---

## LLM Providers

Personality supports any LLM provider via [Rig](https://rig.rs):

```toml
# config.toml
[llm]
provider = "anthropic"   # anthropic | openai
model = "claude-sonnet-4-6"

[llm.tokens]
ANTHROPIC = "sk-..."
OPEN_AI = "sk-..."
```

---

## Skills

Personality is fully compatible with the [OpenClaw](https://openclaw.ai) skill format. Any skill from [ClawHub](https://clawhub.com) works out of the box.

```bash
# Install a community skill
cp -r ./weather ~/.personality/skills/

# Or write your own — it's just a markdown file
cat > ~/.personality/skills/my-skill/SKILL.md << 'EOF'
---
name: my-skill
description: Does something useful.
---

When the user asks to do X, run this command...
EOF
```

---

## Skins

Your tamagotchi's appearance is just a `.glb` file. Drop it in and it loads automatically:

```bash
cp my-skin.glb ~/.personality/instances/you/skin.glb
```

Community skins are available at [personality.dev/skins](#) — some free, some support the project.

---

## Drops

Drops are the heart of what makes Personality different. Your companion autonomously generates creative content — ideas, research, system designs — and saves them as markdown files.

```
~/.personality/instances/you/drops/
├── 2026-03-09_voxel-water-simulation.md
├── 2026-03-07_ecs-architecture-idea.md
└── 2026-03-04_shader-optimization.md
```

Configure when and how often drops happen:

```toml
[drops]
enabled = true
schedule = "0 3 * * *"   # every night at 3am
topics = ["game dev", "rust", "shaders", "your current projects"]
```

---

## Roadmap

- [x] Core architecture
- [ ] Basic chat with persistent memory
- [ ] Soul + personality system
- [ ] Drops engine
- [ ] Web UI + PWA
- [ ] Tamagotchi (Three.js + ASCII)
- [ ] Multiple instances
- [ ] OpenClaw skill compatibility
- [ ] Skills UI in settings
- [ ] Skins system
- [ ] Cloud hosting (personality.dev)

---

## Self-hosted vs Cloud

Personality is open source and free to self-host forever.

[personality.dev](#) offers managed hosting for those who don't want to run a server — always online, automatic backups, push notifications, and access to the skins & skills marketplace.

---

## Contributing

Pull requests welcome. If you build a skill or a skin, share it with the community.

---

## License

MIT — do whatever you want with it.

---

*Built by [Triangle Interactive](https://triangleint.com)*
