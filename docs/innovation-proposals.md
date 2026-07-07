# Innovation Proposals — Multi-Repo, Marketplace, Community Templates

> Research for praxis RESEARCH domain task 10C.
> Builds on `competitor-analysis-opencode.md` and `competitor-analysis-market.md`.
> Sources (accessed 2026-07-07):
> - Cursor Cloud Agents: https://cursor.com/docs/cloud-agent (multi-repo)
> - Claude Code Plugins: https://code.claude.com/docs/en/plugins
> - Claude Code Plugin Marketplaces: https://code.claude.com/docs/en/plugin-marketplaces
> - Claude Code Skills: https://code.claude.com/docs/en/skills
> - praxis architecture: VISION.md, crates/core/src/{config,runtime,pipeline}.rs,
>   crates/core/src/agents/definition.rs

---

## How to Read These Proposals

Each proposal is evaluated against three filters:

1. **VISION alignment** — Does it serve the north star? VISION.md explicitly
   rejects multi-tenant, billing/SaaS, SSO, RBAC, and teams. Proposals that
   require those are rejected or scoped down to the single-operator model.
2. **Architecture fit** — Can it be built on the current architecture without
   a rewrite? praxis has: single `forge.toml`, single working directory,
   single `project_name`, agent `.md` files with YAML frontmatter, 8 built-in
   agents, skills system, SQLite event store, Qdrant memory.
3. **Complexity vs. value** — VISION principle #10: "Simple or don't do it."
   Each proposal gets a verdict: **Build**, **Defer**, or **Reject**.

---

## Proposal 1 — Multi-Repo Goal Support

### The problem

Modern projects split across repositories: a frontend repo, a backend repo,
a shared-types library, an infra repo. A goal like "add a new API endpoint
and the frontend page that calls it" touches two or more repos. praxis
currently operates on a single working directory — one `forge.toml`, one
worktree. Coordinated cross-repo changes require the operator to run
separate `praxis run` invocations per repo and manually reconcile.

### Competitor precedent

**Cursor Cloud Agents** support multi-repo environments. A cloud agent clones
multiple repos into one workspace, inspects the full set, makes coordinated
changes, and opens a PR in each repo it touches. This is the only competitor
with multi-repo support; it is flagged as "the frontier" in the market
analysis.

### Design for praxis

praxis's single-binary, local-first model means we don't clone repos into a
cloud VM. Instead, multi-repo is a **workspace** concept: a manifest that
declares multiple local repo paths and how they relate.

#### Workspace manifest (`praxis-workspace.toml`)

```toml
[workspace]
name = "my-platform"

[[repos]]
name = "backend"
path = "../backend"
default = true          # agents run commands here unless specified

[[repos]]
name = "frontend"
path = "../frontend"

[[repos]]
name = "shared-types"
path = "../shared-types"
```

A workspace goal references repos explicitly or lets the architect agent
decide:

```toml
[[goals]]
name = "add-user-endpoint"
description = "Add a /users endpoint in backend, the UserCard component in frontend, and shared User type in shared-types"
repos = ["backend", "frontend", "shared-types"]
```

#### Architectural changes required

1. **`CoreRuntime` gains a `repos: Vec<RepoHandle>` field** (currently
   single implicit working dir). Each `RepoHandle` has a name, path, and
   whether it's the default. The MCP filesystem server and shell tool
   execution are scoped to the union of repo paths (sandboxed).
2. **Agent context includes the workspace map.** The architect agent
   receives the repo list with paths and a one-line description (from
   each repo's `forge.toml` `[project]` name or a `README.md` first
   paragraph). This lets it plan which repo each sub-task targets.
