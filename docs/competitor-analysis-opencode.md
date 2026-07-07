# Competitor Analysis — OpenCode

> Source: https://opencode.ai/docs (accessed 2026-07-07)
> OpenCode by Anomaly — open-source AI coding agent (TUI, desktop, IDE extension)

---

## Overview

OpenCode is an open-source AI coding agent available as a terminal TUI, desktop
app, or IDE extension. It positions itself as a general-purpose coding
assistant — you chat with it, it makes changes, you review. It is
**conversation-driven**, not loop-driven.

praxis and OpenCode overlap in the "AI coding agent" space but differ
fundamentally in architecture and intent:

| | OpenCode | praxis |
|---|---|---|
| **Core paradigm** | Interactive chat (one request → one response) | Autonomous loop (iterate until goal achieved) |
| **Agent model** | Single primary agent + optional subagents | Multi-agent pipeline (architect → coder → reviewer → security → tester) |
| **Completion** | User decides when done | Outcome-based completion criterion (build + test + gates pass) |
| **Deployment** | TUI / desktop / IDE extension | CLI / VPS+dashboard / desktop (Tauri) |
| **Language** | TypeScript (Node/Bun) | Rust (nightly) |
| **Persistence** | File-based sessions | SQLite event store + Qdrant vector DB |
| **Memory** | Session-scoped | Three-tier: hot (DashMap), episodic (Qdrant), consolidated |

---

## Feature Comparison

### 1. Plan Mode

**OpenCode:** Tab key toggles between "Build" (full tools) and "Plan"
(read-only, no file writes, no bash). The Plan agent analyzes code and suggests
a plan without making changes. User reviews, then switches to Build to execute.

**praxis equivalent:** `praxis plan --goal "..." --output plan.md` runs only
the Planning + Designing phases, producing a plan file. `praxis run --dry-run`
shows the plan without executing. `praxis run --plan <file>` executes a saved
plan.

**Gap:** praxis has the functionality but lacks the interactive toggle.
OpenCode's Tab-switch is instant and contextual; praxis requires separate CLI
invocations. However, praxis's plan is a persistent artifact (markdown file)
that can be version-controlled, shared, and reviewed — OpenCode's plan is
ephemeral (in-session only).

**Assessment:** praxis is **at parity**. The persistence advantage offsets the
interactivity disadvantage. No action needed.

### 2. Custom Commands with Arguments

**OpenCode:** Markdown files in `.opencode/commands/` define slash commands
(`/test`, `/component Button`). Frontmatter sets agent, model, description.
`$ARGUMENTS` and positional `$1`, `$2` placeholders inject arguments. Shell
output injection via `` !`command` `` syntax. File references via `@path`.

**praxis equivalent:** praxis has a **skills** system (`builtin_skills()` in
`skills.rs`) and an **injections** system (mid-loop instruction injection via
`praxis inject` or file-based injections in `data_dir/injections/`). But there
is no equivalent of user-defined slash commands with argument templating.

**Gap:** praxis lacks user-defined command templates with `$ARGUMENTS`
substitution, shell-output injection, and file references.

**Recommendation:** Add a `commands/` directory in the project config. Each
`.md` file defines a reusable goal template with frontmatter (agent, model,
description) and `$ARGUMENTS`/`$1`/`$2` placeholders. This aligns with praxis's
existing `praxis run --goal "..."` flow — a command is just a pre-templated
goal. Shell-output injection is a natural fit since praxis already runs shell
commands for completion verification.

**Priority:** Medium. Useful for workflow ergonomics but not core to the loop.

### 3. LSP Integration

**OpenCode:** Experimental `lsp` tool (gated behind
`OPENCODE_EXPERIMENTAL_LSP_TOOL=true`). Integrates with 30+ language servers
(rust-analyzer, typescript, gopls, etc.). Provides `goToDefinition`,
`findReferences`, `hover`, `documentSymbol`, `workspaceSymbol`,
`goToImplementation`, `callHierarchy`. LSP diagnostics feed back into the agent
loop as context.

