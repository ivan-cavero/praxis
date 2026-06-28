# Configuration Reference

## forge.toml Structure

```toml
[project]
name = "my-api"
version = "0.1.0"

# ─── Providers ────────────────────────────────────────

[providers.openai]
api_key = "keyring:openai"    # Read from OS keychain
default_model = "gpt-5"
timeout_seconds = 60
max_retries = 3

[providers.anthropic]
api_key = "keyring:anthropic"
default_model = "claude-4-opus"

[providers.gemini]
api_key = "keyring:gemini"
default_model = "gemini-2.5-pro"

[providers.ollama]
base_url = "http://localhost:11434"
default_model = "llama-4"

# ─── Roles ────────────────────────────────────────────

[roles.architect]
description = "System design and architecture"
model = "claude-4-opus"
temperature = 0.2
max_tokens = 4096
system_prompt = "You are a senior software architect."
tools = ["filesystem", "web_search"]
context_profile = "generous"

[roles.coder]
description = "Code generation"
model = "gpt-5"
temperature = 0.3
max_tokens = 8192
system_prompt = "You are an expert Rust engineer."
tools = ["filesystem", "execute_command"]
context_profile = "balanced"

# ─── Goals ────────────────────────────────────────────

[[goals]]
name = "full-feature"
description = "Complete feature development"
agents = ["architect", "coder", "reviewer", "security", "tester"]
gates = ["review.pass", "security.no_critical", "test.pass"]
max_iterations = 10
parallel_reviewers = 2

# ─── Limits ───────────────────────────────────────────

[limits]
max_iterations_per_goal = 50
max_iterations_per_phase = 5
session_ttl_seconds = 3600
phase_timeout_seconds = 300
```

## Context Profiles

| Profile | System | Goal | Task | Tools | History | RAG | Project |
|---------|--------|------|------|-------|---------|-----|---------|
| balanced | 5% | 5% | 15% | 10% | 35% | 25% | 5% |
| generous | 5% | 5% | 10% | 10% | 25% | 35% | 10% |
| aggressive | 5% | 5% | 20% | 5% | 20% | 10% | 5% |
| research | 5% | 5% | 10% | 5% | 15% | 50% | 10% |

## Agent Roles

| Role | Default Model | Tools | Context Profile |
|------|---------------|-------|-----------------|
| architect | claude-4-opus | filesystem, web_search | generous |
| coder | gpt-5 | filesystem, execute_command | balanced |
| reviewer | gemini-2.5-pro | filesystem | balanced |
| security | claude-4-haiku | filesystem, grep | aggressive |
| tester | gpt-5 | filesystem, execute_command | balanced |
| researcher | gpt-5 | web_search, read_url | research |
| git | - | filesystem | balanced |

## Phase Transitions

```
Idle → Planning → Researching → Designing → Implementing → Reviewing
                                                                    ↓
                                          Fixing ← Testing ← SecurityScan
                                            ↓
                                          Finalizing → Completed
```

## Gates

| Gate | Evaluator | Description |
|------|-----------|-------------|
| review.pass | AllAgentsPass | All reviewers must pass |
| security.no_critical | NoCritical | No critical security findings |
| test.pass | AllTestsPass | All tests must pass |
| coverage.80 | CoverageThreshold | Test coverage ≥ 80% |

## Model Tiers

| Tier | Models | Use Case |
|------|--------|----------|
| Fast | gpt-4o-mini, claude-haiku | Simple tasks, formatting |
| Balanced | gpt-5, gemini-pro | Most tasks |
| Capable | claude-opus, gemini-ultra | Complex reasoning, architecture |
| Cheapest | local models | Privacy, no cost |

## Drift Recovery Actions

| ASI Score | Status | Action |
|-----------|--------|--------|
| 80-100 | Healthy | None |
| 60-79 | Attention | Log warning |
| 40-59 | Drift | Force consolidation |
| 20-39 | Critical | Model upgrade |
| 0-19 | Severe | Kill session |