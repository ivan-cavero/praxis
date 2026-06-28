# CLI Reference

## Core Commands

### `project-x init <name>`

Create a new project with default configuration.

```bash
project-x init my-api
```

Creates:
- `my-api/forge.toml` — Project configuration
- `my-api/.forge/` — Project data directory
- `my-api/.gitignore`

### `project-x run`

Execute a goal with the agent system.

```bash
# Run with a goal
project-x run --goal "Create a REST API"

# Run with agent overrides
project-x run --goal "..." --agents coder,reviewer --agent coder.model=claude-4-opus

# Resume interrupted session
project-x run --resume

# Dry run (show plan without executing)
project-x run --goal "..." --dry-run

# JSON output for CI/CD
project-x run --goal "..." --headless
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

### `project-x test`

Run comprehensive integration tests.

```bash
project-x test
```

## Project Commands

```bash
project-x project list           # List all projects
project-x project show <id>      # Show project details
project-x project archive <id>   # Archive a project
```

## Session Commands

```bash
project-x session list           # List sessions
project-x session show <id>      # Show session details
project-x session stop <id>      # Stop running session
project-x session logs <id>      # View session logs
project-x session logs <id> --tail  # Stream logs in real-time
```

## Configuration Commands

```bash
project-x config show            # Show current configuration
project-x config get <key>       # Get a config value (dot notation)
project-x config set <key> <val> # Set a config value
project-x config edit            # Open config in $EDITOR
project-x config import <file>   # Import configuration
project-x config export <file>   # Export configuration
```

## Context Commands

```bash
project-x context inspect --session <id>    # Show context budget
project-x context history --session <id>    # Compression history
project-x context force-compress --session <id>  # Force EMC
```

## Provider Commands

```bash
project-x provider list           # List configured providers
project-x provider test <name>    # Test provider connection
```

## MCP Commands

```bash
project-x mcp list               # List connected MCP servers
project-x mcp add <name> <cmd>   # Add MCP server
project-x mcp remove <name>      # Remove MCP server
project-x mcp test <name>        # Test MCP server
```

## Deploy Commands

```bash
project-x deploy setup <host>    # Configure VPS deployment
project-x deploy push            # Push project to VPS
project-x deploy status          # Check VPS status
project-x deploy logs --tail     # Stream logs from VPS
```

## Desktop & Dashboard

```bash
project-x desktop                # Open desktop app
project-x dashboard              # Open web dashboard
project-x monitor                # Open terminal UI
```

## Enterprise Commands

```bash
project-x org create <name>      # Create organization
project-x org list               # List organizations
project-x org switch <id>        # Switch organization

project-x billing show           # Show billing info
project-x billing invoices       # List invoices
```

## Global Options

- `-v, --verbose` — Enable verbose output
- `--version` — Show version
- `--help` — Show help