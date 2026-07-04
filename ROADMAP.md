# praxis — ROADMAP

> Focus: **the loop works → VPS + dashboard → desktop + remote → memory → production.**
> No enterprise. No multi-tenant. No billing. See [VISION.md](./VISION.md).

---

## Legend

- `[x]` done · `[~]` in progress · `[ ]` pending
- 🧪 validation milestone

---

## Current State

What ALREADY exists and compiles (291 tests pass):

| Component | Status |
|---|---|
| Workspace + nightly toolchain | `[x]` |
| Shared types, protocol, config, error | `[x]` |
| Actor model (ractor): Supervisor, EchoAgent | `[x]` |
| EventBus (broadcast) | `[x]` |
| State machine + LoopController | `[x]` |
| DriftGuard (metrics + ASI) | `[x]` |
| LLM providers: OpenAI, Anthropic, Gemini, Ollama, Mock | `[x]` |
| ProviderRouter + ModelTier | `[x]` |
| SQLite event store + snapshots | `[x]` |
| MCP host (stdio transport, tool registry) | `[x]` |
| Hot memory (DashMap) + LLM cache (moka) | `[x]` |
| ContextManager (budget, profiles, compression) | `[x]` |
| Vault (credentials, keyring/env) | `[x]` |
| API server (axum: REST + WebSocket + JWT auth) | `[x]` |
| CLI (clap): init, run, status, server, test... | `[~]` (many commands are stubs) |
| Desktop (Tauri v2) | `[x]` (tray, updater, embedded core) |
| Dashboard (Vue 3) | `[x]` (full: 5 views, 6 components, router, WS, Pinia) |

---

## Phase 1 — The Loop Works

**Goal:** `praxis run --goal "..."` executes agents that **iterate in a loop**
until the goal is complete or a hard limit is reached. With real LLM, real
gates, real checkpointing.

### 1.1 Real Iteration Loop

- [x] **`run_goal` iterates for real** — was passing through phases once and
  stopping. Now loops: `Planning → Implementing → Reviewing → Fixing → ...`
  until gates pass AND goal is complete, or `max_iterations`.
- [x] **Agents are async** — `BaseAgent::execute` is now `async fn`, calls the
  LLM provider directly (no more `call_llm_sync` that returned mock).
- [x] **Feedback loop** — when a gate fails, feedback is consolidated and passed
  to the coder for the next iteration.
- [x] **Default roles** — when no `forge.toml` exists, default mock roles are
  used so the pipeline runs end-to-end.
- [x] **Goal completion check** — after each iteration, the `CompletionCriterion`
  evaluates whether the goal is actually achieved (outcome-based, not
  action-based). See 1.3.
- [x] **Checkpointing per iteration** — persist state to SQLite after each
  phase transition. `--resume` loads the last checkpoint.
- [x] **Graceful shutdown** — Ctrl+C → finish current iteration → checkpoint →
  flush → exit.

### 1.2 Real Agent Execution

- [x] **Agent.execute() calls the real LLM** — connected to `ProviderRouter`,
  calls `provider.chat()` directly.