3. **Per-repo completion criteria.** The default coding criterion runs
   `cargo build`/`test` per repo (or the repo's declared verify command).
   A goal is complete only when all touched repos pass their gates.
4. **PR/commit per repo.** The git agent commits to each repo
   independently. praxis doesn't open PRs (no GitHub auth in scope), but
   the operator can push each repo's branch.

#### What this is NOT

- **Not cloud cloning.** Repos must be checked out locally. praxis
  doesn't fetch repos. This keeps the single-binary, no-external-services
  model.
- **Not monorepo support.** A monorepo already works — it's a single
  working directory. This proposal is for *separate* repos that need
  coordinated changes.
- **Not multi-tenant.** One operator, one workspace. No sharing.

#### Complexity assessment

| Component | Effort | Risk |
|---|---|---|
| Workspace manifest parsing | Low | New config type; additive |
| `CoreRuntime` multi-repo field | Medium | Touches runtime + pipeline; needs careful migration |
| Agent context with repo map | Low | String injection into system prompt |
| Per-repo completion criteria | Medium | CompletionCriterion trait needs per-repo gate evaluation |
| Shell/filesystem sandboxing to repo union | Medium | MCP filesystem server already sandboxes; extend to N paths |
| CLI: `praxis run --workspace ../platform` | Low | New flag |

**Total: Medium.** The hardest part is the completion criterion — verifying
across repos. The rest is additive config + context injection.

### Verdict: **Build (medium priority)**

This is the highest-value proposal. It addresses a real limitation
(single-repo only) that competitors are starting to solve. It fits the
single-operator model (no multi-tenant). The complexity is manageable
because praxis's agent pipeline already abstracts over "where commands
run" — the working directory is an implicit single value that becomes a
list.

**Recommended phasing:**
1. Phase 1: Workspace manifest + multi-repo context injection (architect
   sees all repos). No execution changes — agents can read across repos
   but completion is still single-repo. Low risk, immediate value.
2. Phase 2: Per-repo completion criteria + sandboxed multi-repo shell
   execution. Full multi-repo loops.
3. Phase 3: `praxis workspace` CLI commands (init, add-repo, list).

---

## Proposal 2 — Agent/Model Marketplace

### The problem

praxis has 8 built-in agents (architect, coder, reviewer, security, tester,
git, researcher, explorer) and supports custom agents via `.md` files. But
there's no way to *discover* and *install* agents others have written. An
operator who wants a "database-migration" agent or a "kubernetes" agent
writes it from scratch. The community can't share agent expertise.

### Competitor precedent

**Claude Code** has a full plugin marketplace system:
- **Plugins** are self-contained directories with a `.claude-plugin/
  plugin.json` manifest, bundling skills, agents, hooks, MCP servers, LSP
  servers, and monitors.
- **Marketplaces** are `marketplace.json` catalogs hosted in git repos.
  Users add a marketplace with `/plugin marketplace add <git-url>` and
  install plugins with `/plugin install <name>@<marketplace>`.
- **Sources** support git (github, url, git-subdir) and npm. Plugins are
  cached locally at `~/.claude/plugins/cache`.
- **Versioning**: plugins pin to a `version` field or fall back to git
  commit SHA. Users refresh with `/plugin marketplace update`.
- **Namespacing**: plugin skills are namespaced (`/plugin-name:skill`)
  to prevent conflicts.
- **Symlinks** allow sharing files across plugins within a marketplace.

This is a mature, well-designed system. The question is how much of it
fits praxis's single-operator model.

### Design for praxis

praxis's agents are simpler than Claude Code plugins — they're just `.md`
files with YAML frontmatter (name, model, tools, max_turns, can_spawn,
max_sub_agents) + a Markdown system prompt. No hooks, no MCP servers, no
LSP. This makes a marketplace *simpler* to build.

#### praxis agent registry (`praxis registry`)

```bash
# Add a registry (git repo containing a catalog)
praxis registry add https://github.com/praxis-community/agents

# List available agents
praxis registry list

# Install an agent
praxis registry install rust-expert@praxis-community

# Update
praxis registry update
```

#### Catalog format (`praxis-agents.json`)

```json
{
  "name": "praxis-community",
  "owner": { "name": "praxis community" },
  "agents": [
    {
      "name": "rust-expert",
      "description": "Edition 2024 idiomatic Rust specialist",
      "version": "1.0.0",
      "source": { "source": "github", "repo": "praxis-community/rust-expert" }
    },
    {
      "name": "kubernetes",
      "description": "K8s manifest and Helm chart expert",
      "version": "0.3.1",
      "source": { "source": "github", "repo": "praxis-community/k8s-agent" }
    }
  ]
}
```

Installed agents land in `~/.praxis/agents/` (user scope) or
`./.praxis/agents/` (project scope), reusing the existing 3-scope agent
discovery.

#### What this is NOT

- **Not a SaaS marketplace.** No hosted registry, no accounts, no billing.
  Registries are git repos. Anyone can host one. This fits VISION's "not
  a SaaS product" constraint.
- **Not plugin bundles.** praxis agents don't bundle hooks/MCP/LSP. An
  agent is one `.md` file. Simpler than Claude Code plugins.
- **Not auto-execution.** Installing an agent doesn't run it. The
  operator references it in `forge.toml` roles or via `praxis agent add`.

#### Security considerations (critical)

Agents are **system prompts** — instructions to an LLM with tool access.
A malicious agent could instruct the LLM to exfiltrate secrets, delete
files, or run destructive commands. This is more dangerous than a Claude
Code skill because praxis agents run autonomously in a loop.

Mitigations:
1. **Trust on install.** `praxis registry install` prints the agent's
   system prompt and asks for confirmation. The operator must read it.
2. **No auto-update of system prompts.** Once installed, an agent's
   `.md` is pinned. `praxis registry update` updates the catalog index
   but does NOT overwrite installed agent files without `--force`.
3. **Tool restrictions honored.** The agent's `tools` frontmatter field
   limits what it can call. A `kubernetes` agent with `tools: ["shell"]`
   can't access the vault.
4. **Sandbox.** MCP tools are already untrusted (AGENTS.md security
   rules). Agent-installed MCP servers (if ever supported) get the same
   treatment.

#### Complexity assessment

| Component | Effort | Risk |
|---|---|---|
| Catalog JSON parsing | Low | New type; additive |
| Git clone/install to agents dir | Medium | New `registry` module in CLI; git dep |
| `praxis registry` CLI commands | Low | clap subcommands |
| Install confirmation + prompt display | Low | Print + stdin confirm |
| Agent scope discovery (already exists) | None | Reuse 3-scope system |
| Security: pin installed files, no auto-overwrite | Low | File copy + version check |

**Total: Medium.** The core is a git-clone-and-copy with a JSON catalog.
The security surface (untrusted system prompts) is the real concern, not
the engineering.

### Verdict: **Defer (low-medium priority)**

**Why defer:** The engineering is straightforward, but the security model
for untrusted autonomous agents is not. praxis agents run in a loop with
tool access for hours — a malicious agent prompt is a serious threat.
Until praxis has a permission/approval system (the `--require-approval`
flag flagged as low priority in the OpenCode analysis), installing
third-party autonomous agents is risky. The operator model assumes one
trusted operator writing their own agents; third-party agents break that
assumption.

**When to revisit:** After the `--require-approval` flag for destructive
tools exists (OpenCode analysis recommendation) and after the MCP path
sandboxing (3E) is complete. At that point, a registry with strict
install-time review and tool restriction is reasonable.

**Alternative now:** Support a simpler "agent import" — `praxis agent
import <path-to-.md>` copies a local file into the project's
`.praxis/agents/`. No registry, no git, no remote. The operator
explicitly provides the file. This is a trivial addition (file copy +
validate frontmatter) and covers the "I found an agent .md online"
use case without the marketplace security surface.

---

## Proposal 3 — Community Goal Templates

### The problem

praxis goals are defined in `forge.toml` as `[[goals]]` entries: name,
description, optional workflow. The operator writes each goal from
scratch. Common patterns — "set up CI", "add a REST endpoint", "migrate
to edition 2024", "add a new dashboard view" — are rewritten every time.

The dashboard has a "Goal library (reusable templates)" feature (8B,
marked done). But that's a UI feature for managing saved goals locally.
There's no way to share goal templates across projects or operators.

### Competitor precedent

**Claude Code Skills** are the closest analog. A skill is a `SKILL.md`
file with YAML frontmatter (`description`, `arguments`, `disable-model-
invocation`) + Markdown instructions. Skills support `$ARGUMENTS` and
`$1`/`$2` positional substitution. They live in personal (`~/.claude/
skills/`), project (`.claude/skills/`), or plugin scope. The
`/run-skill-generator` bundled skill even auto-generates project-specific
run skills.

praxis already has a **skills system** (`builtin_skills()` in skills.rs)
and **injections**, but no user-defined slash commands with argument
templating (flagged as medium priority in the OpenCode analysis). Goal
templates are the goal-level equivalent.

### Design for praxis

A goal template is a `.md` file with YAML frontmatter + a goal
description body using `$ARGUMENTS` substitution. Templates live in a
`goals/` directory (project or user scope).

#### Template format (`goals/add-endpoint.md`)

```markdown
---
name: add-endpoint
description: Add a new REST API endpoint with tests
arguments: [resource, methods]
---

Add a $resource REST API endpoint supporting $methods methods.
Include:
- Route handler in the appropriate module
- Request/response types
- Input validation
- Unit tests for happy path and validation failures
- Update the OpenAPI spec if present
```

#### Usage

```bash
# Instantiate a template into forge.toml
praxis goal new add-endpoint "users" "GET,POST,DELETE"

# Or run directly from a template
praxis run --template add-endpoint --args "users" "GET,POST,DELETE"
```

`praxis goal new` writes a new `[[goals]]` entry into `forge.toml` with
the substituted description. `praxis run --template` runs immediately
without modifying `forge.toml`.

#### Template discovery

Templates load from three scopes (matching agents):
1. **Built-in**: a `builtin_goals()` function with common templates
   (add-endpoint, add-ci, migrate-edition, add-dashboard-view).
2. **Project**: `./.praxis/goals/*.md`
3. **User**: `~/.praxis/goals/*.md`

Project overrides user overrides built-in (same precedence as agents).

#### Relationship to the dashboard Goal Library

The dashboard's Goal Library (8B) manages saved goals in the UI. Goal
templates are the file-based version: templates are `.md` files, the
library is the UI view over them. The dashboard can render templates
with argument input fields, letting the operator fill in `$resource`
and `$methods` in the browser and click "Run".

### What this is NOT

- **Not a marketplace.** Templates are local files. Sharing is "copy the
  `.md` file." No registry, no git install. (The registry from Proposal
  2 could later distribute templates too, but templates alone don't
  need it.)
