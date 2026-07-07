# Competitor Analysis — Market (Claude Code, Cursor, GitHub Copilot, Aider)

> Sources (accessed 2026-07-07):
> - Claude Code: https://code.claude.com/docs/en/sub-agents, /en/scheduled-tasks
> - Cursor: https://cursor.com/docs/agent/overview, /docs/cloud-agent
> - GitHub Copilot: https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent
> - Aider: https://aider.chat/docs/
> - Ralph loop: https://wiggum.app/ralph-loop/ (Geoffrey Huntley technique)

---

## Overview

This analysis covers the broader market of AI coding agents beyond
OpenCode (see `competitor-analysis-opencode.md`). The market splits into
two camps:

1. **IDE-embedded agents** — Cursor, GitHub Copilot agent mode. Run inside
   an editor, pair-programming with a human at the keyboard.
2. **CLI/cloud autonomous agents** — Claude Code, GitHub Copilot cloud
   agent, Aider. Run in a terminal or cloud VM, working independently.

praxis sits in the second camp but goes further: it is **loop-native**,
not merely loop-capable. The competitors bolt loops onto a chat agent;
praxis is architected around the loop from the start.

| | Claude Code | Cursor | GitHub Copilot | Aider | praxis |
|---|---|---|---|---|---|
| **Core paradigm** | Interactive CLI + `/loop` | IDE pair-programmer + cloud agent | IDE agent + cloud agent | Terminal pair-programmer | Autonomous loop (native) |
| **Agent model** | Single + subagents | Single agent | Single + custom agents | Single (architect/code modes) | Multi-agent pipeline |
| **Loop** | `/loop` skill, cron, Routines | Cloud agent (background) | Cloud agent (59-min cap) | Bash loop (manual) | Native run loop |
| **Completion** | User or loop condition | User decides | 59-min timeout / user | User decides | Outcome-based (build+test+gates) |
| **Deployment** | CLI | IDE + cloud VM | IDE + GitHub Actions VM | CLI | CLI / VPS+dashboard / desktop |
| **Persistence** | Session (resumable) | Checkpoints (local) | Git commits + PR | Git commits | SQLite + Qdrant + checkpoints |
| **Memory** | Session + Memory (preview) | Indexed codebase | Copilot Memory (preview) | Repo map | Three-tier (hot/episodic/consolidated) |
| **Multi-repo** | No (single worktree) | Yes (cloud agent) | No (single repo) | No | No (10C proposal) |

---

## 1. Claude Code — Ralph Loop Technique

### What it is

The **Ralph loop** (pioneered by Geoffrey Huntley, named after the
Simpsons character) is an autonomous coding technique that breaks feature
development into structured phases — plan, implement, test, verify, PR
— each run as an independent agent loop with checkpoints.

In its purest form (Huntley's original): "Ralph is a Bash loop." A shell
script re-feeds the same prompt to the agent on every exit, preserving
context across sessions. The agent barely matters — it works with Claude
Code, Codex CLI, OpenCode, Aider — anything that doesn't cap tool calls.

Anthropic later shipped an official **Ralph Loop Plugin** that
intercepts session exits via a stop hook and automatically re-feeds the
prompt. Wiggum CLI productizes the technique with phase isolation, TUI
monitoring, and AI-generated specs.

### Claude Code's native loop support

Beyond the Ralph hack, Claude Code v2.1.72+ ships first-class
scheduling:

- **`/loop`** — re-runs a prompt on an interval (fixed cron or
  dynamic, where Claude chooses the delay). Bare `/loop` runs a built-in
  maintenance prompt (continue unfinished work, tend the PR, run cleanup).
- **`CronCreate`/`CronList`/`CronDelete`** — schedule tasks with 5-field
  cron expressions. Session-scoped, 50 max, 7-day expiry.
- **Routines** — cloud-hosted scheduled tasks on Anthropic
  infrastructure (no machine required, 1-hour minimum interval).
- **Desktop scheduled tasks** — local, persistent across restarts.
- **`/goal`** — keeps the session working turn after turn until a
  condition is met (not interval-based).

### praxis comparison

praxis's loop is **native and outcome-based**, not interval-based. The
Ralph loop's phases (plan → implement → test → verify → PR) map almost
exactly to praxis's pipeline phases (Planning → Designing → Coding →
Reviewing → Security → Testing). The key differences:

| | Ralph loop (Claude Code) | praxis |
|---|---|---|
| **Phase execution** | Sequential, one agent per phase | Pipeline with role-specialized agents |
| **Cross-verification** | Same agent verifies its own work | Different model reviews (security, tester) |
| **Completion** | Loop condition or manual stop | Build + test + gates pass (automated) |
| **Persistence** | Session resume (`--resume`) | SQLite event store + checkpoints |
| **Scheduling** | Cron / Routines / Desktop | `praxis run` (immediate); no scheduler yet |

