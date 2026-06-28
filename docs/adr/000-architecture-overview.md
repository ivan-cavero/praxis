# ADR-000: Architecture Overview

## Status: Accepted

## Context

Project-X is a multi-agent autonomous system that:
1. Takes a goal from the user
2. Spawns multiple agents with different models
3. Iterates until the goal is achieved
4. Manages context windows, memory, and drift detection
5. Provides CLI + Desktop + Web interfaces

## Decision

### Core Architecture

```
┌──────────────────────────────────────────────────┐
│                    BINARY                         │
│                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌────────┐  │
│  │   Core        │  │   MCP Host   │  │  API   │  │
│  │  (ractor)     │  │  (tokio)     │  │ (axum) │  │
│  └──────────────┘  └──────────────┘  └────────┘  │
│                                                   │
│  ┌──────────────────────────────────────────────┐  │
│  │              PERSISTENCE                     │  │
│  │  SQLite + Qdrant + moka                      │  │
│  └──────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

### Key Patterns

1. **Actor Model (Ractor):** Each agent is an actor with isolated state
2. **State Machine:** Phase transitions with quality gates
3. **Event Sourcing:** All state changes are persisted as events
4. **Three-tier Memory:** Hot (DashMap), Episodic (Qdrant), Consolidated (Summaries)
5. **Context Budget:** Never exceed 70% of model's context window

### Consequences

- **Positive:** Single binary, no external dependencies, embedded-first
- **Positive:** Cross-model verification prevents blind spots
- **Positive:** Drift detection + auto-recovery keeps agents stable
- **Negative:** More complex than single-agent systems
- **Negative:** Requires understanding of multi-agent coordination