- **Not workflows.** A workflow (6C, done) is a phase sequence. A goal
  template is a parameterized goal description. A template can
  reference a workflow by name in its frontmatter.

#### Complexity assessment

| Component | Effort | Risk |
|---|---|---|
| Template `.md` parsing (reuse agent parser) | Low | AgentFrontmatter parser already exists; adapt |
| `$ARGUMENTS`/`$1`/`$2` substitution | Low | String replacement |
| `praxis goal new` CLI command | Low | New clap subcommand |
| `praxis run --template` flag | Low | Pre-run substitution |
| Template discovery (3 scopes) | Low | Reuse agent discovery pattern |
| Built-in templates | Low | A few `.md` files + `builtin_goals()` |
| Dashboard: render templates with arg fields | Medium | New UI; lower priority |

**Total: Low.** This is the simplest proposal. It reuses the agent `.md`
parsing infrastructure, adds string substitution, and a CLI command. The
core (templates + substitution + `praxis goal new`) is a small,
self-contained change.

### Verdict: **Build (medium priority)**

This is the lowest-complexity, highest-ergonomics proposal. It directly
addresses the OpenCode analysis recommendation (custom commands with
`$ARGUMENTS`, medium priority) at the goal level. It reuses existing
architecture (agent `.md` parser, 3-scope discovery). It has no security
concerns (templates are local files the operator writes or explicitly
copies). And it integrates with the existing dashboard Goal Library.

