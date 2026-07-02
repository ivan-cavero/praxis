# praxis — Vision

> The north star. If a technical decision doesn't serve this vision, don't make it.

---

## What praxis Is

**praxis is a system of autonomous agents that iterate in a loop until they
complete an objective.**

One binary. You install it on a VPS or locally. You create a project, assign
agents to it (each with its own model, its own tools, its own role), give it a
goal, and the agents work in a loop — they design, implement, review, fix, test
— until the goal is done or they hit a hard limit.

It is not a chat. It is not a single-response assistant. It is an **agent
factory** that doesn't stop until the work is finished.

---

## The Three Modes of Use

### 1. CLI (local or VPS)

The base mode. Everything works from the terminal:

```bash
praxis init my-api                    # create a project
praxis agent add coder --model gpt-5  # assign agents
praxis run --goal "REST API with JWT" # loop until complete
praxis status                         # see live status
```

Works the same on your machine and on a VPS. A single binary, no Docker, no
Postgres, no Redis. SQLite + Qdrant embedded run inside the process.

### 2. VPS + Dashboard

On the same VPS where the service runs, you start a web UI:

```bash
praxis server   # starts API + WebSocket on :8080
```

You connect from the browser to `http://your-vps:8080` and see the dashboard:
agent pipeline, token metrics, health of each agent, live logs, mid-loop
instruction injection. You manage and monitor everything from there.

### 3. Desktop App

A desktop app (Tauri) like OpenCode, but focused on **autonomous agents in a
loop**. From it you manage:

- Projects (create, configure, archive)
- Agents and their models (OpenAI, Anthropic, Gemini, Ollama, any
  OpenAI-compatible)
- Connections (API keys via secure vault)
- Goals and running sessions

**And from this app you connect to remote machines** that have praxis installed.
You can manage a remote VPS with the same dashboard as local. Local and remote,
same interface, same control.

---

## What praxis Is NOT

To stay focused, these are **out of scope**:

| Out of scope | Why |
|---|---|
| Multi-tenant / organizations | praxis is for a single operator. If you need isolation, run another instance. |
| Billing / plans / SaaS | It's not a SaaS product. It's a binary you install. |
| SSO / SAML / enterprise auth | Local JWT auth is enough. One operator, one token. |
| RBAC with hierarchical roles | One operator has full control. No roles. |
| Teams / invited users | One operator. If you want to share, share VPS access. |

If this changes in the future, it gets reconsidered. But **none of this is
built until the core works and is actually used.**

---

## The Loop — the Heart of the System

The loop is what differentiates praxis from a single-response chat:

```
┌─────────────────────────────────────────────────┐
│                    GOAL                          │
│  "Create a REST API in Rust with JWT auth"      │
└──────────────────────┬──────────────────────────┘
                       ▼
              ┌────────────────┐
              │  ORCHESTRATOR  │  reads the goal, activates agents
              └────────┬───────┘
                       ▼
   ┌───────────────────────────────────────────┐
   │              ITERATION N                   │
   │                                            │
   │  Architect → designs                       │
   │  Coder     → implements                    │
   │  Reviewer  → reviews (different model)     │
   │  Security  → scans                         │
   │  Tester    → runs tests                    │
   │                                            │
   │  ¿Gates passed? ── no ──→ feedback ──┐    │
   │       │                               │    │
   │       yes                              │    │
   │       ▼                               ▼    │
   │  ¿Goal achieved?              iteration N+1 │
   │       │                               │    │
   │       yes                              │    │
   │       ▼ <─────────────────────────────┘    │
   └───────────────────────────────────────────┘
                       ▼
                ┌────────────┐
                │  COMPLETED  │
                └────────────┘
```

**Loop rules:**

1. **Iterate until complete** — doesn't stop at the first response. Keeps going
   until gates pass AND the goal is achieved.
2. **Hard limits** — `max_iterations`, `session_ttl`, `phase_timeout`. The loop
   never runs uncontrolled.
3. **Cross-verification** — agents with different models review each other. One
   model's blind spot is another's strength.
4. **Drift detection** — if an agent gets stuck (repeats, oscillates, degrades),
   the DriftGuard detects it and acts (context reset, model upgrade, pause).
5. **Mid-loop injection** — you can inject instructions while it runs: "use
   thiserror instead of anyhow", "stop, change the goal to X". The agent
   receives them in the next iteration.
6. **Checkpointing** — every phase transition is persisted. If it crashes, it
   resumes from the last checkpoint.
