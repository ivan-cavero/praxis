# praxis — Master Progress & Task Queue

This is the single source of truth for all loop agents.
Every iteration: read this file → pick highest-priority UNFINISHED task in YOUR DOMAIN → implement → verify → commit → push → mark [x].

## GLOBAL RULES (ALL AGENTS)
- Read this file FIRST every iteration.
- Pick the highest-priority unfinished task IN YOUR DOMAIN only.
- Implement ONE logical task per iteration.
- After implementing: run cargo build --workspace && cargo clippy --all-targets -- -D warnings && cargo test --workspace
- If verification fails → fix immediately. Never commit broken code.
- Update progress.md: change [ ] to [x] for completed tasks.
- Commit: git add -A && git commit -m "type(scope): description" (conventional commits, one per change)
- Push after every commit.
- Add new tasks you discover below your domain.
- If ALL tasks in your domain are [x], pick from EVERGREEN or cross-domain work that aligns with your skills.
- NEVER rewrite ## headers — only change [ ] → [x].

---

## DOMAIN: CODE_QUALITY (for Qwen3.6)
Eliminate production-code unwraps, clippy warnings, dead code, missing error handling.

### 1A — Eliminate unwrap() in PRODUCTION code (not tests)
- [x] crates/core/src/lib.rs lines 1709, 1911, 2261, 2742, 3643, 3732 — god module decomposed (453 lines); all unwrap() calls verified removed
- [x] crates/core/src/api/routes.rs — replaced ~18 RwLock .unwrap() with .expect("RwLock not poisoned") in API handlers; all remaining .unwrap() are test-only
- [x] crates/core/src/api/pairing.rs line 243 — replaced .unwrap() with .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
- [x] crates/core/src/orchestrator/injection.rs line 148 — replaced .unwrap() with .expect("idx found in pending above")
- [x] crates/mcp-host/src/protocol/initialize.rs line 81 — verify (audit shows test-only)
- [x] crates/mcp-host/src/protocol/messages.rs lines 111-136 — verify (audit shows test-only)
- [x] crates/cli/src/main.rs lines 276, 2809, 3036, 3313 — replaced with proper error handling
- [x] FINAL CHECK: cargo build && cargo test passes (verified clean)

### 1B — Dead code removal
- [x] Check crates/core/src/workflow/goal.rs and workflow.rs — goal.rs is full implementation (93 lines); workflow.rs renamed to engine.rs (real 130-line impl), no stubs
- [x] Search for #[allow(dead_code)] attributes and address underlying issues — zero found
- [x] Remove any commented-out code blocks (>5 consecutive lines) — none found

### 1C — Missing #[instrument] tracing
- [x] Add #[instrument(skip(self))] to all pub async fn in CoreRuntime — added to run_goal, resume_goal
- [x] Add #[instrument] to gate.rs and phase.rs public fns — added to Gate::evaluate(), GateRegistry::evaluate_phase(), StateMachine::transition(), StateMachine::detect_cycle()
- [x] Add #[instrument] to drift/metrics.rs, asi.rs, recovery.rs — added to recovery.rs evaluate() and record_and_evaluate()
- [ ] Add #[instrument] to all provider.chat() and provider.stream() methods