- [x] **System prompt assembly** — system_prompt + role + task context.
- [x] **Tool execution** — agents call MCP tools (filesystem: read/write/edit)
  during `execute()`. Tool calls are parsed from ` ```tool JSON blocks`, executed
  via `McpHost`, and results are appended. If tools were called, the agent is
  **re-invoked with tool results** in the same iteration so the LLM can react
  to tool output. `ToolCalled` events published to EventBus.
- [x] **Streaming output** — `provider.stream()` → EventBus → CLI (**done**) +
  Dashboard (**new**). PipelineView and SessionView show real-time `AgentOutput`
  streaming text. `getAgentStream()` / `filterEvents()` helpers in useWebSocket.

### 1.3 Completion Criteria — Outcome-Based

**This is the core of loop engineering.** The loop must stop when the OUTCOME is
verified, not when an ACTION is executed.

- [x] **`CompletionCriterion` trait** — `verify(goal, results) -> OutcomeResult`
- [x] **`OutcomeResult` enum** — `Achieved { evidence }`, `NotAchieved { reason }`,
  `Exhausted { reason }` (give up, no more options)
- [x] **Default coding criterion** — verifies: all agents completed, reviewer
  approved, security clean, tests pass
- [x] **Evidence collection** — when achieved, collects proof (agent outputs)
- [x] **Exhaustion detection** — if no progress after N iterations, marks as
  `Exhausted` and stops
- [x] **Configurable per goal** — via CLI `--completion="coding"|"manual"|"stagnant=N"`

### 1.4 Loop Pathology Detection

An agent stuck in a loop can be destructive. praxis detects this.

- [x] **`LoopPathologyDetector`** — tracks action hashes, phase transitions,
  progress metrics
- [x] **Repetition detection** — same output/action N times → force context reset
- [x] **Oscillation detection** — A→B→A→B phase cycling → break the cycle
- [x] **No-progress detection** — N iterations without state change → pause agent
- [x] **Destructive behavior detection** — process kill/create patterns, file
  deletion loops → kill session immediately
- [x] **Integration into `run_goal`** — check after every iteration
- [x] **Token waste detection** — token usage growing without progress → throttle
- [x] **Cross-model verification** — when pathology suspected, ask a different
  model: "Is this agent making progress?" (via `verify_with_model()`)
- [x] **Streaming output** — `provider.stream()` → EventBus → CLI. Each agent
  publishes `AgentOutput` deltas as they arrive from the LLM; CLI displays them
  in real-time with `│` prefix lines.

### 1.5 Gates That Actually Block

- [x] **GateRegistry connected** — `run_goal` now evaluates gates on
  Reviewing/Testing/SecurityScan phases.
- [x] **ReviewGate** — `AllAgentsPass` evaluator.
- [x] **SecurityGate** — `NoCritical` evaluator.
- [x] **TestGate** — `AllAgentsPass` evaluator.
- [x] **Feedback loop** — gate fails → feedback → coder → fix → re-review.
- [x] **Gate retry limits** — `max_retries` per gate, escalates to `Failed`
  phase when exceeded.

### 1.6 CLI v1 — Commands That Work

- [x] **`init`** — creates project + forge.toml + SQLite.
- [~] **`run --goal`** — executes the real loop.
- [x] **`run --resume`** — resume from checkpoint (loads last session from
  SQLite, calls `resume_goal`).
- [x] **`run --dry-run`** — shows plan without executing.
- [x] **`run --headless`** — JSON output for CI/CD.
- [x] **`run --completion`** — choose completion criterion ("coding", "manual", "stagnant=N").
- [x] **`status`** — no separate command; `session list` and `session show <id>` read from SQLite.
- [x] **Streaming output** — EventBus events published in `run_goal`, CLI subscribes and shows agent start/complete/phase changes/gate results in real-time.
- [x] **Graceful shutdown** — Ctrl+C handler saves checkpoint and exits.
- [x] **`session list/show`** — reads sessions from SQLite event store.
- [~] **`session stop/logs`** — stubs (stop works via Ctrl+C; logs via EventBus).
- [~] **`inject`** — infrastructure in CoreRuntime (`inject()`, `drain_injections()`), CLI stub explains API server requirement.

🧪 **Milestone:** `praxis init demo && praxis run --goal "Create a hello world
in Rust"` → the loop iterates, the coder writes code, the reviewer approves,
tests pass, goal verified complete. With real LLM (or mock if no API key).

---

## Phase 2 — Real Agents + Tools

**Goal:** Multiple agents with different models, cross-verification, MCP tools,
mid-loop injection.

### 2.1 Agent Roles + Tool Integration