7. **Cross-session memory** — everything is indexed in a vector DB. The agent
   remembers decisions and learnings from previous sessions.

---

## Completion Criteria — Outcome-Based, Not Action-Based

**This is the most important principle of loop engineering.**

A completion criterion defines when the agent is allowed to stop iterating.

**Bad criterion:** "Stop when you call the book tool."
- This assumes executing the tool means the task is done. But the booking can
  fail, the slot can become unavailable, the confirmation might not arrive.

**Good criterion:** "Stop only when the appointment is confirmed OR there are no
slots available."
- This verifies the OUTCOME, not the action.

### The rule

> A completion criterion must measure whether the action produced the expected
> result, not whether the action was executed.

### Implementation

Every goal has a `CompletionCriterion` that is evaluated after each iteration:

- `OutcomeResult::Achieved` — the goal is verified complete (with evidence).
- `OutcomeResult::NotAchieved` — keep iterating.
- `OutcomeResult::Exhausted` — give up (no more options, impossible to achieve).

For coding tasks, the default criterion verifies:
- `cargo build` succeeds
- `cargo test` passes
- Reviewer gate passes
- Security gate passes

---

## Loop Pathology Detection

An agent stuck in a loop can be destructive — killing processes, creating
processes, repeating the same failed action forever. praxis detects this.

### Detection mechanisms

| Pattern | What it detects | Action |
|---|---|---|
| **Repetition** | Same output/action N times in a row | Force context reset |
| **Oscillation** | A→B→A→B phase cycling | Break the cycle, escalate |
| **No progress** | N iterations without state change | Pause agent, notify |
| **Destructive behavior** | Process kill/create patterns, file deletion loops | Kill session immediately |
| **Token waste** | Token usage growing without progress | Throttle, downgrade model |

### Cross-model verification

When pathology is suspected, a second model (different from the one stuck) can
be asked: "Is this agent making progress? Yes or no, with reasoning." This
catches subtle stuck states that algorithmic detection misses.

---

## Architecture — Simple and Boring

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
│  │              PERSISTENCE                        │ │
│  │  SQLite (event store) · Qdrant (vector) ·       │ │
│  │  moka (cache) · DashMap (hot state)             │ │
│  └─────────────────────────────────────────────────┘ │
│                                                       │
│  ┌─────────────────────────────────────────────────┐ │
│  │          Tauri Desktop (Vue 3)                   │ │
│  │  Pipeline · Tokens · Agent Health · Memory ·    │ │
│  │  Inject · Config · Remote VPS connection        │ │
│  └─────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

One binary. No external services. SQLite and Qdrant embedded. The UI is the same
binary in desktop mode or served via HTTP.

---

## Stack

- **Rust** (nightly) — core, CLI, desktop
- **ractor** — actor model (each agent is an actor with its own mailbox)
- **tokio** — async runtime
- **axum** — REST API + WebSocket
- **SQLite** (rusqlite + r2d2) — event store, checkpoints
- **Qdrant** (embedded) — episodic memory (embeddings)
- **moka** — LLM cache
- **DashMap** — hot state
- **Tauri v2** — desktop app
- **Vue 3.5** (Composition API) + Vite + TypeScript + Tailwind — dashboard
- **reqwest** (rustls) — LLM providers (OpenAI, Anthropic, Gemini, Ollama)

---

## Design Principles

1. **One binary, any environment.** Install once. No Docker, no Postgres, no
   Redis. SQLite + Qdrant embedded.
2. **The loop is the product.** It's not a chat. It's agents that iterate until
   done.
3. **Cross-verification.** Different models for different agents. One model
   writes, another reviews, another audits.
4. **Loop with brakes.** State machine, quality gates, drift detection, hard
   limits. Never runs uncontrolled.
5. **Outcome-based completion.** The loop stops when the result is verified, not
   when an action is executed.
6. **Pathology detection.** Stuck, oscillating, or destructive agents are
   detected and stopped.
7. **Three-tier memory.** Hot (in-memory), Episodic (vector DB), Consolidated
   (summaries). The agent remembers between sessions.
8. **CLI and desktop are the same binary.** Tauri embeds the core. The CLI is
   the same binary in headless mode.
9. **Local and remote, same interface.** The desktop connects to remote
   instances just like local.
10. **Simple or don't do it.** The boring, readable solution first.
11. **Delete dead code without fear.** What's not used gets removed.
12. **Small commits.** Atomic, logical, reviewable.