**Recommended phasing:**
1. Phase 1: Template format + `praxis goal new` + 3-scope discovery +
   3-4 built-in templates. Core value, low risk.
2. Phase 2: `praxis run --template` flag for immediate execution.
3. Phase 3: Dashboard renders templates with argument input fields.

---

## Summary

| Proposal | Verdict | Priority | Complexity | Key risk |
|---|---|---|---|---|
| **1. Multi-repo goal support** | Build | Medium | Medium | Per-repo completion criteria |
| **2. Agent/model marketplace** | Defer | Low-medium | Medium | Untrusted autonomous agent security |
| **3. Community goal templates** | Build | Medium | Low | None (local files) |

### Recommended order

1. **Goal templates (Proposal 3)** — lowest complexity, immediate
   ergonomics win, reuses existing architecture. Do first.
2. **Multi-repo (Proposal 1)** — highest value, addresses a real
   competitor-driven limitation. Phase 1 (context injection) is low
   risk; full multi-repo loops are medium.
3. **Agent marketplace (Proposal 2)** — defer until the permission/
   approval system exists. Offer `praxis agent import` (local file) as
   an interim.

### Cross-cutting dependencies

- **Proposal 2 depends on the `--require-approval` flag** (OpenCode
  analysis, low priority) and **MCP path sandboxing** (3E, in progress)
  for safe third-party agent installation.