- [x] **6 agent types** — Architect, Coder, Reviewer, Security, Tester, Git.
- [x] **MCP servers connected** — `connect_mcp_servers()` called in `run_goal`.
- [x] **Tool context injected** — `prepare_tool_context()` adds available tool descriptions to each agent's task context.
- [x] **Tool calls executed** — `execute_tool_calls()` parses ` ```tool JSON blocks` from agent output, calls tools via `McpHost`, and appends results.
- [x] **Agent receives tool results** — tool output is appended to agent result so the LLM can see it in the next iteration.

### 2.2 Cross-Verification

- [x] **Parallel execution** — `tokio::JoinSet` for parallel reviewers in review phases.
- [x] **`parallel_reviewers` config** — `get_agents_for_phase` creates N reviewers with unique focus angles, configurable via `GoalConfig.parallel_reviewers`.
- [x] **ConsensusConsolidator** — implemented (AllPass, MajorityPass, Weighted, EscalateToBest) and integrated into the main loop.
- [x] **Cross-model feedback loop** — `CrossModelFeedbackLoop::generate_feedback` integrated into gate evaluation for multi-reviewer phases.

### 2.3 MCP Tools

- [x] **Filesystem MCP server** — read/write/edit/list/search/glob, path sandboxing.
- [x] **Git MCP server** — status/add/commit/log/diff/branch/checkout/push/pull (9 tools).
- [x] **GitHub MCP server** — create_pr/list_issues/create_issue/list_prs/list_branches/search_code (6 tools, gh CLI or direct API).
- [x] **WebSearch MCP server** — search (DuckDuckGo + Brave API) and extract (URL fetch + HTML strip).

### 2.4 Mid-Loop Injection

- [x] **InjectionChannel** — `inject()`/`drain_injections()` in `CoreRuntime`.
- [x] **Agent checks pending injections** — drained before each agent execution in `run_goal`.
- [x] **Injections have CRITICAL priority** — never compressed/dropped.
- [x] **`inject` CLI command** — writes to `{data_dir}/injections/`, runtime picks up on next iteration.
- [x] **Audit** — all injections are logged via `tracing::info!` on inject/drain/apply, plus `InjectionTriggered` event bus message.

🧪 **Milestone:** Goal → Architect designs → Coder implements → 2 Reviewers
(GPT-5 + Claude) review in parallel → consensus → Security scans → Tester runs
→ gate passes. Inject "use thiserror" mid-loop → coder applies it.

---

## Phase 3 — VPS + Dashboard

**Goal:** Install on VPS with one command. Modern dark mode web dashboard to
monitor and manage.

### 3.1 Easy VPS Installation

- [x] **Install script** — `curl -fsSL https://praxis.dev/install.sh | bash`.
  OS detection, download binary, verify checksum. (`scripts/install.sh`)
- [x] **systemd service** — `praxis server` as a service, auto-restart. (`deploy/praxis.service`)
- [x] **Config file** — `~/.config/praxis/config.toml` (port, data dir, auth). (`deploy/praxis-config.toml`)
- [x] **Single binary** — release build with LTO + strip (~15-20 MB). (already in Cargo.toml)

### 3.2 API Server (refine)

- [x] **REST endpoints** — projects, sessions, agents, metrics, context, inject.
  Sessions read from shared SQLite event store (cross-process).
- [~] **WebSocket** — real-time events (phase changes, token usage, drift,
  compression, injections). Handler exists, broadcast works same-process.
- [x] **Auth JWT** — first-run token, 24h expiry. Done.
- [x] **CORS** — configurable for remote dashboard. Done.

### 3.3 Vue 3 Dashboard — Modern Dark Mode

- [x] **Design system** — Tailwind 4, dark palette (zinc), CSS custom properties.
- [x] **DashboardView** — metric cards, sessions table, agent grid.
- [x] **PipelineView** — 7-phase flow visualization (CSS animated).
- [x] **SessionView** — session detail + SessionsView list.
- [x] **TokenChart** — token usage bar chart (CSS, no canvas).
- [x] **AgentHealth** — agent status table with status badges.
- [x] **ContextPanel** — pressure gauge (SVG arc), budget bar.
- [x] **InjectPanel** — target selector, type, message, send.
- [x] **ProjectConfig** — TOML editor with save support.
- [x] **useWebSocket** composable — auto-reconnect, backoff, event buffering.
- [x] **Responsive** — 3-col desktop, 2-col tablet, 1-col mobile. Sidebar collapses to icon-only at tablet, top nav bar at mobile. SessionsView has mobile card layout.

