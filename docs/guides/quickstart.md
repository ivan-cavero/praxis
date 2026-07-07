# Quick Start Tutorial

Get from zero to running your first goal in 5 minutes.

## Step 1: Install

```bash
curl -fsSL https://raw.githubusercontent.com/ivan-cavero/praxis/main/scripts/install.sh | bash
```

## Step 2: Create a Project

```bash
praxis init my-api
```

This creates a project directory at `~/.config/praxis/projects/my-api/` containing:

```
my-api/
├── config.toml      # Project configuration
├── skills/          # Project-specific skills
├── plans/           # Generated plans
└── injections/      # Mid-loop injections
```

## Step 3: Configure Your API Key

```bash
# Set your OpenAI API key
praxis provider add openai --api-key "sk-..."

# Or use environment variable
export OPENAI_API_KEY="sk-..."
```

## Step 4: Run Your First Goal

```bash
praxis run --goal "Create a REST API with user authentication"
```

The system will:
1. Spawn an Orchestrator
2. Activate agents: Architect → Coder → Reviewer → Security → Tester
3. Iterate until the goal is achieved
4. Commit to git when complete

## Step 5: Monitor Progress

```bash
# Open the terminal monitor
praxis monitor

# Or open the web dashboard
praxis dashboard
```

## Step 6: Check Context

```bash
# See context budget and pressure
praxis context inspect --session <session-id>

# Force compression if needed
praxis context force-compress --session <session-id>
```

## Step 7: Deploy (Optional)

Deployment to VPS is not yet implemented. For now, run praxis on a remote machine via SSH:

```bash
ssh user@your-vps.com
# Clone the repo, install, and run:
praxis run --goal "your goal"
```

## What Just Happened?

1. **Project created** with config.toml defining roles (architect, coder, reviewer, security, tester)
2. **Orchestrator** read the goal, spawned agents with different models
3. **Architect** designed the system structure
4. **Coder** generated code
5. **Reviewer** checked quality
6. **Security** scanned for vulnerabilities
7. **Tester** generated and ran tests
8. **Git** committed the result
9. **DriftGuard** monitored stability throughout
10. **Memory** stored everything for future sessions

## Next Steps

- [Configuration Reference](./configuration.md) — Customize agents and workflows
- [CLI Reference](./cli.md) — All available commands
- [Architecture](../adr/000-architecture-overview.md) — How the system works internally