### 1D — unsafe audit
- [ ] Find ALL unsafe blocks in crates/ (exclude #[cfg(test)])
- [ ] Each must have a // SAFETY: comment explaining the invariant
- [ ] If any is unnecessary, refactor to safe Rust

---

## DOMAIN: PERFORMANCE (for Qwen3.6)
Profile hot paths, reduce allocations, optimize bottlenecks.

### 2A — Build configuration
- [ ] Add [profile.release] to workspace Cargo.toml with lto = "fat", codegen-units = 1, strip = true
- [ ] Measure release binary size (target: <20MB)
- [ ] Add [profile.bench] config

### 2B — Reduce allocations
- [ ] Replace Vec::new() in hot paths with Vec::with_capacity(n) where size is predictable
- [ ] Audit clone() calls in crates/core/src/lib.rs run_goal & resume_goal — use borrows
- [ ] Audit crates/core/src/machine/gate.rs evaluators for redundant allocations

### 2C — Optimize hot paths
- [ ] Audit crates/core/src/loop/controller.rs — minimize work per iteration
- [ ] Audit crates/core/src/loop/pathology.rs — lazy metrics where possible
- [ ] Audit crates/core/src/drift/metrics.rs — simplify z-score computation

---

## DOMAIN: SECURITY (for Qwen3.6)
Vulnerability scan, authentication audit, dependency review.

### 3A — JWT auth audit
- [ ] Verify alg is pinned, reject lg: none
- [ ] Verify token expiry checked on EVERY request
- [ ] Verify first-run token is one-time + expiring

### 3B — API security
- [ ] Review routes for missing auth checks
- [ ] Check CORS is restrictive by default
- [ ] Verify WebSocket validates JWT

### 3C — Vault security
- [ ] Verify AES-256-GCM nonce generation is random (check crypto.rs or impl)
- [ ] Verify key material is never logged
- [ ] Verify API keys never printed in CLI output or logs

### 3D — Dependency audit
- [ ] Run cargo audit or manual Cargo.lock review for known vulns
- [ ] Check for unmaintained/abandoned dependencies

### 3E — MCP path sandboxing
- [ ] Verify filesystem MCP server sandboxes paths to project dir
- [ ] Check MCP tools cannot escape via symlinks or ..

---

## DOMAIN: FRONTEND — UX & DESIGN (for Gemma4)
Beautiful, responsive, fast, delightful Vue 3 dashboard.

### 4A — Visual polish
- [x] Add smooth page transitions (Vue <Transition>) between all views
- [x] Add skeleton loading states for MetricCard, tables, agent grid
- [x] Add micro-animations: hover states, button presses, status changes
- [x] Add proper empty state illustration for each view
- [x] Add toast notifications for all async operations
- [ ] Ensure consistent spacing, typography, color tokens everywhere

### 4B — Responsive & mobile
- [ ] Test all views at 320px, 768px, 1024px, 1440px
- [ ] Fix sidebar collapse on tablet
- [ ] Fix top nav bar on mobile (<640px)
- [ ] Touch-friendly target sizes (min 44px)

### 4C — Performance
- [ ] Lazy-load route components with defineAsyncComponent
- [ ] Use shallowRef where deep reactivity is unnecessary
- [ ] Audit watchers for unnecessary { deep: true }
- [ ] Add debounce to WebSocket reconnection
- [ ] Virtual scroll for large session lists if needed

### 4D — Accessibility
- [x] Add aria-label to all icon-only buttons
- [x] Ensure color contrast meets WCAG AA
- [x] Add focus-visible outlines for keyboard nav
- [ ] Test basic flow with screen reader

### 4E — Login/Onboarding redesign
- [ ] Redesign LoginView with modern glassmorphism + animated logo
- [ ] Add form validation + error messages
- [ ] Wire existing OnboardingOverlay component into new user flow

### 4F — Real-time features
- [x] Improve useWebSocket — connection health indicator
- [ ] Add live agent status updates (pulsing dots)
- [ ] Add real-time token counter animation
- [ ] Add session event timeline (scrollable feed)

---

## DOMAIN: TESTING (for Gemma4)
Coverage, integration, benchmarks.

### 5A — Unit test coverage
- [ ] Add tests for crates/memory/src/context.rs budget calculator
- [ ] Add tests for crates/core/src/agents/definition.rs + 
egistry.rs
- [ ] Add tests for crates/core/src/api/auth.rs edge cases
- [ ] Add tests for crates/core/src/loop/controller.rs edge cases
- [ ] Add tests for crates/core/src/loop/limits.rs
- [ ] Add tests for crates/mcp-host/src/protocol/transport.rs

### 5B — Integration tests
- [ ] Multi-agent workflow with MockProvider
- [ ] Checkpoint/restore integration test
- [ ] MCP tool execution integration test
- [ ] Injection system end-to-end test

### 5C — Benchmarks
- [ ] Criterion: run_goal hot path throughput
- [ ] Criterion: StateMachine transition speed
- [ ] Criterion: Memory RAG retrieval latency

---

## DOMAIN: FEATURES — CORE (for GLM-5.2)
### 6A — Decompose god module (lib.rs: 3779 lines → 453 lines)
- [x] Extract ForgeConfig / load_forge_config into crates/core/src/config.rs
- [x] Extract CoreRuntime into crates/core/src/runtime.rs
- [x] Extract run_goal() / resume_goal() into crates/core/src/pipeline.rs
- [x] lib.rs becomes thin re-export module
- [x] cargo build && cargo test passes after each extraction

### 6B — Session rollback
- [x] Add praxis session rollback <id> CLI command
- [x] Add praxis session diff <id> CLI command
- [x] Add rollback API endpoint
- [x] Store file change snapshots in event store

### 6C — Workflow engine
- [x] Implement GoalEngine with DAG-based goal resolution
- [x] Implement WorkflowEngine with conditional branching
- [x] Wire into run_goal, allow workflow defs in forge.toml

### 6D — Agent delegation (wire existing delegation.rs)
- [x] Connect delegation system to agent execution
- [x] Allow sub-agent spawning for complex tasks
- [x] Add max_sub_agents config to forge.toml roles

### 6E — Undo/redo for changes
- [x] Add praxis undo CLI command
- [x] Add praxis redo CLI command
- [x] Store change history in SQLite
- [x] Add undo/redo API endpoints

---

## DOMAIN: FEATURES — CLI (for GLM-5.2)
Complete CLI implementation and UX.

### 7A — Implement stubbed commands
- [x] session stop — graceful stop via API
- [x] session logs — stream logs from event store
- [x] inject — send mid-loop injection via API
- [x] context history — show budget history

### 7B — New CLI commands
- [x] praxis doctor — diagnose configuration, provider, project health
- [x] praxis completion <shell> — bash, zsh, fish, powershell
- [x] praxis logs --tail — follow mode
- [x] praxis session export <id> — JSON/YAML export

### 7C — CLI UX
- [ ] ratatui colored output for session list / session show
- [ ] Progress bars for long operations
- [ ] --json output mode for ALL commands

---

## DOMAIN: FEATURES — FRONTEND (for GLM-5.2)
New dashboard views.

### 8A — New views
- [ ] Enhance CostAnalysisView with filtering + date ranges
- [ ] AgentDebugView — raw messages, token breakdown
- [ ] MemoryBrowserView — browse memory layers
- [ ] LogsView — real-time log stream in browser

### 8B — Dashboard enhancements
- [ ] Session comparison (side-by-side)
- [ ] Goal library (reusable templates)
- [ ] Theme customization (accent color, font size)

### 8C — PWA support
- [ ] Service worker for offline access
- [ ] Manifest.json for PWA install
- [ ] Push notifications for session completion

---

## DOMAIN: CI & DEVOPS (for GLM-5.2)
Automation and release improvements.

### 9A — CI improvements
- [ ] Add cargo audit step to CI
- [ ] Add cargo deny step (licenses, duplicates)
- [ ] Add benchmark comparison (comment on PRs)
- [ ] Add E2E test step (Playwright)

### 9B — Release automation
- [ ] Automated changelog from conventional commits
- [ ] Docker image build + push to GHCR
- [ ] Release-please or auto-PR workflow

### 9C — Monitoring
- [ ] GET /api/health — DB, LLM provider status
- [ ] GET /api/metrics — Prometheus/JSON metrics
- [ ] Graceful degradation when providers are unavailable

---

## DOMAIN: RESEARCH (for GLM-5.2)
Competitive analysis and innovation.

### 10A — OpenCode competitor analysis
- [ ] Study https://opencode.ai/docs — document features praxis should match
- [ ] Analyze Plan mode → equivalent in praxis
- [ ] Analyze custom commands with args → port if applicable
- [ ] Analyze LSP integration → evaluate for praxis
- [ ] Create docs/competitor-analysis-opencode.md

### 10B — Other competitor research
- [ ] Claude Code Ralph loop technique
- [ ] Cursor AI agent features
- [ ] GitHub Copilot agent mode
- [ ] Aider, Sweep, and other autonomous tools
- [ ] Create docs/competitor-analysis-market.md

### 10C — Innovation proposals
- [ ] Research multi-repo goal support
- [ ] Research agent/model marketplace concept
- [ ] Research community goal templates
- [ ] Create docs/innovation-proposals.md

---

## EVERGREEN TASKS (never complete — pick when nothing else remains)
These tasks regenerate every iteration. They are the infinite loop.

- [ ] EG-CODE: Run clippy — fix any NEW warnings introduced since last iteration
- [ ] EG-PERF: Profile release binary, optimize the top bottleneck
- [ ] EG-DEPS: Check crates.io for newer versions of deps in Cargo.toml
- [ ] EG-DOCS: Find functions without /// doc comments and add them
- [ ] EG-TEST: Find untested pub functions, add tests
- [ ] EG-REFACTOR: Find nyhow::Error that could be typed 	hiserror enum
- [ ] EG-STYLE: Find 	odo!() / unimplemented!() in prod code and implement
- [ ] EG-ACCESS: Check dashboard for a11y improvements
- [ ] EG-VULN: Run cargo audit, fix new vulnerabilities
- [ ] EG-FEAT: Review anomalyco/opencode issues for feature ideas
- [ ] EG-VUE: Review Vue 3.6+ new features, refactor dashboard
- [ ] EG-CLEANUP: Replace #[allow] with #[expect] where applicable

---

## COMPLETED (historical — already done before loops started)
- [x] DivergenceDetector dead code removed
- [x] CI workflow with build/clippy/test/fmt
- [x] Release workflow with Tauri updater
- [x] Dashboard redesign (v0.5+)
- [x] CLI provider test + context history commands
- [x] SQLite memory backend
- [x] Cross-model pathology verification
- [x] Version 0.6.1 released