🧪 **Milestone:** `praxis server` on VPS → browser to `http://vps:8080` → modern
dark dashboard shows the loop live, tokens, agent health. Inject instruction
from the panel → reaches the agent.

---

## Phase 4 — Desktop + Remote

**Goal:** Desktop app (Tauri) that manages everything and connects to remote VPS.

### 4.1 Desktop App Base

- [x] **Tauri v2** — window 1280x800, title "praxis".
- [x] **Core embedded** — `CoreRuntime` initialized in setup.
- [x] **Same UI as dashboard** — the Vue dashboard served via Tauri WebView.
- [x] **System tray** — show/hide, new session, settings, quit. Left-click toggles window visibility. Close button hides instead of quitting.
- [x] **Auto-update** — `tauri-plugin-updater` registered. Endpoints configured for GitHub releases. Missing: pubkey (generate with `cargo tauri signer generate`), frontend JS integration.

### 4.2 Local Management

- [x] **Projects** — create, configure, archive, list. Full CRUD via REST API + store.
- [x] **Agents** — configure roles, models, system prompts, tools. AgentsConfig panel in Settings → Skills.
- [x] **Models/Providers** — add API keys via secure vault, test connections.
- [x] **Goals/Sessions** — create goals, launch, stop, watch live. GoalLaunch panel on DashboardView. Uses Tauri IPC in desktop mode.
- [x] **Config** — forge.toml editor with validation. ProjectConfig.vue with TOML text editor + AgentsConfig panel.

### 4.3 Remote Connection

- [x] **Pairing system (Fase A)** — QR-based device pairing. `POST /api/pair` generates code, browser confirmation page, JWT retrieval via `/api/pair/{code}/token`. In-memory codes with 5min TTL + background expiry.
- [x] **CLI --pair** — `praxis server --pair` enables pairing mode, prints ASCII QR in terminal.
- [x] **Device management** — devices stored in `{data_dir}/devices.json`, REST endpoints for list/revoke, JWT auth with "device" role.
- [x] **Frontend Remote Connections** — Settings tab with saved connections (localStorage), "Add Remote" modal with QR rendering + polling + JWT retrieval. Works on LAN.
- [x] **Connection toggle + API proxy** — header indicator (green dot + host) when remote, dropdown to switch local/remote, all API calls redirect to remote when in remote mode.
- [x] **Designed for Fase B** — PairingState, Device, JWT claims all extendable. When `praxis.dev` exists, QR will point to cloud relay instead of direct server URL.

#### 🌐 Fase B — praxis.dev Platform (FUTURE)

- [ ] **praxis.dev domain** — register domain, deploy cloud API + web app.
- [ ] **OAuth login** — Google/GitHub login via praxis.dev (not per-VPS). Users create ONE account.
- [ ] **Cloud account** — accounts table in cloud DB (PostgreSQL), JWT issued by praxis.dev.
- [ ] **Device registry** — VPS/desktops register themselves to the user's praxis.dev account.
- [ ] **Cloud relay pairing** — QR points to `app.praxis.dev/pair?code=X&host=...` instead of direct VPS URL.
- [ ] **Multi-device sync** — connections, config, and preferences synced across devices via praxis.dev.
- [ ] **Remote tunnel** — if VPS has no public IP, praxis.dev relays the connection (Tailscale-like).
- [ ] **Web dashboard** — `app.praxis.dev` web app to manage all your devices from anywhere.

🧪 **Milestone:** Desktop app open → manage local project → add remote VPS →
connect → see the remote VPS dashboard → launch a remote goal → monitor it live
from the desktop.

---

## Phase 5 — Memory + Self-Healing

**Goal:** The system remembers across sessions and auto-recovers from drift.

### 5.1 Episodic Memory (Qdrant)

- [ ] **Qdrant embedded** — `Qdrant::local(path)`, `embeddings` collection.
- [ ] **EmbeddingService** — wrap provider.embed(), batch, cache.
- [ ] **Chunking** — by size (512 tokens), by structure, by turn.
- [ ] **RAG** — before each agent call, search relevant chunks, inject top-K.
- [ ] **Dynamic K** — adjust K based on available budget.
- [ ] **MemoryKeeper actor** — background indexing every N interactions.

