# CLI Reference

## Core Commands

### `praxis init <name>`

Create a new project with default configuration.

```bash
praxis init my-api
```

Creates:
- `~/.config/praxis/projects/my-api/config.toml` — Project configuration
- `~/.config/praxis/projects/my-api/skills/` — Project skills directory
- `~/.config/praxis/projects/my-api/plans/` — Generated plans
- `~/.config/praxis/projects/my-api/injections/` — Mid-loop injections

### `praxis run`

Execute a goal with the agent system.

```bash
# Run with a goal
praxis run --goal "Create a REST API"

# Run with agent overrides
praxis run --goal "..." --agents coder,reviewer --agent coder.model=claude-4-opus

# Resume interrupted session
praxis run --resume

# Dry run (show plan without executing)
praxis run --goal "..." --dry-run

# JSON output for CI/CD
praxis run --goal "..." --headless
```

**Options:**
- `--goal <text>` — Goal description or name from config.toml
- `--file <path>` — Read goal from file
- `--resume` — Resume last interrupted session
- `--session <id>` — Resume specific session
- `--agents <list>` — Comma-separated agent names
- `--agent <key>=<value>` — Per-agent override (e.g., `coder.model=claude-4-opus`)
- `--parallel-reviewers <n>` — Number of parallel reviewers
- `--dry-run` — Show plan without executing
- `--headless` — JSON output for CI/CD

### `praxis test`

Run comprehensive integration tests.

```bash
praxis test
```

## Project Commands

```bash
praxis project list           # List all projects
praxis project show <id>      # Show project details
praxis project archive <id>   # Archive a project
```

## Session Commands

```bash
praxis session list           # List sessions
praxis session show <id>      # Show session details
praxis session stop <id>      # Stop running session
praxis session logs <id>      # View session logs
praxis session logs <id> --tail  # Stream logs in real-time
```

## Context Commands

```bash
praxis context inspect --session <id>    # Show context budget
praxis context history --session <id>    # Compression history
praxis context force-compress --session <id>  # Force EMC
```

## Provider Commands

```bash
praxis provider list           # List configured providers
praxis provider add <name>     # Add a provider (e.g., openai, anthropic)
praxis provider test <name>    # Test provider connection
```

## MCP Commands

```bash
praxis mcp list               # List connected MCP servers
# Note: MCP servers are managed via the dashboard Settings page
```

## Deploy Commands

```bash
praxis deploy setup <host>    # Configure VPS deployment (stub — not implemented)
praxis deploy push            # Push project to VPS (stub — not implemented)
praxis deploy status          # Check VPS status (stub — not implemented)
praxis deploy logs --tail     # Stream logs from VPS (stub — not implemented)
```

> **Note:** VPS deployment is not part of the core vision. Deploy manually via SSH + `praxis server`.

## Desktop & Dashboard

```bash
praxis desktop                # Open desktop app
praxis dashboard              # Open web dashboard
praxis monitor                # Open terminal UI
```

## Global Options

- `-v, --verbose` — Enable verbose output
- `--version` — Show version
- `--help` — Show help