**praxis equivalent:** None. praxis relies on shell commands (`cargo build`,
`cargo test`, `cargo clippy`) for code intelligence. No LSP integration.

**Gap:** praxis has no LSP integration. The agent navigates code via file
reads and grep, not via semantic code intelligence.

**Assessment:** LSP is a **double-edged sword** (per OpenCode's own docs):
language servers consume memory, can get out of sync, vary by version, and
slow down agent workflows. OpenCode's own best-practices section recommends
running lint/typecheck CLI tools directly instead of LSP in many projects.

For praxis, the existing approach (shell commands for build/test/lint) is
**simpler and more reliable** than LSP. praxis's completion criterion already
runs `cargo build` + `cargo test` — this is the ground truth, not LSP
diagnostics. LSP would add complexity without clear benefit for the
loop-driven use case.

**Recommendation:** **Do not implement LSP integration.** The VISION.md
principle "simple or don't do it" applies. Shell-based verification is more
robust for autonomous loops. If a future use case needs semantic navigation
(e.g., "find all callers of this function"), a lightweight `grep`-based
approach or a future `lsp` MCP server is sufficient.

**Priority:** Low. Not recommended for praxis.

### 4. Agent System

**OpenCode:** Two primary agents (Build, Plan) + three subagents (General,
Explore, Scout). Primary agents switched via Tab. Subagents invoked via
`@mention` or automatically by the primary agent. Each agent has its own
model, temperature, prompt, and permission config. Agents defined via JSON
or markdown files in `.opencode/agents/`.

**praxis equivalent:** Full multi-agent pipeline with roles defined in
`forge.toml`. Each agent has its own model, tools, system prompt, max_turns,
max_depth, max_sub_agents. Agent delegation system (6D — sub-agent spawning).
Built-in roles: architect, coder, reviewer, security, tester. Custom agents
via `praxis agent add`.

**Assessment:** praxis is **more advanced** than OpenCode here. praxis's
multi-agent pipeline with cross-verification (different models review each
other) is a core differentiator. OpenCode's agent system is simpler (single
primary + on-demand subagents) but lacks the structured pipeline.

**Gap:** None. praxis leads.

### 5. Subagent Architecture

**OpenCode:** Subagents create child sessions. Navigation between parent and
child sessions via keybinds. Subagents run in parallel. `subtask: true`
forces subagent invocation to keep primary context clean.

**praxis equivalent:** Agent delegation system (6D) allows sub-agent spawning
for complex tasks. `max_sub_agents` config per role. Sub-agents get their own
provider access via `resolve_provider_for_model`.

**Assessment:** praxis has sub-agent spawning but lacks the **session
hierarchy navigation** that OpenCode provides (parent/child session browsing).
This is a dashboard UX feature, not a core capability.

**Recommendation:** Consider adding parent/child session visualization to the
dashboard in a future frontend iteration.

**Priority:** Low.

### 6. Configuration System

**OpenCode:** Layered config (remote → global → custom → project → .opencode
dirs → inline → managed). JSON/JSONC with schema validation. Supports
organizational managed settings (MDM on macOS). TUI-specific config separate
from server config.

**praxis equivalent:** `forge.toml` (TOML) for project config. Global config
in data_dir. No layered/remote/managed config. No schema validation.

**Gap:** praxis's config is simpler (single `forge.toml` per project) but
lacks the layering and organizational control OpenCode provides.

**Assessment:** praxis's VISION explicitly says "one operator, one token" and
rejects multi-tenant/organizational features. The layered config system is
designed for organizations — **not applicable to praxis's use case**.

**Recommendation:** No action. praxis's single-operator model doesn't need
organizational config layering.

### 7. Undo/Redo

**OpenCode:** `/undo` reverts the last change and restores the original
prompt. `/redo` reapplies. Multiple undos supported. Changes are per-message.

**praxis equivalent:** `praxis undo` / `praxis redo` CLI commands (6E). Change
history stored in SQLite. Undo/redo API endpoints. Works at the file-change
level (git-based snapshots).