### 5.2 Consolidated Memory

- [ ] **SummarizerAgent** — structured summary of N interactions.
- [ ] **Consolidated memory** — summaries indexed in Qdrant, cross-session.
- [ ] **Cross-project memory** — search learnings from other projects.
- [ ] **TTL cleanup** — raw chunks: 30 days, summaries: forever.

### 5.3 Auto-Recovery

- [ ] **DriftGuard v2** — 8 ASI dimensions + context health.
- [ ] **Recovery actions** — LogOnly, ForceConsolidation, ContextReset,
  ModelUpgrade, PauseAgent, KillSession.
- [ ] **EMC** — emergency consolidation when pressure > 85%.
- [ ] **Model switching** — upgrade model if drift persists.
- [ ] **Session handoff** — new session with learnings if ASI < 30.

🧪 **Milestone:** Session 1 indexed. Session 2 asks "what did we decide?" → RAG
retrieves → agent references past decisions. Agent degrades → ASI drops →
auto-recovery (context reset or model upgrade) → stabilizes.

---

## Phase 6 — Production

**Goal:** Solid tests, docs, installer, release v1.0.

### 6.1 Tests

- [ ] **Unit tests > 80% coverage**.
- [ ] **Integration tests** — full multi-agent workflow with MockProvider.
- [ ] **Context stress test** — 10k interactions, 100 compressions, no leak.
- [ ] **Crash recovery test** — kill mid-execution, verify resume.
- [ ] **Benchmarks** — token throughput, latency per phase, memory (criterion).

### 6.2 Docs

- [ ] **Installation guide** — VPS, local, desktop.
- [ ] **Quickstart** — init → run → see result.
- [ ] **CLI reference** — all commands.
- [ ] **Configuration guide** — forge.toml, roles, goals, providers.
- [ ] **Architecture ADRs** — key decisions documented.
- [ ] **API docs** — OpenAPI spec + WebSocket protocol.

### 6.3 Release

- [ ] **Install script** — `curl | bash`, OS detection, checksum, GPG.
- [ ] **GitHub Actions release** — build all targets, test, upload.
- [ ] **Changelog** — from conventional commits.
- [ ] **Release v1.0**.

🧪 **Milestone:** `curl -fsSL https://praxis.dev/install.sh | bash` on a clean
VPS → `praxis init demo && praxis run --goal "..."` → works. Tests pass. Docs
complete. v1.0 released.

---

## Out of Scope (not built)

These features are **explicitly out** until the core works and is actually used.
If needed, they get reconsidered then:

- Multi-tenant / organizations / teams / users
- Billing / plans / SaaS
- SSO / SAML / OIDC enterprise
- RBAC with hierarchical roles
- Enterprise audit logs
- Webhooks
- Postgres / Redis / Docker Compose (we use SQLite + Qdrant embedded)
- Team-scoped API keys

**Reason:** praxis is a single-operator binary. If you need isolation, run
another instance. Enterprise complexity drowns the core. See
[VISION.md](./VISION.md).

---

## Stack

| Layer | Tech | Version |
|---|---|---|
| Language | Rust | nightly-2026-06-01 |
| Async runtime | tokio | 1.x |
| Actor framework | ractor | 0.15 |
| HTTP/WS | axum | 0.8 |
| SQLite | rusqlite + r2d2 | 0.39 |
| Vector DB | Qdrant (embedded) | 1.12 |
| Cache | moka | 0.12 |
| Hot state | DashMap | 6.x |
| Tokenizer | tiktoken-rs | 0.12 |
| CLI | clap | 4.x |
| TUI | ratatui | 0.30 |
| Desktop | Tauri | 2.x |
| Frontend | Vue 3.5 + Vite 8 + TS 6 | strict |
| Styling | Tailwind CSS | 4.x |
| Pipeline viz | Vue Flow | 1.x |
| Charts | ApexCharts | 4.x |
| Auth | jsonwebtoken | 9.x |
| Updates | self_update | 0.44 |
| CI/CD | GitHub Actions | — |
