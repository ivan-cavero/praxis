# praxis — ROADMAP

> Focus: **the loop works → VPS + dashboard → desktop + remote → memory → production.**
> No enterprise. No multi-tenant. No billing. See [VISION.md](./VISION.md).

---

## Legend

- `[x]` done · `[~]` in progress · `[ ]` pending
- 🧪 validation milestone

---

## Current State

What ALREADY exists and compiles (262 tests pass):

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
| Desktop (Tauri v2) | `[~]` (minimal shell) |
| Dashboard (Vue 3) | `[~]` (minimal: login + settings) |

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
- [ ] **Tool execution** — agents can call MCP tools (filesystem: read/write/edit)
  during `execute()`.
- [ ] **Streaming output** — `provider.stream()` → EventBus → CLI/dashboard.

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
- [ ] **Configurable per goal** — in `forge.toml`: `completion = "tests_pass"` or
  custom verifier

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
- [ ] **Token waste detection** — token usage growing without progress → throttle
- [ ] **Cross-model verification** — when pathology suspected, ask a different
  model: "Is this agent making progress?"

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
- [x] **Graceful shutdown** — Ctrl+C handler saves checkpoint and exits.
- [ ] **`status`** — status of active sessions.
- [~] **`session list/show/stop/logs`** — manage sessions (stubs).
- [~] **`inject`** — mid-loop injection (stub).

🧪 **Milestone:** `praxis init demo && praxis run --goal "Create a hello world
in Rust"` → the loop iterates, the coder writes code, the reviewer approves,
tests pass, goal verified complete. With real LLM (or mock if no API key).

---

## Phase 2 — Real Agents + Tools

**Goal:** Multiple agents with different models, cross-verification, MCP tools,
mid-loop injection.

### 2.1 Agent Roles

- [ ] **ArchitectAgent** — designs, generates ADRs. Profile: `generous`.
- [ ] **CoderAgent** — implements, compiles, iterates on errors. Profile: `balanced`.
- [ ] **ReviewerAgent** — reviews code, pass/fail + comments. Profile: `aggressive`.
- [ ] **SecurityAgent** — scans secrets, unsafe, injection. Profile: `aggressive`.
- [ ] **TesterAgent** — generates tests, runs `cargo test`. Profile: `balanced`.
- [ ] **GitAgent** — commit, branch, PR.

### 2.2 Cross-Verification

- [ ] **Parallel execution** — `tokio::join!` for parallel reviewers.
- [ ] **`parallel_reviewers` config** — spawn N instances with different models.
- [ ] **ConsensusConsolidator** — all_pass, majority, weighted, escalate.
- [ ] **Cross-model feedback loop** — consolidate comments → coder → fix.

### 2.3 MCP Tools

- [ ] **Filesystem MCP server** — read/write/edit/list/search, path sandboxing.
- [ ] **Git MCP server** — init/add/commit/branch/diff/log/push.
- [ ] **GitHub MCP server** — PR create/list/comment, issues, checks.
- [ ] **WebSearch MCP server** — search, fetch (for ResearcherAgent).

### 2.4 Mid-Loop Injection

- [ ] **InjectionChannel** — broadcast channel for mid-loop instructions.
- [ ] **Agent checks pending injections** before each LLM call.
- [ ] **Injections have CRITICAL priority** — never compressed/dropped.
- [ ] **`inject` CLI command** — works for real.
- [ ] **Audit** — all injections are logged.

🧪 **Milestone:** Goal → Architect designs → Coder implements → 2 Reviewers
(GPT-5 + Claude) review in parallel → consensus → Security scans → Tester runs
→ gate passes. Inject "use thiserror" mid-loop → coder applies it.

---

## Phase 3 — VPS + Dashboard

**Goal:** Install on VPS with one command. Modern dark mode web dashboard to
monitor and manage.

### 3.1 Easy VPS Installation

- [ ] **Install script** — `curl -fsSL https://praxis.dev/install.sh | bash`.
  OS detection, download binary, verify checksum.
- [ ] **systemd service** — `praxis server` as a service, auto-restart.
- [ ] **Config file** — `~/.config/praxis/config.toml` (port, data dir, auth).
- [ ] **Single binary** — release build with LTO + strip (~15-20 MB).

### 3.2 API Server (refine)

- [ ] **REST endpoints** — projects, sessions, agents, metrics, context, inject.
  `[~]` partial, complete the missing ones.
- [ ] **WebSocket** — real-time events (phase changes, token usage, drift,
  compression, injections). `[~]` partial.
- [ ] **Auth JWT** — first-run token, 24h expiry. `[x]` done, verify.
- [ ] **CORS** — configurable for remote dashboard. `[x]` done.

### 3.3 Vue 3 Dashboard — Modern Dark Mode

- [ ] **Design system** — Tailwind 4, dark palette (black/zinc), clean typography,
      clean components.
- [ ] **DashboardView** — overview: active sessions, total tokens, health.
- [ ] **PipelineView** — phase graph with colors, animated edges (Vue Flow).
- [ ] **SessionView** — session detail: agents, iterations, live logs.
- [ ] **TokenChart** — real-time area chart, breakdown by agent/model.
- [ ] **AgentHealth** — table: model, status, ASI, latency, error rate.
- [ ] **ContextPanel** — pressure gauge, budget breakdown, compression history.
- [ ] **InjectPanel** — target selector, type, message, send, history.
- [ ] **ProjectConfig** — TOML editor with validation.
- [ ] **useWebSocket** composable — auto-reconnect, backoff, event buffering.
- [ ] **Responsive** — 3-col desktop, 2-col tablet, 1-col mobile.

🧪 **Milestone:** `praxis server` on VPS → browser to `http://vps:8080` → modern
dark dashboard shows the loop live, tokens, agent health. Inject instruction
from the panel → reaches the agent.

---

## Phase 4 — Desktop + Remote

**Goal:** Desktop app (Tauri) that manages everything and connects to remote VPS.

### 4.1 Desktop App Base

- [ ] **Tauri v2** — window 1280x800, title "praxis".
- [ ] **Core embedded** — `CoreRuntime` initialized in setup.
- [ ] **Same UI as dashboard** — the Vue dashboard served via Tauri WebView.
- [ ] **System tray** — show/hide, new session, settings, quit.
- [ ] **Auto-update** — `tauri-plugin-updater` from GitHub releases.

### 4.2 Local Management

- [ ] **Projects** — create, configure, archive, list.
- [ ] **Agents** — configure roles, models, system prompts, tools.
- [ ] **Models/Providers** — add API keys via secure vault, test connections.
- [ ] **Goals/Sessions** — create goals, launch, stop, watch live.
- [ ] **Config** — forge.toml editor with validation.

### 4.3 Remote Connection

- [ ] **Connection manager** — list of remote VPS (host, port, token).
- [ ] **Add remote** — form: host, port, auth token. Test connection.
- [ ] **Switch local/remote** — toggle between local and remote instance.
- [ ] **Remote dashboard** — same UI but connected to the remote VPS API.
- [ ] **Remote management** — everything from local management, but against VPS.
- [ ] **Connection status** — connection indicator, latency, auto-reconnect.

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