**Assessment:** praxis is **at parity**. Both support undo/redo. praxis's
implementation is more robust (SQLite-backed, git snapshots) vs OpenCode's
in-session-only undo.

### 8. Session Sharing

**OpenCode:** `/share` creates a shareable link to the conversation. Conversations
are not shared by default.

**praxis equivalent:** None. praxis sessions are local to the VPS.

**Gap:** praxis has no session sharing. However, praxis's VISION says "one
operator" — sharing is not a priority.

**Recommendation:** Low priority. Could be useful for sharing a session log
with a colleague, but not core to the autonomous loop.

### 9. MCP Server Support

**OpenCode:** Full MCP client support. Custom tools via MCP servers. Permission
system controls MCP tool access.

**praxis equivalent:** `praxis-mcp-host` crate — MCP client with discovery,
protocol, tool registry. First-party MCP servers (filesystem, git, github,
web-search). MCP tools are treated as untrusted (per AGENTS.md security rules).

**Assessment:** praxis is **at parity**. Both support MCP. praxis has
first-party MCP servers built in.

### 10. Permissions System

**OpenCode:** Per-tool permissions (allow/deny/ask). Wildcard patterns for
MCP tools. Per-agent permission overrides. Configurable via JSON.

**praxis equivalent:** No granular permission system. Agents have access to
all tools. MCP tools are sandboxed (untrusted) but there's no per-tool
allow/deny/ask control.

**Gap:** praxis lacks a permission system for tool access control.

**Assessment:** praxis's VISION says "one operator has full control. No
roles." A permission system adds complexity that doesn't serve the
single-operator model. However, an `ask` mode (require approval before
destructive actions) could be valuable for safety in autonomous loops.

**Recommendation:** Consider adding a `--require-approval` flag for
destructive tools (file delete, process kill) in a future iteration. Not
critical for the current use case.

**Priority:** Low.

---

## Features praxis Should Match

Based on the analysis, these OpenCode features are worth porting to praxis:

| Feature | Priority | Rationale |
|---|---|---|
| **Custom commands with args** | Medium | Workflow ergonomics; reusable goal templates with `$ARGUMENTS` |
| **Parent/child session navigation** | Low | Dashboard UX for sub-agent sessions |
| **`--require-approval` for destructive tools** | Low | Safety for autonomous loops |

## Features praxis Should NOT Port

| Feature | Rationale |
|---|---|
| **LSP integration** | Shell-based verification (build/test/clippy) is simpler and more reliable for autonomous loops. LSP adds memory/sync complexity without clear benefit. |
| **Organizational config layering** | praxis is single-operator. No need for remote/managed/MDM config. |
| **Session sharing** | One-operator model. Not a priority. |
| **Granular per-tool permissions** | One operator has full control. An approval flag for destructive actions is sufficient. |

---

## Key Insights

1. **praxis's loop is its moat.** OpenCode is conversation-driven (one request,
   one response). praxis is loop-driven (iterate until verified complete). This
   is the fundamental differentiator — praxis doesn't stop until the work is
   done.

2. **Cross-verification is unique.** OpenCode's agents don't cross-verify each
   other. praxis's pipeline (architect → coder → reviewer → security → tester)
   with different models per role is a quality guarantee that OpenCode lacks.

3. **Simplicity is a feature.** OpenCode has 30+ LSP servers, layered config,
   MDM support, organizational settings. praxis has one binary, one config
   file, shell-based verification. The VISION.md principle "simple or don't
   do it" is validated — OpenCode's own docs admit LSP is often not worth the
   tradeoff.

4. **praxis leads on persistence.** SQLite event store, checkpointing,
   cross-session memory (Qdrant), undo/redo with git snapshots. OpenCode's
   sessions are ephemeral by comparison.

5. **Custom commands are the one worth porting.** OpenCode's
   `.opencode/commands/` system with `$ARGUMENTS` templating is elegant and
   would improve praxis's workflow ergonomics without adding architectural
   complexity.
