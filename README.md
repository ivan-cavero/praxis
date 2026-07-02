# praxis — README

**Autonomous agents that iterate in a loop until the objective is complete.**

> **Stack:** Rust (nightly) · Tauri v2 · Vue 3 · SQLite · Qdrant (embedded) · Ractor
> **Distribution:** One binary. No Docker, no Postgres, no Redis.

---

## What It Is

praxis is a system where you define a **goal**, assign **agents** to it (each
with its own model, its own tools, its own role), and the agents **iterate in a
loop** — they design, implement, review, fix, test — until the goal is done.

It's not a single-response chat. It's an agent factory that doesn't stop until
the work is finished.

**See [VISION.md](./VISION.md) for the full north star.**

---

## The Three Modes of Use

### 1. CLI (local or VPS)

```bash
praxis init my-api                    # create a project
praxis run --goal "REST API with JWT" # loop until complete
praxis status                         # live status
```

One binary. Works the same on your machine and on a VPS. SQLite + Qdrant
embedded run inside the process.

### 2. VPS + Dashboard

```bash
praxis server   # API + WebSocket on :8080
```

Connect from the browser and see the dashboard: agent pipeline, token metrics,
health of each agent, live logs, mid-loop instruction injection.

### 3. Desktop App

A desktop app (Tauri) like OpenCode, but focused on autonomous agents in a loop.
Manage projects, agents, models, connections. **Connect to remote machines**
that have praxis installed — same dashboard, same control, local or remote.

---

## How the Loop Works

```
GOAL → Orchestrator activates agents
         │
         ▼
   ┌─────────────────────────────────┐
   │  ITERATION N                    │
   │  Architect → designs            │
   │  Coder     → implements         │
   │  Reviewer  → reviews (other model)│
   │  Security  → scans              │
   │  Tester    → runs tests         │
   │                                 │
   │  ¿Gates pass? ─ no → feedback ──┼─→ iteration N+1
   │       │ yes                     │
   │  ¿Goal achieved? ─ no ──────────┘
   │       │ yes
   └───────▼─────────────────────────┘
         COMPLETED
```

- **Iterates until complete** — doesn't stop at the first response.
- **Outcome-based completion** — stops when the result is verified, not when an
  action is executed.
- **Hard limits** — `max_iterations`, `session_ttl`, `phase_timeout`.
- **Cross-verification** — different models review each other.
- **Pathology detection** — stuck, oscillating, or destructive agents are
  detected and stopped.
- **Mid-loop injection** — inject instructions while it runs.
- **Checkpointing** — crash recovery built-in.
- **Cross-session memory** — vector DB, the agent remembers.

---

## Agents

| Role | Responsibility |
|------|----------------|
| **Orchestrator** | Supervisor, routing, consensus, state machine |
| **Architect** | Design, ADRs, technical decisions |
| **Coder** | Code generation, compilation, iteration |
| **Reviewer** | Code review, quality, edge cases |
| **Security** | Vulnerabilities, secrets, unsafe |
| **Tester** | Tests, execution, coverage |
| **Git** | Commit, branch, push, PR |
| **Researcher** | Web search, docs, synthesis |

Each role is configurable in `forge.toml`: model, temperature, system prompt,
tools. You can define new roles.

---

## Configuration

Everything in `forge.toml`:

```toml
[project]
name = "my-api"

[providers.openai]
base_url = "https://api.openai.com/v1"
api_key = "env:OPENAI_API_KEY"
default_model = "gpt-5"

[providers.anthropic]
base_url = "https://api.anthropic.com"
api_key = "env:ANTHROPIC_API_KEY"
default_model = "claude-4-opus"

[roles.coder]
model = "gpt-5"
temperature = 0.3
system_prompt = "You are an expert Rust engineer..."

[roles.reviewer]
model = "claude-4-opus"
temperature = 0.2

[[goals]]
name = "full-feature"
agents = ["architect", "coder", "reviewer", "security", "tester"]
gates = ["review.pass", "security.no_critical", "test.pass"]
max_iterations = 10
```

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                    BINARY praxis                     │
│                                                       │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐  │
│  │   Core        │  │   MCP Host   │  │  HTTP/WS   │  │
│  │  (ractor)     │  │  (tokio)     │  │  (axum)    │  │
│  │               │  │              │  │            │  │
│  │ Orchestrator  │  │ FileSystem   │  │ REST API   │  │
│  │ Agents        │  │ Git          │  │ WebSocket  │  │
│  │ State Machine │  │ WebSearch    │  │ Auth (JWT) │  │
│  │ Loop Ctrl     │  │ GitHub       │  │            │  │
│  │ DriftGuard    │  │ Custom MCPs  │  │            │  │
│  │ Completion    │  │              │  │            │  │
│  │ Pathology     │  │              │  │            │  │
│  └──────┬───────┘  └──────┬───────┘  └─────┬──────┘  │
│         └──────────┬──────┴─────────────────┘        │
│                    ▼                                  │
│  ┌─────────────────────────────────────────────────┐ │
│  │  SQLite (event store) · Qdrant (vector) ·       │ │
│  │  moka (cache) · DashMap (hot state)             │ │
│  └─────────────────────────────────────────────────┘ │
│                                                       │
│  ┌─────────────────────────────────────────────────┐ │
│  │  Tauri Desktop (Vue 3) — local + remote         │ │
│  └─────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

One binary. No external services.

---

## Workspace

```
praxis/
├── crates/
│   ├── shared/           # Types, protocol, config, error
│   ├── agent-traits/     # Traits: LLMProvider, Tool, Memory, Persistence
│   ├── core/             # Runtime: ractor actors, state machine, orchestrator
│   ├── providers/        # LLM: OpenAI, Anthropic, Gemini, Ollama
│   ├── mcp-host/         # MCP client: discovery, protocol, tool registry
│   ├── memory/           # Hot (DashMap), Episodic (Qdrant), Context manager
│   ├── persistence/      # Event store: SQLite
│   ├── vault/            # Credential management: keyring, tauri, env
│   └── cli/              # CLI binary: clap + ratatui
├── dashboard/            # Vue 3 frontend (Vite + TS + Tailwind)
├── desktop/              # Tauri v2 binary
├── mcp-servers/          # First-party MCP servers
├── tests/                # Integration tests
└── docs/                 # ADRs + guides
```

### Dependency rule (inward only)

```
[providers, mcp-host, memory, persistence, vault]  ← depend on traits + shared
                    ↓
              [agent-traits]                        ← depends on shared
                    ↓
                  [shared]                          ← depends on nothing internal
                    ↑
                  [core]                            ← orchestrates everything
                    ↑
              [cli, desktop]                        ← binaries
```

---

## Commands

```bash
# Core
praxis init <name>              # create project
praxis run --goal "..."         # execute loop
praxis run --resume             # resume session
praxis run --dry-run            # see plan without executing
praxis run --headless           # JSON for CI/CD
praxis status                   # session status
praxis inject --session <id> --agent coder --message "use thiserror"

# Management
praxis project list/show/archive
praxis session list/show/stop/logs
praxis provider list/add/test
praxis mcp list/add/remove/test
praxis context inspect/history/force-compress

# Deploy
praxis server                   # API + WebSocket + dashboard
praxis desktop                  # desktop app
praxis deploy setup/push/status/logs  # VPS

# System
praxis version
praxis update
praxis test                     # integration test
```

---

## Development

```bash
# Rust
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# Dashboard (bun)
cd dashboard && bun install
cd dashboard && bun dev          # :3000, proxy to :8080
cd dashboard && bun run build
```

---

## Status

Phase 1 (the loop works) in progress. See [ROADMAP.md](./ROADMAP.md).

---

## License

Proprietary. Open-source planned for the future.
