# Quick Start Tutorial

Get from zero to running your first goal in 5 minutes.

## Step 1: Install

```bash
curl -fsSL https://project-x.dev/install.sh | bash
```

## Step 2: Create a Project

```bash
project-x init my-api
cd my-api
```

This creates:
```
my-api/
├── forge.toml      # Project configuration
├── .forge/         # Project data
└── .gitignore
```

## Step 3: Configure Your API Key

```bash
# Set your OpenAI API key
project-x config set providers.openai.api_key "sk-..."

# Or use environment variable
export OPENAI_API_KEY="sk-..."
```

## Step 4: Run Your First Goal

```bash
project-x run --goal "Create a REST API with user authentication"
```

The system will:
1. Spawn an Orchestrator
2. Activate agents: Architect → Coder → Reviewer → Security → Tester
3. Iterate until the goal is achieved
4. Commit to git when complete

## Step 5: Monitor Progress

```bash
# Open the terminal monitor
project-x monitor

# Or open the web dashboard
project-x dashboard
```

## Step 6: Check Context

```bash
# See context budget and pressure
project-x context inspect --session <session-id>

# Force compression if needed
project-x context force-compress --session <session-id>
```

## Step 7: Deploy (Optional)

```bash
# Deploy to a VPS
project-x deploy setup user@your-vps.com
project-x deploy push
```

## What Just Happened?

1. **Project created** with forge.toml defining roles (architect, coder, reviewer, security, tester)
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
- [Architecture](../adr/ARCHITECTURE.md) — How the system works internally