### Gap & recommendation

**Gap:** praxis lacks scheduled/autonomous execution. A Ralph loop
requires a human to start `praxis run` each time. Claude Code can wake
itself on a schedule or on a condition (`/goal`).

**Recommendation:** Add a `praxis schedule` command and a daemon mode:

- `praxis schedule --cron "0 9 * * 1-5" --goal "..."` — run a goal on a
  schedule (like Claude Code's `CronCreate`).
- `praxis daemon` — long-running process that executes scheduled goals,
  monitors the worktree for drift, and tends open PRs (like Claude
  Code's bare `/loop` maintenance prompt).
- Event triggers: `praxis schedule --on webhook --goal "..."` to run on
  external events (CI failure, issue opened).

**Priority:** Medium-high. This is the single biggest capability gap vs
Claude Code. praxis's loop is more robust, but it can't start itself.
Scheduling unlocks true autonomy — "praxis watches the repo and fixes
things overnight."

---

## 2. Cursor — Agent & Cloud Agent

### What it is

Cursor's **Agent** (Cmd+I in the sidepane) is an IDE-embedded coding
agent with tools: semantic search, file read/edit, shell commands,
browser control, image generation, and clarifying questions. It runs
indefinitely — no tool-call limit.

**Cloud Agents** (formerly Background Agents) run the same agent
fundamentals in isolated VMs in the cloud. They clone the repo, work on
a branch, and open a PR. Key features:

- **Multi-repo environments** — a cloud agent can span separate
  frontend/backend/infra repos, making coordinated changes and opening
  PRs in each.
- **Full dev environments** — agent-led setup, saved snapshots, or
  Dockerfile in `.cursor/environment.json`. Agents can build, test, and
  interact with the changed software.
- **Remote desktop control** — take over the agent's VM to test the
  software yourself, then release control back.
- **Artifacts** — screenshots, videos, logs demonstrating what changed.
- **MCP support** — HTTP and stdio transports, OAuth.
- **Hooks** — `beforeShellExecution`, `afterFileEdit`, `preToolUse`,
  `subagentStart` run in cloud agents.
- **Runtimes** — Cursor-managed, My Machines, or Self-Hosted Pool.

### Checkpoints

Cursor's **Checkpoints** auto-snapshot the codebase before significant
changes during an agent session. Restore any checkpoint to revert.
Stored locally, separate from Git — for undoing agent changes only.

### praxis comparison

| | Cursor Cloud Agent | praxis |
|---|---|---|
| **Execution** | Cloud VM (isolated) | Local/VPS (shared worktree) |
| **Multi-repo** | Yes | No |
| **Environment** | Full VM with desktop/browser | Shell-only (build/test/clippy) |
| **Verification** | Agent runs tests in VM | Outcome-based (build+test+gates) |
| **Undo** | Checkpoints (local, ephemeral) | SQLite + git snapshots (persistent) |
| **Browser/visual** | Yes (screenshots, remote desktop) | No |
| **Triggers** | Manual / @cursor in Slack/GitHub/Linear | `praxis run` (manual) |

### Gap & recommendation

**Gap 1 — Multi-repo.** Cursor cloud agents can coordinate changes across
multiple repositories in one run. praxis operates on a single worktree.
This is a real limitation for projects with shared libraries or
frontend/backend split. → **10C innovation proposal** (multi-repo goal
support).

**Gap 2 — Visual verification.** Cursor agents can launch a browser,
screenshot the app, and verify visual changes. praxis verifies via
build/test/clippy only. For a project with a Vue dashboard, visual
verification would catch CSS/layout regressions that tests miss.

**Recommendation (visual verification):** Add an optional
`--verify-visual` flag to `praxis run` that, after the Testing phase,
launches a headless browser (via the existing `browser` MCP server or a
Playwright integration), navigates key routes, and screenshots. The
agent compares screenshots against a baseline. This is lower priority
than scheduling but valuable for the dashboard.

**Priority:** Multi-repo = Medium (10C). Visual verification = Low-medium.

---

## 3. GitHub Copilot — Cloud Agent

### What it is

GitHub Copilot **cloud agent** works autonomously in a GitHub
Actions-powered ephemeral environment. You assign it tasks from issues,
Copilot Chat, or the agents panel. It researches the repo, creates a
plan, makes changes on a branch, and optionally opens a PR.

Key characteristics:

- **Ephemeral dev environment** — powered by GitHub Actions, with its
  own clone where it can explore, edit, run tests and linters.
- **59-minute hard cap** per session. Cannot be extended. Complex tasks
  must be broken up.
- **Single repo, single branch, single PR** per task. Cannot make
  changes across multiple repositories.
- **Custom agents** — specialized versions for different tasks (frontend,
  docs, testing) with tailored prompts and tools.
- **Copilot Memory** (preview) — stores learned details about a repo
  for reuse across sessions.
- **Automations** — run on a schedule or in response to events (issue
  opened, etc.).
- **Custom instructions** — natural-language files in the repo.
- **MCP servers** — GitHub MCP server and Playwright MCP server enabled
  by default.
- **Hooks** — shell commands at key points during execution.
- **Skills** — instructions, scripts, resources for specialized tasks.
- **Integrations** — Jira, Slack, Teams, Azure Boards, Linear, Raycast,
  GitHub CLI, REST API, MCP Server.

### praxis comparison

| | GitHub Copilot cloud agent | praxis |
|---|---|---|
| **Execution** | GitHub Actions VM (ephemeral) | Local/VPS (persistent) |
| **Time limit** | 59 minutes (hard) | Unlimited |
| **Completion** | User creates PR when ready | Outcome-based (automated) |
| **Multi-agent** | Custom agents (single per task) | Pipeline (multiple per goal) |
| **Memory** | Copilot Memory (preview, repo-scoped) | Three-tier (hot/episodic/consolidated) |
| **Triggers** | Issue assign, schedule, events | `praxis run` (manual) |
| **Ecosystem** | Deep GitHub/IDE/integrations | Standalone |

### Gap & recommendation

**Gap — Event-driven triggers.** Copilot cloud agent can be triggered by
GitHub events (issue opened, PR comment, security alert). praxis has no
event-driven execution.

**Recommendation:** Add GitHub webhook integration to the `praxis
daemon` (proposed in §1). A webhook handler receives issue/PR events and
triggers `praxis run --goal "<issue body>"`. This makes praxis a
self-hosted alternative to Copilot cloud agent — assign an issue, praxis
picks it up, runs the full pipeline, pushes a branch. Combined with
scheduling (§1), this closes the autonomy gap.

**Priority:** Medium. Depends on the daemon mode (§1). The 59-minute cap
is a Copilot weakness praxis inherently avoids — praxis runs until
done.

**Insight:** Copilot's 59-minute hard cap is a significant limitation
for complex tasks. praxis's unlimited runtime is a differentiator. The
marketing position: "praxis doesn't time out — it finishes."

---

## 4. Aider — Terminal Pair Programmer

### What it is

Aider is AI pair programming in the terminal. It is
**conversation-driven** — you chat, it edits files in your local git
repo. Key features:

- **Chat modes** — `code` (edits files), `architect` (plans, then hands
  to code mode), `ask` (read-only Q&A), `help`.
- **Repository map** — uses tree-sitter to build a map of the repo,
  giving the LLM code context without reading every file.
- **Git integration** — auto-commits after each change with descriptive
  messages. Easy undo via git revert.
- **Edit formats** — multiple strategies (whole, diff, udiff, etc.) for
  how the LLM expresses edits.
- **Watch mode** — monitors files for `AI` comments in your IDE and
  responds to them.
- **Lint/test** — automatically fixes linting and test errors after
  edits.
- **Multi-LLM** — connects to most providers (OpenAI, Anthropic, Gemini,
  Ollama, etc.).
- **Leaderboards** — quantitative benchmarks of LLM code editing skill.

### praxis comparison

| | Aider | praxis |
|---|---|---|
| **Core paradigm** | Conversation (pair programmer) | Autonomous loop |
| **Agent model** | Single (mode-switched) | Multi-agent pipeline |
| **Completion** | User decides | Outcome-based (automated) |
| **Repo context** | Repository map (tree-sitter) | File reads + grep |
| **Git** | Auto-commit per change | Checkpoints + undo/redo |
| **Edit strategy** | Multiple edit formats | Agent-driven (file writes) |
| **Autonomy** | None (interactive only) | Full loop until done |

### Gap & recommendation

**Gap — Repository map.** Aider's tree-sitter-based repository map gives
the LLM structural context (which files define which symbols, call
relationships) without reading entire files. praxis relies on file reads
and grep, which costs tokens and misses structural relationships.

**Recommendation:** Consider a lightweight repo-map feature: a
tree-sitter pass that builds a symbol index (file → symbols →
callers/callees), injected into the Planning phase context. This is
distinct from LSP (rejected in the OpenCode analysis) — a repo map is a
static, one-pass index, not a live language server. It costs nothing at
runtime and gives the architect agent structural awareness.

**Priority:** Low-medium. The OpenCode analysis rejected LSP for
runtime complexity reasons. A static repo map avoids those issues —
it's a build-time artifact, not a running server. Worth evaluating but
not urgent; praxis's pipeline already works without it.

**Insight:** Aider is the closest competitor to praxis's CLI
experience, but it is fundamentally a pair programmer, not an autonomous
agent. praxis's value proposition over Aider: "walk away, come back to
finished work." Aider requires you to stay at the keyboard.

---

## Sweep (brief)

Sweep (sweep.dev) was an AI junior developer that turned GitHub issues
into pull requests. As of 2024, Sweep pivoted/wound down — the
issue-to-PR space was absorbed by GitHub Copilot cloud agent. No active
analysis warranted; the concept (issue → autonomous PR) is covered in
the Copilot section above and the daemon/webhook recommendation (§3).

---

## Features praxis Should Match

| Feature | Priority | Source | Rationale |
|---|---|---|---|
| **Scheduled execution (`praxis schedule`, daemon mode)** | Medium-high | Claude Code `/loop`, Copilot automations | Biggest autonomy gap. praxis can't start itself. Unlocks "fix overnight" use case. |
| **Event-driven triggers (GitHub webhooks)** | Medium | Copilot cloud agent | Assign issue → praxis runs. Self-hosted Copilot alternative. Depends on daemon. |
| **Multi-repo goal support** | Medium | Cursor cloud agent | Coordinated changes across repos. Separate 10C proposal. |
| **Static repository map (tree-sitter)** | Low-medium | Aider | Structural context for Planning phase without LSP runtime cost. |
| **Visual verification (`--verify-visual`)** | Low-medium | Cursor browser tool | Catch CSS/layout regressions in dashboard. Optional phase. |

## Features praxis Should NOT Port

| Feature | Rationale |
|---|---|
| **IDE embedding** | praxis is CLI/server-first. IDE embedding adds complexity for a different audience. |
| **Cloud VM execution** | praxis runs on your VPS. Cloud VMs add cost, latency, and a trust boundary. The local model is a feature. |
| **59-minute session cap** | Copilot's cap is a limitation, not a feature. praxis's unlimited runtime is a differentiator. |
| **Remote desktop control** | Cursor's remote desktop is for visual apps. praxis is backend-focused; shell verification suffices. |
| **Image generation tool** | Not relevant to praxis's autonomous coding loop. |

---

## Key Insights

1. **The loop is the market's direction.** Every major competitor is
   converging on autonomous loops: Claude Code's `/loop` and Routines,
   Cursor's Cloud Agents, Copilot's cloud agent, the Ralph loop
   technique. praxis was born as a loop — this is validation, not a
   race to catch up.

2. **praxis's completion criterion is unique.** Competitors stop on
   user decision, time limits (Copilot's 59 min), or manual loop
   conditions. praxis stops when build + test + gates pass — objective
   completion, not subjective. This is the core differentiator and
   should be the marketing headline.

3. **Cross-verification is unmatched.** No competitor runs a pipeline
   where different models review each other's work (architect → coder →
   reviewer → security → tester). Claude Code subagents are
   task-specialized but don't cross-verify. Cursor and Copilot are
   single-agent. Aider is single-agent with mode-switching. praxis's
   multi-agent cross-verification is a quality guarantee none offer.

4. **Scheduling is the gap to close.** praxis's loop is more robust
   than any competitor's, but it can't start itself. Claude Code's
   `/loop`/Routines and Copilot's automations can. Adding `praxis
   schedule` + daemon mode + webhook triggers would make praxis the
   only tool with both a robust loop **and** autonomous initiation.

5. **Persistence is a moat.** SQLite event store, checkpointing,
   three-tier memory, undo/redo with git snapshots. Competitors have
   session resume (Claude Code), local checkpoints (Cursor), repo
   memory (Copilot preview), repo maps (Aider). praxis has all of this
   integrated and persistent across sessions. Don't understate this.

6. **The 59-minute cap is a wedge.** Copilot cloud agent's hard
   59-minute session limit is a significant constraint. praxis runs
   until done. For complex refactors or multi-file features, this is a
   clear advantage. Position: "praxis doesn't time out — it finishes."

7. **Multi-repo is the frontier.** Only Cursor cloud agents support
   multi-repo coordinated changes. As projects increasingly split into
   frontend/backend/shared-library repos, this becomes table stakes.
   The 10C innovation proposal should prioritize multi-repo goal
   support.

8. **Static repo map > LSP.** Aider's tree-sitter repo map gives
   structural context without LSP's runtime cost. This aligns with the
   OpenCode analysis's rejection of LSP — a static, one-pass symbol
   index is the middle ground: structural awareness without a running
   server.