- **Proposal 1 and 3 compose**: a goal template can target a workspace
  (`repos = ["backend", "frontend"]` in frontmatter), so "add-endpoint"
  works across repos.
- **Proposal 2 and 3 compose**: a registry could distribute goal
  templates alongside agents, using the same catalog format.

---

## Key Insights

1. **praxis's agent format is a marketplace advantage.** Claude Code
   plugins bundle skills + agents + hooks + MCP + LSP + monitors — a
   complex packaging format. praxis agents are one `.md` file. A praxis
   agent marketplace is structurally simpler, though the security bar is
   higher (autonomous loop vs. interactive chat).

2. **Multi-repo is local, not cloud.** Cursor's multi-repo works because
   cloud agents clone repos into a VM. praxis's single-binary model means
   multi-repo is a local workspace manifest — repos must be checked out
   on the same machine. This is a feature (no cloud dependency) and a
   constraint (no cross-machine coordination).

3. **Goal templates are the OpenCode custom-commands recommendation,
   realized.** The OpenCode analysis recommended `commands/` with
   `$ARGUMENTS` at medium priority. Goal templates are the praxis-native
   version: parameterized goal descriptions that feed the existing
   `praxis run --goal` flow. No new execution model, just ergonomics.

4. **Security gates the marketplace.** The single-operator model assumes
   the operator writes their own agents. Third-party agents break that
   trust model. Until praxis can restrict what an installed agent does
   (tool limits exist; approval-before-destructive-action doesn't), a
   marketplace is premature. The interim `praxis agent import` (local
   file, explicit) covers the safe case.

5. **Templates and agents share infrastructure.** Both are `.md` files
   with YAML frontmatter + Markdown body, discovered in 3 scopes. The
   agent parser (`parse_agent_md`) can be generalized to a
   `parse_md_template` that both agents and goal templates use. This
   keeps the codebase DRY and makes a future registry that distributes
   both a single feature.

6. **VISION is the filter, not the constraint.** All three proposals fit
   the single-operator, single-binary, no-SaaS model. Multi-repo is a
   workspace (not multi-tenant). The marketplace is git-based (not a
   hosted SaaS). Templates are local files (not a service). VISION
   rejects *hosted organizational features*, not *local multi-repo or
   file sharing*.
