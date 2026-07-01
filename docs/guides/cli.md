# CLI Reference

## Core Commands

### `praxis init <name>`

Create a new project with default configuration.

```bash
praxis init my-api
```

Creates:
- `my-api/forge.toml` — Project configuration
- `my-api/.forge/` — Project data directory
- `my-api/.gitignore`

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
- `--goal <text>` — Goal description or name from forge.toml
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

## Configuration Commands

```bash
praxis config show            # Show current configuration
praxis config get <key>       # Get a config value (dot notation)
praxis config set <key> <val> # Set a config value
praxis config edit            # Open config in $EDITOR
praxis config import <file>   # Import configuration
praxis config export <file>   # Export configuration
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
praxis provider test <name>    # Test provider connection
```

## MCP Commands

```bash
praxis mcp list               # List connected MCP servers
praxis mcp add <name> <cmd>   # Add MCP server
praxis mcp remove <name>      # Remove MCP server
praxis mcp test <name>        # Test MCP server
```

## Deploy Commands

```bash
praxis deploy setup <host>    # Configure VPS deployment
praxis deploy push            # Push project to VPS
praxis deploy status          # Check VPS status
praxis deploy logs --tail     # Stream logs from VPS
```

## Desktop & Dashboard

```bash
praxis desktop                # Open desktop app
praxis dashboard              # Open web dashboard
praxis monitor                # Open terminal UI
```

## Enterprise Commands

```bash
praxis org create <name>      # Create organization
praxis org list               # List organizations
praxis org switch <id>        # Switch organization

praxis billing show           # Show billing info
praxis billing invoices       # List invoices
```

## Global Options

- `-v, --verbose` — Enable verbose output
- `--version` — Show version
- `--help` — Show help