//! praxis CLI — Multi-Agent Autonomous System
//!
//! Usage: praxis <command> [options]
//! See `praxis help` for full documentation.

mod commands;

use clap::{Parser, Subcommand};
use colored::Colorize;
use praxis_agent_traits::persistence::EventStore;
use praxis_agent_traits::provider::{ChatConfig, ChatMessage, ChatRole, LLMProvider};
use rusqlite::params as rusqlite_params;
use std::path::PathBuf;

/// Parse a human-readable duration string (e.g., "30s", "5min", "1h", "2h30min").
fn parse_duration(s: &str) -> Option<std::time::Duration> {
    let lower = s.to_lowercase();
    let mut remaining = lower.as_str();
    let mut total_secs: u64 = 0;

    while !remaining.is_empty() {
        // Skip whitespace
        remaining = remaining.trim_start();

        // Extract the numeric part
        let num_end = remaining
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(remaining.len());
        if num_end == 0 {
            return None;
        }
        let num: u64 = remaining[..num_end].parse().ok()?;
        remaining = &remaining[num_end..];

        // Extract the unit part
        let unit_end = remaining
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or(remaining.len());
        let unit = &remaining[..unit_end];
        remaining = &remaining[unit_end..];

        let multiplier: u64 = match unit {
            "s" | "sec" | "secs" | "second" | "seconds" => 1,
            "min" | "minute" | "minutes" => 60,
            "h" | "hr" | "hrs" | "hour" | "hours" => 3600,
            "d" | "day" | "days" => 86400,
            "" => 1, // bare number = seconds
            _ => return None,
        };

        total_secs = total_secs
            .saturating_mul(1)
            .saturating_add(num.saturating_mul(multiplier));
    }

    if total_secs == 0 {
        return None;
    }
    Some(std::time::Duration::from_secs(total_secs))
}

/// Run a shell command and return true if it exits 0.
fn check_until_command(command: &str) -> bool {
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", command]);
        c
    };
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

/// Create a git worktree for an isolated session.
///
/// Creates a new worktree at `../praxis-worktree-<branch>` with a new branch.
/// Returns the worktree path. The caller must clean up via `remove_worktree`.
fn create_worktree(session_id: &str) -> Option<PathBuf> {
    // Check if we're in a git repo
    let status = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    if !status.success() {
        tracing::info!("Not in a git repo, skipping worktree creation");
        return None;
    }

    let branch_name = format!("praxis-{}", &session_id[..8.min(session_id.len())]);
    let worktree_path = std::env::current_dir().ok()?.parent()?.join(format!(
        "praxis-worktree-{}",
        &session_id[..8.min(session_id.len())]
    ));

    // Create the worktree with a new branch
    let output = std::process::Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            &branch_name,
            worktree_path.to_str()?,
            "HEAD",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        tracing::warn!(
            "Failed to create worktree: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }

    println!(
        "  {} Created worktree: {}",
        "→".dimmed(),
        worktree_path.display()
    );
    println!("  {} Branch: {}", "→".dimmed(), branch_name);
    Some(worktree_path)
}

/// Remove a git worktree and its branch.
fn remove_worktree(worktree_path: &std::path::Path) {
    let path_str = worktree_path.to_string_lossy().to_string();
    let _ = std::process::Command::new("git")
        .args(["worktree", "remove", "--force", &path_str])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    // Also delete the branch
    let branch_name = worktree_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .replace("praxis-worktree-", "praxis-");
    let _ = std::process::Command::new("git")
        .args(["branch", "-D", &branch_name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    println!(
        "  {} Removed worktree: {}",
        "→".dimmed(),
        worktree_path.display()
    );
}

/// Load vault service from .forge/credentials.vault.json if it exists.
fn load_vault() -> Option<std::sync::Arc<praxis_vault::VaultService>> {
    let vault_path = get_data_dir().join("credentials.vault.json");
    if vault_path.exists() {
        let vault = std::sync::Arc::new(praxis_vault::VaultService::with_path(
            vault_path.clone(),
            None,
        ));
        if vault.init().is_ok() {
            tracing::info!("Vault loaded from {}", vault_path.display());
            return Some(vault);
        }
    }
    None
}

/// Get the central data directory for all projects and config.
///
/// Uses `~/.config/praxis` on ALL platforms for consistency:
///   - `$PRAXIS_DATA_DIR` overrides everything
///   - `$HOME/.config/praxis` if HOME is set
///   - `%USERPROFILE%/.config/praxis` on Windows if HOME is not set
///   - `.praxis-data` in cwd as last resort
pub fn get_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PRAXIS_DATA_DIR") {
        return PathBuf::from(dir);
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config").join("praxis");
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            return PathBuf::from(userprofile).join(".config").join("praxis");
        }
    }

    PathBuf::from(".praxis-data")
}

/// Resolve the config path for a project.
///
/// If the project has a `path` field (new directory structure), returns the
/// real `config.toml` path directly — no temp file needed.
/// If the project only has `forge_toml` (legacy), writes to a temp file and
/// returns that path. Caller must clean up temp files.
fn resolve_config_path(project_name: Option<&str>) -> Option<PathBuf> {
    let data_dir = get_data_dir();
    let projects_path = data_dir.join("projects.json");
    let content = std::fs::read_to_string(projects_path).ok()?;
    let projects: Vec<serde_json::Value> = serde_json::from_str(&content).ok()?;
    if projects.is_empty() {
        return None;
    }

    let project = match project_name {
        Some(name) => projects
            .iter()
            .find(|p| p.get("name").and_then(|v| v.as_str()) == Some(name)),
        None => projects.last(),
    }?;

    // New directory structure: use the real config.toml path
    if let Some(path) = project.get("path").and_then(|v| v.as_str()) {
        let config_path = std::path::PathBuf::from(path).join("config.toml");
        if config_path.exists() {
            return Some(config_path);
        }
    }

    // Legacy: write forge_toml to a temp file
    let forge_toml = project.get("forge_toml").and_then(|v| v.as_str())?;
    let name = project
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let tmp = std::env::temp_dir().join(format!("praxis-{}.toml", name));
    std::fs::write(&tmp, forge_toml).ok()?;
    Some(tmp)
}

/// Load ForgeConfig from project, or return default empty config.
fn load_project_config(project_name: Option<&str>) -> praxis_core::ForgeConfig {
    resolve_config_path(project_name)
        .and_then(|path| praxis_core::load_forge_config(&path).ok())
        .unwrap_or_default()
}

/// Add or update a `[providers.<name>]` section in a project's config.toml.
///
/// Reads the project's config.toml, adds/replaces the provider section, and writes it back.
/// The user never has to edit TOML manually.
fn add_provider_to_project_config(
    project_name: Option<&str>,
    provider_name: &str,
    base_url: &str,
    api_key_ref: &str,
    default_model: Option<&str>,
) -> Result<String, String> {
    let data_dir = get_data_dir();
    let projects_path = data_dir.join("projects.json");
    let content = std::fs::read_to_string(&projects_path)
        .map_err(|e| format!("Cannot read projects.json: {}", e))?;
    let projects: Vec<serde_json::Value> =
        serde_json::from_str(&content).map_err(|e| format!("Cannot parse projects.json: {}", e))?;

    if projects.is_empty() {
        return Err("No projects found. Run `praxis init <name>` first.".to_string());
    }

    let project = match project_name {
        Some(name) => projects
            .iter()
            .find(|p| p.get("name").and_then(|v| v.as_str()) == Some(name))
            .ok_or_else(|| format!("Project '{}' not found", name))?,
        None => projects.last().unwrap(),
    };

    let proj_name = project
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let proj_path = project
        .get("path")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| data_dir.join("projects").join(proj_name));

    let config_path = proj_path.join("config.toml");
    let toml_content = std::fs::read_to_string(&config_path).unwrap_or_default();

    // Build the new provider section
    let mut section = format!(
        "[providers.{}]\nbase_url = \"{}\"\napi_key = \"{}\"\n",
        provider_name, base_url, api_key_ref
    );
    if let Some(model) = default_model {
        section.push_str(&format!("default_model = \"{}\"\n", model));
    }

    // Check if [providers.<name>] already exists
    let section_header = format!("[providers.{}]", provider_name);
    let updated_toml = if toml_content.contains(&section_header) {
        // Replace the existing section
        let start = toml_content.find(&section_header);
        if let Some(start_idx) = start {
            // Find the end of the section (next [ section header or EOF)
            let after_header = &toml_content[start_idx..];
            let end_idx = after_header[1..]
                .find("\n[")
                .map(|i| start_idx + 1 + i + 1)
                .unwrap_or(toml_content.len());
            let mut result = String::new();
            result.push_str(&toml_content[..start_idx]);
            result.push_str(&section);
            // Add newline if there's more content
            if end_idx < toml_content.len() {
                result.push('\n');
                result.push_str(&toml_content[end_idx..]);
            }
            result
        } else {
            toml_content
        }
    } else {
        // Append the new section
        let mut result = toml_content.clone();
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push('\n');
        result.push_str(&section);
        result
    };

    std::fs::write(&config_path, &updated_toml)
        .map_err(|e| format!("Failed to write config.toml: {}", e))?;

    Ok(proj_name.to_string())
}

#[derive(Parser)]
#[command(name = "praxis")]
#[command(
    about = "Autonomous Multi-Agent System",
    long_about = "praxis — Autonomous Multi-Agent System\n\nAn AI agent orchestration system that runs goals through a pipeline of\nspecialized agents (architect, coder, reviewer, security, tester).\n\nEXAMPLES:\n  praxis init my-project\n  praxis run --project my-project --goal \"Build a hello world CLI\"\n  praxis monitor\n  praxis dashboard\n"
)]
#[command(version = "0.5.0")]
#[command(arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new project
    ///
    /// Creates a project directory with a default config.toml, skills/, plans/,
    /// and injections/ directories.
    ///
    /// EXAMPLES:
    ///   praxis init my-app
    ///   praxis init my-project
    Init {
        /// Project name (kebab-case recommended)
        name: String,
    },

    /// Execute a goal through the agent pipeline
    ///
    /// Runs the goal through Planning → Designing → Implementing → Reviewing →
    /// Testing → SecurityScan → Finalizing phases.
    ///
    /// EXAMPLES:
    ///   praxis run --project my-app --goal "Build a hello world CLI"
    ///   praxis run --goal "Fix the login bug" --dry-run
    ///   praxis run --goal "Add tests" --until "cargo test"
    ///   praxis run --goal "Refactor API" --max-tokens 100000 --max-cost 5.0
    Run {
        /// Goal description or name
        #[arg(long)]
        goal: Option<String>,

        /// Read goal from file
        #[arg(long)]
        file: Option<PathBuf>,

        /// Project name to load config from (uses most recent if omitted)
        #[arg(long)]
        project: Option<String>,

        /// Resume last interrupted session
        #[arg(long)]
        resume: bool,

        /// Resume specific session
        #[arg(long)]
        session: Option<String>,

        /// Show plan without executing
        #[arg(long)]
        dry_run: bool,

        /// JSON output for CI/CD
        #[arg(long)]
        headless: bool,

        /// Completion criterion: "coding" (default), "manual", "stagnant=N"
        #[arg(long, default_value = "coding")]
        completion: String,

        /// Override agents (comma-separated: coder,reviewer)
        #[arg(long)]
        agents: Option<String>,

        /// Override agent properties (e.g., --agent.coder.model claude-4-opus)
        #[arg(long, action = clap::ArgAction::Append)]
        agent: Vec<String>,

        /// Number of parallel reviewers
        #[arg(long)]
        parallel_reviewers: Option<u32>,

        /// Maximum total tokens across all agents (stops the loop when exceeded)
        #[arg(long)]
        max_tokens: Option<u64>,

        /// Maximum estimated cost in USD (stops the loop when exceeded)
        #[arg(long)]
        max_cost: Option<f64>,

        /// Shell command that must exit 0 for the goal to be considered achieved
        /// (e.g., --until "cargo test" or --until "npm test")
        #[arg(long)]
        until: Option<String>,

        /// Execute a saved plan file (from `praxis plan --output <file>`)
        #[arg(long)]
        plan: Option<PathBuf>,

        /// Create a git worktree for this session (isolated working directory).
        /// The worktree is created at ../praxis-worktree-<session-id> and removed
        /// when the session completes.
        #[arg(long)]
        worktree: bool,
    },

    /// Run a goal on a repeating schedule until a condition is met
    ///
    /// Repeats the goal every <interval> until the --until command exits 0
    /// or max-runs is reached.
    ///
    /// EXAMPLES:
    ///   praxis schedule --goal "Fix failing tests" --until "cargo test" --every 30s
    ///   praxis schedule --goal "Improve coverage" --until "cargo test" --every 5min --max-runs 20
    Schedule {
        /// Goal description
        #[arg(long)]
        goal: String,

        /// Project name to load config from
        #[arg(long)]
        project: Option<String>,

        /// Time between runs (e.g., "30s", "5min", "1h", "2h30min")
        #[arg(long, default_value = "5min")]
        every: String,

        /// Shell command that must exit 0 for the schedule to stop
        /// (e.g., --until "cargo test" — schedule repeats until this passes)
        #[arg(long)]
        until: String,

        /// Maximum number of runs before giving up (default: 10)
        #[arg(long, default_value = "10")]
        max_runs: u32,

        /// Maximum total tokens across all runs combined
        #[arg(long)]
        max_tokens: Option<u64>,

        /// Maximum estimated cost in USD across all runs combined
        #[arg(long)]
        max_cost: Option<f64>,
    },

    /// Plan a goal without executing — runs Planning + Designing phases only.
    ///
    /// Produces a plan file that can be reviewed and then executed via
    /// `praxis run --plan <file>`.
    ///
    /// EXAMPLES:
    ///   praxis plan --goal "Build a REST API" --output plan.md
    ///   praxis plan --goal "Add authentication" --project my-app
    Plan {
        /// Goal description
        #[arg(long)]
        goal: String,

        /// Project name to load config from
        #[arg(long)]
        project: Option<String>,

        /// Output file for the plan (default: projects/<name>/plans/<timestamp>.md)
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Manage projects (list, show, archive)
    ///
    /// EXAMPLES:
    ///   praxis project list
    ///   praxis project show my-app
    #[command(subcommand)]
    Project(ProjectCommands),

    /// Manage agents (list, add, edit, remove)
    ///
    /// Agents are Markdown+YAML files with 3 scopes: builtin, global, project.
    ///
    /// EXAMPLES:
    ///   praxis agent list
    ///   praxis agent list --scope builtin
    ///   praxis agent show coder
    ///   praxis agent add my-agent --prompt "You are a custom agent" --model gpt-5
    ///   praxis agent edit coder
    ///   praxis agent remove my-agent
    #[command(subcommand)]
    Agent(AgentCommands),

    /// Manage sessions (list, show, stop, logs)
    ///
    /// EXAMPLES:
    ///   praxis session list
    ///   praxis session show <id>
    ///   praxis session stop <id>
    ///   praxis session logs <id> --tail
    #[command(subcommand)]
    Session(SessionCommands),

    /// LLM provider management
    ///
    /// EXAMPLES:
    ///   praxis provider list
    ///   praxis provider add openai https://api.openai.com/v1 --api-key-stdin
    ///   praxis provider test openai
    #[command(subcommand)]
    Provider(ProviderCommands),

    /// MCP server management
    ///
    /// EXAMPLES:
    ///   praxis mcp list
    ///   praxis mcp add filesystem npx -- -y @modelcontextprotocol/server-filesystem /tmp
    #[command(subcommand)]
    Mcp(McpCommands),

    /// Context management (inspect, history, compress)
    ///
    /// EXAMPLES:
    ///   praxis context inspect <session-id>
    #[command(subcommand)]
    Context(ContextCommands),

    /// Memory debugging (stats, sessions, events, checkpoints)
    ///
    /// EXAMPLES:
    ///   praxis memory stats
    ///   praxis memory sessions
    ///   praxis memory events <id>
    #[command(subcommand)]
    Memory(MemoryCommands),

    /// Inject mid-loop instructions into a running session
    ///
    /// EXAMPLES:
    ///   praxis inject --session <id> --agent coder --message "Focus on error handling"
    ///   praxis inject --session <id> --agent all --message-type halt --message "Stop now"
    Inject {
        /// Target session
        #[arg(long)]
        session: String,

        /// Target agent (or "all")
        #[arg(long)]
        agent: String,

        /// Message type: instruction|context|correction|halt
        #[arg(long, default_value = "instruction")]
        message_type: String,

        /// The instruction text
        #[arg(long)]
        message: String,
    },

    /// Open the desktop app (Tauri)
    ///
    /// Builds and launches the Tauri desktop app with the Vue dashboard.
    Desktop,

    /// Start the web dashboard (API server + opens browser)
    ///
    /// Starts the API server on port 8080 and opens the dashboard in your browser.
    /// Press Ctrl+C to stop.
    Dashboard,

    /// Start the API server (REST + WebSocket)
    ///
    /// The API server provides REST endpoints and a WebSocket for real-time events.
    ///
    /// EXAMPLES:
    ///   praxis server
    ///   praxis server --pair
    Server {
        /// Enable QR pairing system for remote connections
        #[arg(long)]
        pair: bool,
    },

    /// Open terminal UI monitor — picks the most recent session to watch
    ///
    /// Lists all sessions and watches the most recent one in real-time.
    /// If the API server is not running, starts it automatically.
    ///
    /// EXAMPLES:
    ///   praxis monitor
    Monitor,

    /// Watch a specific session's progress in real-time
    ///
    /// Polls the API server and displays session status, events, and STATE.md.
    ///
    /// EXAMPLES:
    ///   praxis watch <session-id>
    ///   praxis watch <session-id> --api http://localhost:8080 --interval 1
    Watch {
        /// Session ID to watch
        session_id: String,

        /// API server URL (default: http://localhost:8080)
        #[arg(long, default_value = "http://localhost:8080")]
        api: String,

        /// Refresh interval in seconds (default: 2)
        #[arg(long, default_value = "2")]
        interval: u64,
    },

    /// Check for a newer version of praxis
    ///
    /// Compares your installed version against the latest GitHub release.
    Update {
        /// Release channel (stable | beta)
        #[arg(long, default_value = "stable")]
        channel: String,
    },

    /// Show version
    Version,

    /// VPS deployment (placeholder — praxis runs locally as a single binary)
    ///
    /// NOTE: praxis is designed as a local-first, single-binary system.
    /// These commands provide guidance for running praxis on a remote machine
    /// but do not perform automated deployment.
    #[command(subcommand)]
    Deploy(DeployCommands),

    /// Run a comprehensive integration test (15 subsystem checks)
    Test,

    /// Diagnose your praxis setup — checks data dir, vault, database, API server
    ///
    /// EXAMPLES:
    ///   praxis doctor
    Doctor,

    /// Interactive quickstart guide — sets up your first project and provider
    ///
    /// EXAMPLES:
    ///   praxis quickstart
    Quickstart,
}

#[derive(Subcommand)]
enum ProjectCommands {
    List,
    Show { id: String },
    Archive { id: String },
}

#[derive(Subcommand)]
enum AgentCommands {
    /// List all agents (builtin + global + project)
    List {
        /// Filter by scope: all | builtin | global | project
        #[arg(long, default_value = "all")]
        scope: String,
    },

    /// Show details of a single agent
    Show {
        /// Agent name
        name: String,
    },

    /// Add a new agent (creates a .md file)
    Add {
        /// Agent name (kebab-case)
        name: String,

        /// Short description of what this agent does
        #[arg(long, default_value = "")]
        description: String,

        /// Model to use (e.g., gpt-5, claude-sonnet-4-20250514)
        #[arg(long, default_value = "gpt-5")]
        model: String,

        /// Temperature (0.0–1.0)
        #[arg(long, default_value = "0.3")]
        temperature: f32,

        /// Max tokens for response
        #[arg(long, default_value = "4096")]
        max_tokens: u32,

        /// Max LLM turns
        #[arg(long, default_value = "25")]
        max_turns: u32,

        /// Max delegation depth (0 = leaf agent)
        #[arg(long, default_value = "0")]
        max_depth: u8,

        /// Tools available to this agent (comma-separated)
        #[arg(long)]
        tools: Option<String>,

        /// Agent types this agent can spawn (comma-separated, requires max_depth > 0)
        #[arg(long)]
        can_spawn: Option<String>,

        /// System prompt (use --prompt-file for multi-line)
        #[arg(long)]
        prompt: Option<String>,

        /// Read system prompt from file
        #[arg(long)]
        prompt_file: Option<PathBuf>,

        /// Scope: project or global
        #[arg(long, default_value = "project")]
        scope: String,
    },

    /// Edit an agent's .md file in $EDITOR
    Edit {
        /// Agent name
        name: String,

        /// Scope: project or global
        #[arg(long, default_value = "project")]
        scope: String,
    },

    /// Remove an agent (only project/global scope, not builtin)
    Remove {
        /// Agent name
        name: String,

        /// Scope: project or global (default: tries project first, then global)
        #[arg(long)]
        scope: Option<String>,
    },
}

#[derive(Subcommand)]
enum SessionCommands {
    List {
        project: Option<String>,
    },
    Show {
        id: String,
    },
    Stop {
        id: String,
    },
    Logs {
        id: String,
        #[arg(long)]
        tail: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ProviderCommands {
    /// List configured LLM providers
    List,

    /// Test a provider connection
    Test {
        /// Provider name
        name: String,
    },

    /// Add a custom OpenAI-compatible provider — saves API key to vault AND wires it to the project config automatically
    ///
    /// EXAMPLES:
    ///   praxis provider add openai https://api.openai.com/v1 --api-key-stdin --model gpt-4o
    ///   praxis provider add anthropic https://api.anthropic.com/v1 --api-key sk-ant-... --model claude-sonnet-4-20250514 --project my-app
    Add {
        /// Provider name (e.g., "openai", "anthropic", "custom")
        name: String,

        /// Base URL (e.g., "https://api.openai.com/v1")
        base_url: String,

        /// API key (use --api-key to avoid shell history leakage)
        #[arg(long)]
        api_key: Option<String>,

        /// Read API key from stdin (most secure — no shell history)
        #[arg(long)]
        api_key_stdin: bool,

        /// Default model for this provider
        #[arg(long)]
        model: Option<String>,

        /// Project to wire this provider into (defaults to most recent project)
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum McpCommands {
    List,
    Add {
        name: String,
        command: String,
        args: Vec<String>,
    },
    Remove {
        name: String,
    },
    Test {
        name: String,
    },
}

#[derive(Subcommand)]
enum ContextCommands {
    Inspect { session: String },
    History { session: String },
    ForceCompress { session: String },
}

#[derive(Subcommand)]
enum MemoryCommands {
    /// Show memory and persistence statistics
    Stats,
    /// List all sessions with their checkpoint state
    Sessions,
    /// Show events for a session
    Events { id: String },
    /// Show the checkpoint for a session
    Checkpoint { id: String },
}

#[derive(Subcommand)]
enum DeployCommands {
    /// Configure VPS deployment
    Setup { host: String },
    /// Push project to VPS
    Push,
    /// Check VPS status
    Status,
    /// Stream logs from VPS
    Logs {
        #[arg(long)]
        tail: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(log_level.into()),
        )
        .init();

    match cli.command {
        // ─── Init ──────────────────────────────────────────
        Commands::Init { name } => {
            println!("{} Initializing project...", "→".cyan());
            commands::init::init_project(&name)?;
            println!();
            println!(
                "{} Project '{}' created!",
                "✓".green().bold(),
                name.green().bold()
            );
            println!();
            println!("  Next steps:");
            println!(
                "    {} --project {} --goal \"your goal here\"",
                "praxis run".yellow(),
                name
            );
            println!(
                "    {} --project {} --goal \"your goal\" --dry-run",
                "praxis run".yellow(),
                name
            );
            println!();
            println!("  Or start the dashboard:");
            println!("    {}", "praxis dashboard".yellow());
            println!();
            println!("  Check your setup:");
            println!("    {}", "praxis doctor".yellow());
        }

        // ─── Run ───────────────────────────────────────────
        Commands::Run {
            goal,
            file,
            project,
            resume,
            session: _,
            dry_run,
            headless,
            completion,
            agents,
            agent: agent_overrides,
            parallel_reviewers: _,
            max_tokens,
            max_cost,
            until,
            plan,
            worktree,
        } => {
            // If --plan is provided, load the plan file and use it as the goal
            let goal = if let Some(ref plan_path) = plan {
                match std::fs::read_to_string(plan_path) {
                    Ok(content) => {
                        println!("{} Loading plan from: {}", "→".cyan(), plan_path.display());
                        Some(content)
                    }
                    Err(e) => {
                        println!("{} Failed to read plan file: {}", "✗".red(), e);
                        std::process::exit(1);
                    }
                }
            } else {
                goal
            };

            if let Some(g) = goal {
                // Check if any project exists before running
                let data_dir = get_data_dir();
                let projects_path = data_dir.join("projects.json");
                let has_projects = std::fs::read_to_string(&projects_path)
                    .ok()
                    .and_then(|c| serde_json::from_str::<Vec<serde_json::Value>>(&c).ok())
                    .map(|p| !p.is_empty())
                    .unwrap_or(false);

                if !has_projects && project.is_none() {
                    println!(
                        "{} No project found. You need to create one first:",
                        "✗".red()
                    );
                    println!();
                    println!("  {} my-project", "praxis init".yellow());
                    println!(
                        "  {} --project my-project --goal \"your goal\"",
                        "praxis run".yellow()
                    );
                    std::process::exit(1);
                }

                // Parse agent overrides
                let mut overrides = std::collections::HashMap::new();
                for arg in &agent_overrides {
                    if let Some((key, value)) = arg.split_once('=') {
                        overrides.insert(key.to_string(), value.to_string());
                    }
                }

                // Parse agents list
                let _agents_list: Vec<String> = agents
                    .as_ref()
                    .map(|a| a.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default();

                if dry_run {
                    // Dry run: show plan without executing
                    println!("{} Goal: {}", "→".cyan(), g.white().bold());
                    println!();
                    println!("{}", "📋 Workflow Plan (dry-run)".cyan().bold());
                    println!("{}", "─".repeat(50).dimmed());

                    // Load config to show real plan
                    let config = load_project_config(project.as_deref());

                    println!();
                    println!("  {} Agents that would be spawned:", "1.".cyan());
                    for (name, role) in &config.roles {
                        println!(
                            "    {} {} ({})",
                            "•".dimmed(),
                            name.cyan(),
                            role.model.dimmed()
                        );
                    }

                    println!();
                    println!("  {} Pipeline phases:", "2.".cyan());
                    println!("    {} Planning → Designing → Implementing", "•".dimmed());
                    println!(
                        "    {} Reviewing → Testing → SecurityScan → Finalizing",
                        "•".dimmed()
                    );

                    println!();
                    println!("  {} Context Budget:", "3.".cyan());
                    println!(
                        "    {} Default: 128k context (70% hard limit)",
                        "•".dimmed()
                    );

                    println!();
                    println!("  {} Estimated Cost:", "4.".cyan());
                    let estimated_tokens: u32 = config.roles.len() as u32 * 2000;
                    println!(
                        "    {} ~{} tokens per agent ({} agents)",
                        "•".dimmed(),
                        estimated_tokens,
                        config.roles.len()
                    );

                    println!();
                    println!("  {} Hard Limits:", "5.".cyan());
                    println!("    {} Max iterations: 50", "•".dimmed());
                    println!("    {} Session TTL: 60 min", "•".dimmed());
                    println!("    {} Phase timeout: 5 min", "•".dimmed());

                    // Show overrides if any
                    if !overrides.is_empty() {
                        println!();
                        println!("  {} Overrides:", "6.".cyan());
                        for (key, value) in &overrides {
                            println!("    {} {} = {}", "•".dimmed(), key, value);
                        }
                    }

                    println!();
                    println!("{} Run without --dry-run to execute", "→".cyan());
                } else if headless {
                    // Headless: JSON output
                    println!("{} Running in headless mode", "→".cyan());

                    // Create worktree if requested
                    let worktree_path = if worktree {
                        let session_id = uuid::Uuid::new_v4().to_string();
                        create_worktree(&session_id)
                    } else {
                        None
                    };

                    // Change to worktree directory if created
                    let mut runtime = praxis_core::CoreRuntime::new()
                        .await?
                        .with_default_memory()
                        .with_state_file()
                        .with_skills();

                    // Set project name on runtime for checkpoint metadata
                    if let Some(ref name) = project {
                        runtime = runtime.with_project_name(name.clone());
                    }

                    // Apply completion criterion: --until takes priority, then --completion
                    if let Some(ref cmd) = until {
                        runtime = runtime.with_completion(
                            praxis_core::CompletionCriterion::from_until_command(cmd.clone()),
                        );
                    } else if completion != "coding"
                        && let Some(criterion) =
                            praxis_core::CompletionCriterion::from_string(&completion)
                    {
                        runtime = runtime.with_completion(criterion);
                    }

                    // Apply token/cost budget overrides from CLI
                    if max_tokens.is_some() || max_cost.is_some() {
                        runtime.loop_controller.limits.max_tokens = max_tokens;
                        runtime.loop_controller.limits.max_cost_usd = max_cost;
                    }

                    // Load vault if exists
                    let vault = load_vault();

                    // Resolve project config path (temp file, cleaned up after)
                    let config_path = resolve_config_path(project.as_deref());
                    if let Some(ref path) = config_path {
                        println!(
                            "  {} Using project config: {}",
                            "→".dimmed(),
                            path.display()
                        );
                    }

                    let result = runtime
                        .run_goal(&g, config_path.as_deref(), vault.as_deref())
                        .await?;

                    // Clean up temp config file
                    if let Some(path) = config_path {
                        let _ = std::fs::remove_file(path);
                    }

                    // Clean up worktree if created
                    if let Some(ref wt_path) = worktree_path {
                        remove_worktree(wt_path);
                    }

                    let json_result = serde_json::json!({
                        "status": if result.passed { "completed" } else { "failed" },
                        "goal": result.goal,
                        "agents_executed": result.agent_results.len(),
                        "passed": result.passed,
                        "total_duration_ms": result.total_duration_ms,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });
                    println!("{}", serde_json::to_string_pretty(&json_result)?);

                    let _ = runtime.shutdown().await;
                } else {
                    // Normal execution with live streaming output
                    println!("{} {}", "→ Running goal:".cyan(), g.white().bold());
                    println!("  Press Ctrl+C to stop gracefully");
                    println!();

                    // Create worktree if requested
                    let worktree_path = if worktree {
                        let session_id = uuid::Uuid::new_v4().to_string();
                        create_worktree(&session_id)
                    } else {
                        None
                    };

                    println!("{}", "📦 Starting core runtime...".dimmed());
                    let data_dir = get_data_dir();
                    std::fs::create_dir_all(&data_dir)?;
                    let db_path = data_dir.join("state.db");

                    let store = praxis_persistence::SqliteEventStore::new(&db_path)
                        .map_err(|e| anyhow::anyhow!(e))?;
                    let pool = store.pool().clone();
                    let mut runtime = praxis_core::CoreRuntime::new()
                        .await?
                        .with_sqlite_memory(pool)
                        .with_event_store(store)
                        .with_state_file()
                        .with_skills();

                    // Apply completion criterion: --until takes priority, then --completion
                    if let Some(ref cmd) = until {
                        runtime = runtime.with_completion(
                            praxis_core::CompletionCriterion::from_until_command(cmd.clone()),
                        );
                        println!("  {} Until: {}", "→".dimmed(), cmd.cyan());
                    } else if completion != "coding"
                        && let Some(criterion) =
                            praxis_core::CompletionCriterion::from_string(&completion)
                    {
                        runtime = runtime.with_completion(criterion);
                        println!("  {} Completion: {}", "→".dimmed(), completion);
                    }

                    // Apply token/cost budget overrides from CLI
                    if max_tokens.is_some() || max_cost.is_some() {
                        runtime.loop_controller.limits.max_tokens = max_tokens;
                        runtime.loop_controller.limits.max_cost_usd = max_cost;
                        if let Some(mt) = max_tokens {
                            println!("  {} Token budget: {} tokens", "→".dimmed(), mt);
                        }
                        if let Some(mc) = max_cost {
                            println!("  {} Cost budget: ${:.4}", "→".dimmed(), mc);
                        }
                    }

                    // Load vault if exists
                    let vault = load_vault();

                    // Set up Ctrl+C handler for graceful shutdown
                    let shutdown_flag = runtime.shutdown_handle();
                    tokio::spawn(async move {
                        let _ = tokio::signal::ctrl_c().await;
                        println!(
                            "\n{} Ctrl+C received. Finishing current iteration and saving checkpoint...",
                            "⚠".yellow()
                        );
                        shutdown_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    });

                    // Subscribe to EventBus for live streaming of agent output
                    let event_bus = runtime.bus.clone();
                    let event_printer = tokio::spawn(async move {
                        let mut rx = event_bus.subscribe();
                        loop {
                            match rx.recv().await {
                                Ok(event) => {
                                    use praxis_shared::protocol::MessageKind;
                                    match &event.kind {
                                        MessageKind::AgentStarted {
                                            agent,
                                            role: _,
                                            phase,
                                        } => {
                                            println!(
                                                "  {} {} ({}) started",
                                                "→".cyan(),
                                                agent.cyan(),
                                                format!("{:?}", phase).dimmed()
                                            );
                                        }
                                        MessageKind::AgentOutput { agent: _, delta } => {
                                            // Print each line of the delta as it arrives
                                            for line in delta.lines() {
                                                if !line.is_empty() {
                                                    println!("    │ {}", line.dimmed());
                                                }
                                            }
                                        }
                                        MessageKind::AgentCompleted {
                                            agent,
                                            status,
                                            duration_ms,
                                            output_preview,
                                            ..
                                        } => {
                                            let preview = if output_preview.len() > 80 {
                                                format!("{}...", &output_preview[..80])
                                            } else {
                                                output_preview.clone()
                                            };
                                            println!(
                                                "  {} {} completed — {} ({}ms)",
                                                "✓".green(),
                                                agent.cyan(),
                                                status.dimmed(),
                                                duration_ms
                                            );
                                            println!("    {}", preview.dimmed());
                                        }
                                        MessageKind::PhaseChanged(transition) => {
                                            println!(
                                                "  {} Phase: {:?} → {:?}",
                                                "▶".cyan(),
                                                transition.from,
                                                transition.to
                                            );
                                        }
                                        MessageKind::PathologyDetected(alert) => {
                                            println!(
                                                "  {} Pathology: {} ({})",
                                                "⚠".yellow(),
                                                alert.details.dimmed(),
                                                alert.severity
                                            );
                                        }
                                        MessageKind::CheckpointSaved(info) => {
                                            println!(
                                                "  {} Checkpoint saved (iteration {})",
                                                "💾".dimmed(),
                                                info.iteration
                                            );
                                        }
                                        MessageKind::GateResult(result) => {
                                            println!(
                                                "  {} Gate: {} — {}",
                                                "🔍".dimmed(),
                                                result.gate_name,
                                                if result.passed {
                                                    "PASS".green()
                                                } else {
                                                    "FAIL".red()
                                                }
                                            );
                                        }
                                        _ => {}
                                    }
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                    tracing::warn!("EventBus subscriber lagged by {} events", n);
                                }
                            }
                        }
                    });

                    println!("{}", "🤖 Initializing agent pipeline...".dimmed());

                    // Resolve project config path (temp file, cleaned up after)
                    let config_path = resolve_config_path(project.as_deref());
                    if let Some(ref path) = config_path {
                        println!(
                            "  {} Using project config: {}",
                            "→".dimmed(),
                            path.display()
                        );
                    }

                    // Set project name on runtime for checkpoint metadata
                    if let Some(ref name) = project {
                        runtime = runtime.with_project_name(name.clone());
                    }

                    // Run through the full agent pipeline
                    let result = runtime
                        .run_goal(&g, config_path.as_deref(), vault.as_deref())
                        .await?;

                    // Clean up temp config file
                    if let Some(path) = config_path {
                        let _ = std::fs::remove_file(path);
                    }

                    // Clean up worktree if created
                    if let Some(ref wt_path) = worktree_path {
                        remove_worktree(wt_path);
                    }

                    // Stop the event printer
                    event_printer.abort();

                    println!();
                    println!("  {} Goal: {}", "→".cyan(), result.goal.white().bold());
                    println!(
                        "  {} Status: {}",
                        "→".cyan(),
                        if result.passed {
                            "✅ PASSED".green().bold()
                        } else {
                            "❌ FAILED".red().bold()
                        }
                    );
                    println!(
                        "  {} Agents executed: {}",
                        "→".cyan(),
                        result.agent_results.len()
                    );
                    for agent_result in &result.agent_results {
                        println!(
                            "    {} {} ({}) — {:?} — {}ms",
                            "•".dimmed(),
                            agent_result.agent_id.cyan(),
                            agent_result.role,
                            agent_result.status,
                            agent_result.duration_ms,
                        );
                    }
                    println!(
                        "  {} Total duration: {}ms",
                        "→".cyan(),
                        result.total_duration_ms
                    );

                    if let Some(sid) = runtime.session_id {
                        println!("  {} Session: {}", "→".cyan(), sid);
                    }

                    println!();
                    println!("{}", "🔌 Shutting down...".dimmed());
                    runtime.shutdown().await?;
                    println!("{} Done", "✓".green().bold());
                }
            } else if let Some(f) = file {
                let content = std::fs::read_to_string(&f)?;
                println!("{} Reading goal from: {}", "→".cyan(), f.display());
                println!("  {}", content.trim().dimmed());
                println!("{}", "⚠ File-based goals not yet implemented".yellow());
            } else if resume {
                println!("{} Resuming last session...", "→".cyan());
                let data_dir = get_data_dir();
                let db_path = data_dir.join("state.db");

                if !db_path.exists() {
                    println!(
                        "  {} No database found at {}. Run a session first.",
                        "✗".red(),
                        db_path.display()
                    );
                    std::process::exit(1);
                }

                let store = praxis_persistence::SqliteEventStore::new(&db_path)
                    .map_err(|e| anyhow::anyhow!(e))?;

                // Find the last session
                let session_ids = store
                    .list_aggregates("session")
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;

                let session_id = match session_ids.last() {
                    Some(id) => *id,
                    None => {
                        println!("  {} No sessions found to resume.", "✗".red());
                        std::process::exit(1);
                    }
                };

                println!("  {} Resuming session: {}", "→".dimmed(), session_id);

                let mut runtime = praxis_core::CoreRuntime::new()
                    .await?
                    .with_default_memory()
                    .with_event_store(store)
                    .with_state_file()
                    .with_skills();

                let vault = load_vault();

                // Set up Ctrl+C handler
                let shutdown_flag = runtime.shutdown_handle();
                tokio::spawn(async move {
                    let _ = tokio::signal::ctrl_c().await;
                    println!(
                        "\n{} Ctrl+C received. Finishing current iteration and saving checkpoint...",
                        "⚠".yellow()
                    );
                    shutdown_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                });

                let result = runtime
                    .resume_goal(session_id, None, vault.as_deref())
                    .await?;

                match result {
                    Some(result) => {
                        println!();
                        println!("  {} Goal: {}", "→".cyan(), result.goal.white().bold());
                        println!(
                            "  {} Status: {}",
                            "→".cyan(),
                            if result.passed {
                                "✅ PASSED".green().bold()
                            } else {
                                "❌ FAILED".red().bold()
                            }
                        );
                        println!(
                            "  {} Agents executed: {}",
                            "→".cyan(),
                            result.agent_results.len()
                        );
                        println!(
                            "  {} Total duration: {}ms",
                            "→".cyan(),
                            result.total_duration_ms
                        );
                    }
                    None => {
                        println!(
                            "  {} No checkpoint found for session {}.",
                            "✗".red(),
                            session_id
                        );
                    }
                }

                println!();
                println!("{}", "🔌 Shutting down...".dimmed());
                runtime.shutdown().await?;
                println!("{} Done", "✓".green().bold());
            } else {
                println!("{} Please provide --goal, --file, or --resume", "✗".red());
                std::process::exit(1);
            }
        }

        // ─── Version ───────────────────────────────────────
        Commands::Version => {
            println!("{} v{}", "praxis".cyan().bold(), env!("CARGO_PKG_VERSION"));
        }

        // ─── Test ──────────────────────────────────────────
        Commands::Test => {
            println!("{}", "🧪 Sprint 0.1–0.5 Integration Test".cyan().bold());
            println!("{}", "═".repeat(50).dimmed());
            println!();

            print!("  {} EventBus... ", "1.".cyan());
            let bus = praxis_core::EventBus::new();
            println!("{} capacity={}", "✓".green(), bus.capacity());

            print!("  {} CoreRuntime... ", "2.".cyan());
            let runtime = praxis_core::CoreRuntime::new().await?;
            println!("{}", "✓".green());

            print!("  {} Spawning 3 agents... ", "3.".cyan());
            for i in 0..3 {
                runtime.spawn_echo_agent(&format!("agent-{}", i)).await?;
            }
            println!("{} spawned", "✓".green());

            print!("  {} Echo messages... ", "4.".cyan());
            for i in 0..3 {
                let response = runtime
                    .echo_to(&format!("agent-{}", i), &format!("msg-{}", i))
                    .await?;
                if !response.contains("echo") {
                    anyhow::bail!("Unexpected response: {}", response);
                }
            }
            println!("{} all received", "✓".green());

            print!("  {} List agents... ", "5.".cyan());
            let agents = runtime.list_agents().await?;
            if agents.len() != 3 {
                anyhow::bail!("Expected 3 agents, got {}", agents.len());
            }
            println!("{} {} agents", "✓".green(), agents.len());

            use praxis_agent_traits::persistence::EventStore;
            print!("  {} SQLite event store... ", "6.".cyan());
            let store = praxis_persistence::SqliteEventStore::in_memory()
                .map_err(|e| anyhow::anyhow!(e))?;
            let agg_id = uuid::Uuid::new_v4();
            let event = praxis_agent_traits::persistence::StoredEvent {
                id: uuid::Uuid::new_v4(),
                aggregate_id: agg_id,
                aggregate_type: "test".to_string(),
                event_type: "test.event".to_string(),
                payload: serde_json::json!({"hello": "world"}),
                metadata: serde_json::json!({}),
                version: 1,
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            store.append(event).await?;
            let events = store.read_events(agg_id, None).await?;
            if events.len() != 1 {
                anyhow::bail!("Expected 1 event, got {}", events.len());
            }
            println!("{} append+read ok", "✓".green());

            print!("  {} Snapshots... ", "7.".cyan());
            let snapshot = praxis_agent_traits::persistence::StoredSnapshot {
                aggregate_id: agg_id,
                aggregate_type: "test".to_string(),
                state: serde_json::json!({"phase": "testing"}),
                version: 1,
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            store.save_snapshot(snapshot).await?;
            let loaded = store.get_snapshot(agg_id).await?;
            if loaded.is_none() {
                anyhow::bail!("Snapshot not found");
            }
            println!("{} save+load ok", "✓".green());

            print!("  {} ProviderRouter... ", "8.".cyan());
            let mut router = praxis_providers::ProviderRouter::new();
            let mock: std::sync::Arc<dyn praxis_providers::LLMProvider> =
                std::sync::Arc::new(praxis_providers::MockProvider::simple("test"));
            router.register("mock", mock, praxis_providers::ModelTier::Balanced);
            let resolved = router.resolve("mock").map_err(|e| anyhow::anyhow!(e))?;
            if resolved.provider_name() != "mock" {
                anyhow::bail!("Router failed");
            }
            println!("{} resolve ok", "✓".green());

            print!("  {} StateMachine... ", "9.".cyan());
            let mut sm = praxis_core::StateMachine::new();
            sm.transition(praxis_core::machine::phase::Phase::Planning, 0)
                .map_err(|e| anyhow::anyhow!(e))?;
            sm.transition(praxis_core::machine::phase::Phase::Implementing, 1)
                .map_err(|e| anyhow::anyhow!(e))?;
            sm.transition(praxis_core::machine::phase::Phase::Reviewing, 2)
                .map_err(|e| anyhow::anyhow!(e))?;
            sm.transition(praxis_core::machine::phase::Phase::Testing, 3)
                .map_err(|e| anyhow::anyhow!(e))?;
            sm.transition(praxis_core::machine::phase::Phase::Finalizing, 4)
                .map_err(|e| anyhow::anyhow!(e))?;
            sm.transition(praxis_core::machine::phase::Phase::Completed, 5)
                .map_err(|e| anyhow::anyhow!(e))?;
            if sm.current() != praxis_core::machine::phase::Phase::Completed {
                anyhow::bail!("State machine failed");
            }
            println!("{} full flow ok", "✓".green());

            print!("  {} LoopController... ", "10.".cyan());
            let mut ctrl = praxis_core::LoopController::new();
            ctrl.start();
            ctrl.advance(praxis_core::machine::phase::Phase::Planning)
                .map_err(|e| anyhow::anyhow!(e))?;
            ctrl.increment_iteration();
            ctrl.advance(praxis_core::machine::phase::Phase::Implementing)
                .map_err(|e| anyhow::anyhow!(e))?;
            ctrl.increment_iteration();
            ctrl.advance(praxis_core::machine::phase::Phase::Reviewing)
                .map_err(|e| anyhow::anyhow!(e))?;
            ctrl.advance(praxis_core::machine::phase::Phase::Completed)
                .map_err(|e| anyhow::anyhow!(e))?;
            if !ctrl.phase_info().current.is_terminal() {
                anyhow::bail!("Loop controller failed");
            }
            println!("{} ok", "✓".green());

            print!("  {} DriftGuard... ", "11.".cyan());
            let mut drift = praxis_core::DriftGuard::new();
            for i in 0..12 {
                drift.record_and_evaluate(
                    praxis_core::drift::metrics::MetricSample {
                        iteration: i,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        latency_ms: 100,
                        output_tokens: 50,
                        input_tokens: 100,
                        tool_calls: 2,
                        tool_errors: 0,
                        output_length_chars: 200,
                        gate_passed: true,
                        context_pressure: 0.3,
                    },
                    None,
                );
            }
            println!("{} metrics ok", "✓".green());

            print!("  {} HotMemory... ", "12.".cyan());
            let _mem = praxis_core::EventBus::new();
            let hot = praxis_memory::HotMemory::new();
            hot.create_session("test-s1", "test-p1", "test goal");
            hot.push_interaction(
                "test-s1",
                "coder",
                praxis_memory::Interaction {
                    role: "user".to_string(),
                    content: "test".to_string(),
                    token_count: 5,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                },
            );
            let ctx = hot.get_context("test-s1", "coder");
            if ctx.is_none() {
                anyhow::bail!("HotMemory context failed");
            }
            println!("{} session+context ok", "✓".green());

            print!("  {} LLMCache... ", "13.".cyan());
            let cache = praxis_memory::LLMCache::default_cache();
            let key = praxis_memory::LLMCache::key("gpt-5", &["test".to_string()], 0.3);
            cache.insert(
                key,
                praxis_memory::CachedResponse {
                    content: "test".to_string(),
                    model: "gpt-5".to_string(),
                    input_tokens: 5,
                    output_tokens: 3,
                    cached_at: std::time::Instant::now(),
                },
            );
            let cached = cache.get(&key);
            if cached.is_none() {
                anyhow::bail!("LLMCache failed");
            }
            println!("{} insert+get ok", "✓".green());

            print!("  {} ContextManager... ", "14.".cyan());
            let mut ctx_mgr =
                praxis_memory::ContextManager::new(128_000, praxis_memory::BudgetProfile::Balanced);
            let mut ctx_window = praxis_memory::ContextWindow::new();
            ctx_window.push(praxis_memory::Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
            });
            let prepared = ctx_mgr.prepare(&mut ctx_window);
            if prepared.is_empty() {
                anyhow::bail!("ContextManager failed");
            }
            println!("{} ok (health: {:?})", "✓".green(), ctx_mgr.health_status());

            print!("  {} Shutdown... ", "15.".cyan());
            let _ = runtime.shutdown().await;
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            println!("{} graceful", "✓".green());

            println!();
            println!("{} All 15 tests passed!", "🎉".green().bold());
        }

        // ─── Subcommands ───────────────────────────────────
        Commands::Agent(cmd) => {
            commands::agent::handle(cmd);
        }

        Commands::Project(cmd) => match cmd {
            ProjectCommands::List => {
                let data_dir = get_data_dir();
                let projects_path = data_dir.join("projects.json");
                match std::fs::read_to_string(&projects_path) {
                    Ok(content) => {
                        let projects: Vec<serde_json::Value> =
                            serde_json::from_str(&content).unwrap_or_default();
                        if projects.is_empty() {
                            println!("{} No projects found", "→".cyan());
                        } else {
                            println!("{} Projects ({})", "→".cyan(), projects.len());
                            for project in &projects {
                                let name =
                                    project.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                                let id = project.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                                let created = project
                                    .get("created_at")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("?");
                                let desc = project
                                    .get("description")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                println!("  {} {} ({})", "•".dimmed(), name.cyan(), id.dimmed());
                                if !desc.is_empty() {
                                    println!("    {}", desc.dimmed());
                                }
                                println!("    Created: {}", created.dimmed());
                            }
                        }
                    }
                    Err(_) => {
                        println!("{} No projects found (no projects.json)", "→".cyan());
                        println!(
                            "  {} Run {} to create one",
                            "→".dimmed(),
                            "praxis init <name>".yellow()
                        );
                    }
                }
            }
            ProjectCommands::Show { id } => {
                let data_dir = get_data_dir();
                let projects_path = data_dir.join("projects.json");
                let content = std::fs::read_to_string(&projects_path)
                    .map_err(|e| anyhow::anyhow!("Failed to read projects.json: {}", e))?;
                let projects: Vec<serde_json::Value> = serde_json::from_str(&content)
                    .map_err(|e| anyhow::anyhow!("Failed to parse projects.json: {}", e))?;
                let project = projects
                    .iter()
                    .find(|p| {
                        p.get("id").and_then(|v| v.as_str()) == Some(&id)
                            || p.get("name").and_then(|v| v.as_str()) == Some(&id)
                    })
                    .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", id))?;

                println!(
                    "  {} Name: {}",
                    "→".cyan(),
                    project
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .cyan()
                );
                println!(
                    "  {} ID: {}",
                    "→".cyan(),
                    project
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .dimmed()
                );
                if let Some(desc) = project.get("description").and_then(|v| v.as_str())
                    && !desc.is_empty()
                {
                    println!("  {} Description: {}", "→".cyan(), desc);
                }
                println!(
                    "  {} Created: {}",
                    "→".cyan(),
                    project
                        .get("created_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                );
                if let Some(toml) = project.get("forge_toml").and_then(|v| v.as_str()) {
                    println!();
                    println!("  {} forge.toml:", "→".cyan());
                    println!("{}", "─".repeat(50).dimmed());
                    println!("{}", toml.dimmed());
                }
            }
            ProjectCommands::Archive { id } => {
                println!("{} Archiving project: {}", "→".cyan(), id);
                println!(
                    "  {} (archive not yet implemented — use dashboard to delete)",
                    "→".dimmed()
                );
            }
        },

        Commands::Session(cmd) => match cmd {
            SessionCommands::List { project: _ } => {
                let data_dir = get_data_dir();
                let db_path = data_dir.join("state.db");
                if !db_path.exists() {
                    println!("{} No database found. Run a session first.", "→".cyan());
                } else {
                    let store = praxis_persistence::SqliteEventStore::new(&db_path)
                        .map_err(|e| anyhow::anyhow!(e))?;
                    let session_ids = store
                        .list_aggregates("session")
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                    if session_ids.is_empty() {
                        println!("{} No sessions found", "→".cyan());
                    } else {
                        println!("{} Sessions ({})", "→".cyan(), session_ids.len());
                        for sid in &session_ids {
                            // Load snapshot for metadata
                            let snapshot = store
                                .get_snapshot(*sid)
                                .await
                                .map_err(|e| anyhow::anyhow!(e))?;
                            if let Some(snap) = snapshot {
                                let goal = snap
                                    .state
                                    .get("goal")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("?");
                                let phase = snap
                                    .state
                                    .get("phase")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("?");
                                let iteration = snap
                                    .state
                                    .get("iteration")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                println!(
                                    "  {} {} — {} (iter {}) — {}",
                                    "•".dimmed(),
                                    sid.to_string().dimmed(),
                                    goal.cyan(),
                                    iteration,
                                    phase.dimmed(),
                                );
                            } else {
                                println!("  {} {} (no checkpoint)", "•".dimmed(), sid);
                            }
                        }
                    }
                }
            }
            SessionCommands::Show { id } => {
                let data_dir = get_data_dir();
                let db_path = data_dir.join("state.db");
                if !db_path.exists() {
                    println!("{} No database found.", "→".cyan());
                } else {
                    let sid = uuid::Uuid::parse_str(&id)
                        .map_err(|e| anyhow::anyhow!("Invalid session ID: {}", e))?;
                    let store = praxis_persistence::SqliteEventStore::new(&db_path)
                        .map_err(|e| anyhow::anyhow!(e))?;
                    let snapshot = store
                        .get_snapshot(sid)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                    match snapshot {
                        Some(snap) => {
                            println!("  {} Session: {}", "→".cyan(), sid);
                            println!("  {} Type: {}", "→".cyan(), snap.aggregate_type);
                            println!("  {} Version: {}", "→".cyan(), snap.version);
                            println!("  {} Updated: {}", "→".cyan(), snap.updated_at);
                            if let Some(goal) = snap.state.get("goal").and_then(|v| v.as_str()) {
                                println!("  {} Goal: {}", "→".cyan(), goal);
                            }
                            if let Some(phase) = snap.state.get("phase").and_then(|v| v.as_str()) {
                                println!("  {} Phase: {}", "→".cyan(), phase);
                            }
                            if let Some(iteration) =
                                snap.state.get("iteration").and_then(|v| v.as_u64())
                            {
                                println!("  {} Iteration: {}", "→".cyan(), iteration);
                            }
                        }
                        None => {
                            println!("{} No checkpoint found for session {}", "→".cyan(), id);
                        }
                    }
                }
            }
            SessionCommands::Stop { id } => {
                // Try to stop via the API server
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(5))
                    .build()
                    .unwrap_or_default();

                let api_url = "http://localhost:8080";
                let stop_url = format!("{}/api/sessions/{}/stop", api_url, id);

                match client.post(&stop_url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        println!("{} Stop signal sent to session {}", "✓".green(), id);
                        println!("  The session will stop after the current iteration.");
                    }
                    Ok(resp) if resp.status() == reqwest::StatusCode::NOT_FOUND => {
                        println!("{} Session {} not found on the API server.", "✗".red(), id);
                        println!(
                            "  Is the API server running? Start it with: {}",
                            "praxis server".cyan()
                        );
                    }
                    Ok(resp) => {
                        println!(
                            "{} API server returned status {}",
                            "⚠".yellow(),
                            resp.status()
                        );
                    }
                    Err(e) if e.is_connect() => {
                        println!("{} Cannot reach API server at {}", "✗".red(), api_url);
                        println!("  Start it with: {}", "praxis server".cyan());
                        println!();
                        println!(
                            "  If the session is running in this terminal, press Ctrl+C to stop it."
                        );
                    }
                    Err(e) => {
                        println!("{} Error stopping session: {}", "✗".red(), e);
                    }
                }
            }
            SessionCommands::Logs { id, tail, json } => {
                let data_dir = get_data_dir();
                let db_path = data_dir.join("state.db");

                if !db_path.exists() {
                    println!("{} No database found. Run a session first.", "→".cyan());
                    std::process::exit(1);
                }

                let sid = uuid::Uuid::parse_str(&id)
                    .map_err(|e| anyhow::anyhow!("Invalid session ID: {}", e))?;
                let store = praxis_persistence::SqliteEventStore::new(&db_path)
                    .map_err(|e| anyhow::anyhow!(e))?;
                let events = store
                    .read_events(sid, None)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;

                if events.is_empty() {
                    println!("{} No events found for session {}", "→".cyan(), id);
                    std::process::exit(0);
                }

                if json {
                    let json_events: Vec<_> = events
                        .iter()
                        .map(|e| {
                            serde_json::json!({
                                "id": e.id,
                                "type": e.event_type,
                                "version": e.version,
                                "created_at": e.created_at,
                                "payload": e.payload,
                            })
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&json_events)?);
                } else {
                    let mut display_events: Vec<_> = if tail {
                        events.iter().rev().take(50).collect()
                    } else {
                        events.iter().collect()
                    };
                    if tail {
                        display_events.reverse();
                    }

                    println!(
                        "{} Logs for session {} ({} events)",
                        "→".cyan(),
                        id,
                        events.len()
                    );
                    println!("{}", "─".repeat(80));
                    for event in &display_events {
                        let time: String = event.created_at.chars().skip(11).take(8).collect();
                        println!(
                            "  {} {} {}",
                            time.dimmed(),
                            event.event_type.cyan(),
                            event.version
                        );
                        if let Ok(pretty) = serde_json::to_string_pretty(&event.payload) {
                            for line in pretty.lines().take(3) {
                                println!("    {}", line.dimmed());
                            }
                        }
                    }
                }
            }
        },

        Commands::Provider(cmd) => match cmd {
            ProviderCommands::List => {
                // Read providers from the most recent project's forge.toml
                let config = load_project_config(None);
                if config.providers.is_empty() {
                    println!("{} No providers configured", "→".cyan());
                    println!(
                        "  {} Use {} to add one",
                        "→".dimmed(),
                        "praxis provider add <name> <base_url> --api-key-stdin".yellow()
                    );
                } else {
                    println!(
                        "{} Configured providers ({})",
                        "→".cyan(),
                        config.providers.len()
                    );
                    for (name, provider) in &config.providers {
                        let key_status = if provider.api_key_ref.starts_with("env:") {
                            format!(
                                "env:{}",
                                provider.api_key_ref.strip_prefix("env:").unwrap_or("?")
                            )
                        } else if provider.api_key_ref.starts_with("keyring:")
                            || provider.api_key_ref.starts_with("vault:")
                        {
                            "vault".to_string()
                        } else if provider.api_key_ref.is_empty() {
                            "none".to_string()
                        } else {
                            "literal".to_string()
                        };
                        println!(
                            "  {} {} — {} ({})",
                            "•".dimmed(),
                            name.cyan(),
                            provider.default_model.dimmed(),
                            key_status.yellow()
                        );
                        println!("    Base URL: {}", provider.base_url.dimmed());
                    }
                }
                println!();
                println!("  Supported APIs:");
                println!("    {} OpenAI (api.openai.com)", "•".dimmed());
                println!("    {} Anthropic (api.anthropic.com)", "•".dimmed());
                println!(
                    "    {} Google AI (generativelanguage.googleapis.com)",
                    "•".dimmed()
                );
                println!(
                    "    {} Any OpenAI-compatible API (custom base_url)",
                    "•".dimmed()
                );
            }
            ProviderCommands::Test { name } => {
                println!("{} Testing provider: {}...", "→".cyan(), name);
                let config = load_project_config(None);
                if let Some(provider_cfg) = config.providers.get(&name) {
                    println!("  {} Base URL: {}", "→".dimmed(), provider_cfg.base_url);
                    println!("  {} Model: {}", "→".dimmed(), provider_cfg.default_model);

                    // Resolve API key the same way CoreRuntime does
                    let api_key = if provider_cfg.api_key_ref.starts_with("env:") {
                        let env_var = &provider_cfg.api_key_ref[4..];
                        std::env::var(env_var).unwrap_or_default()
                    } else if !provider_cfg.api_key_ref.is_empty() {
                        provider_cfg.api_key_ref.clone()
                    } else {
                        String::new()
                    };

                    if api_key.is_empty() {
                        println!("  {} No API key configured", "⚠".yellow());
                        println!("  {} Provider will use mock behavior", "→".dimmed());
                        return Ok(());
                    }

                    // Build the provider
                    let result: Result<String, anyhow::Error> = match provider_cfg.name.as_str() {
                        "nan" | "openai" | "openai_compat" => {
                            let p = praxis_providers::OpenAIProvider::new(
                                api_key,
                                provider_cfg.default_model.clone(),
                                Some(provider_cfg.base_url.clone()),
                                None,
                                None,
                            )
                            .map_err(|e| anyhow::anyhow!("Failed to create provider: {}", e.0))?;
                            let resp = p
                                .chat(
                                    &[ChatMessage {
                                        role: ChatRole::User,
                                        content: "ping".to_string(),
                                        tool_calls: None,
                                        tool_call_id: None,
                                    }],
                                    &ChatConfig {
                                        max_tokens: 5,
                                        temperature: 0.0,
                                        ..Default::default()
                                    },
                                )
                                .await;
                            match resp {
                                Ok(r) => Ok(format!("Response: {}", r.content.trim())),
                                Err(e) => Err(anyhow::anyhow!("API error: {}", e)),
                            }
                        }
                        "anthropic" => {
                            let p = praxis_providers::AnthropicProvider::new(
                                api_key,
                                provider_cfg.default_model.clone(),
                                Some(provider_cfg.base_url.clone()),
                                None,
                                None,
                            )
                            .map_err(|e| anyhow::anyhow!("Failed to create provider: {}", e.0))?;
                            let resp = p
                                .chat(
                                    &[ChatMessage {
                                        role: ChatRole::User,
                                        content: "ping".to_string(),
                                        tool_calls: None,
                                        tool_call_id: None,
                                    }],
                                    &ChatConfig {
                                        max_tokens: 5,
                                        temperature: 0.0,
                                        ..Default::default()
                                    },
                                )
                                .await;
                            match resp {
                                Ok(r) => Ok(format!("Response: {}", r.content.trim())),
                                Err(e) => Err(anyhow::anyhow!("API error: {}", e)),
                            }
                        }
                        "gemini" => {
                            let p = praxis_providers::GeminiProvider::new(
                                api_key,
                                provider_cfg.default_model.clone(),
                                Some(provider_cfg.base_url.clone()),
                                None,
                                None,
                            )
                            .map_err(|e| anyhow::anyhow!("Failed to create provider: {}", e.0))?;
                            let resp = p
                                .chat(
                                    &[ChatMessage {
                                        role: ChatRole::User,
                                        content: "ping".to_string(),
                                        tool_calls: None,
                                        tool_call_id: None,
                                    }],
                                    &ChatConfig {
                                        max_tokens: 5,
                                        temperature: 0.0,
                                        ..Default::default()
                                    },
                                )
                                .await;
                            match resp {
                                Ok(r) => Ok(format!("Response: {}", r.content.trim())),
                                Err(e) => Err(anyhow::anyhow!("API error: {}", e)),
                            }
                        }
                        "ollama" => {
                            let p = praxis_providers::OllamaProvider::new(
                                provider_cfg.default_model.clone(),
                                Some(provider_cfg.base_url.clone()),
                            )
                            .map_err(|e| anyhow::anyhow!("Failed to create provider: {}", e))?;
                            let resp = p
                                .chat(
                                    &[ChatMessage {
                                        role: ChatRole::User,
                                        content: "ping".to_string(),
                                        tool_calls: None,
                                        tool_call_id: None,
                                    }],
                                    &ChatConfig {
                                        max_tokens: 5,
                                        temperature: 0.0,
                                        ..Default::default()
                                    },
                                )
                                .await;
                            match resp {
                                Ok(r) => Ok(format!("Response: {}", r.content.trim())),
                                Err(e) => Err(anyhow::anyhow!("API error: {}", e)),
                            }
                        }
                        _ => Err(anyhow::anyhow!(
                            "Unknown provider type: {}",
                            provider_cfg.name
                        )),
                    };

                    match result {
                        Ok(msg) => println!("  ✓ {}", msg.green()),
                        Err(msg) => println!("  ✗ {}", msg.to_string().red()),
                    }
                } else {
                    println!("  {} Provider '{}' not found in config", "✗".red(), name);
                }
            }
            ProviderCommands::Add {
                name,
                base_url,
                api_key,
                api_key_stdin,
                model,
                project,
            } => {
                println!("{} Adding provider: {}", "→".cyan(), name);

                // Get the API key securely
                let key = if api_key_stdin {
                    println!("  {} Reading API key from stdin...", "→".dimmed());
                    use std::io::Read;
                    let mut buf = String::new();
                    if std::io::stdin().read_to_string(&mut buf).is_err() {
                        println!("{} Failed to read API key from stdin", "✗".red());
                        std::process::exit(1);
                    }
                    buf.trim().to_string()
                } else if let Some(k) = api_key {
                    println!(
                        "  {} {} --api-key leaks to shell history. Use --api-key-stdin next time.",
                        "⚠".yellow(),
                        "Tip:".dimmed()
                    );
                    k
                } else {
                    println!(
                        "{} No API key provided. Use --api-key <key> or --api-key-stdin",
                        "✗".red()
                    );
                    std::process::exit(1);
                };

                if key.is_empty() {
                    println!("{} API key is empty", "✗".red());
                    std::process::exit(1);
                }

                let data_dir = get_data_dir();
                std::fs::create_dir_all(&data_dir)?;
                let vault_path = data_dir.join("credentials.vault.json");

                // Save to vault
                let vault = praxis_vault::VaultService::with_path(vault_path.clone(), None);
                if let Err(e) = vault.init() {
                    println!("{} Failed to initialize vault: {}", "✗".red(), e);
                    std::process::exit(1);
                }

                let key_ref = format!("vault:{}", name);
                if let Err(e) = vault.set(&name, &key) {
                    println!("{} Failed to store API key in vault: {}", "✗".red(), e);
                    std::process::exit(1);
                }

                let masked = if key.len() > 8 {
                    format!("{}***{}", &key[..4], &key[key.len() - 4..])
                } else {
                    "****".to_string()
                };

                println!("  Base URL: {}", base_url);
                println!("  API Key: {} (saved to vault)", masked);

                // Auto-wire the provider into the project's config.toml
                match add_provider_to_project_config(
                    project.as_deref(),
                    &name,
                    &base_url,
                    &key_ref,
                    model.as_deref(),
                ) {
                    Ok(proj_name) => {
                        println!(
                            "{} Provider '{}' saved to vault and wired to project '{}'",
                            "✓".green(),
                            name.cyan(),
                            proj_name.cyan()
                        );
                        if let Some(m) = &model {
                            println!("  Default model: {}", m);
                        }
                    }
                    Err(e) => {
                        println!(
                            "{} API key saved to vault, but could not wire to project config:",
                            "⚠".yellow()
                        );
                        println!("  {}", e);
                        println!(
                            "  The key is in the vault. Create a project with {} to use it.",
                            "praxis init".yellow()
                        );
                    }
                }
            }
        },

        Commands::Mcp(cmd) => match cmd {
            McpCommands::List => {
                let config = load_project_config(None);
                if config.mcp_servers.is_empty() {
                    println!("{} No MCP servers configured", "→".cyan());
                } else {
                    println!("{} MCP servers ({})", "→".cyan(), config.mcp_servers.len());
                    for server in &config.mcp_servers {
                        println!(
                            "  {} {} — {} {:?}",
                            "•".dimmed(),
                            server.name.cyan(),
                            server.command.dimmed(),
                            server.args
                        );
                    }
                }
            }
            McpCommands::Add {
                name,
                command,
                args,
            } => {
                println!("{} Adding MCP server: {}", "→".cyan(), name);
                println!("  Command: {} {:?}", command.dimmed(), args);
                println!(
                    "  {} MCP servers are managed via the dashboard Settings.",
                    "→".cyan()
                );
                println!(
                    "  {} Open with: {}",
                    "→".dimmed(),
                    "praxis dashboard".yellow()
                );
            }
            McpCommands::Remove { name } => {
                println!("{} Removing MCP server: {}", "→".cyan(), name);
                println!(
                    "  {} MCP servers are managed via the dashboard Settings.",
                    "→".cyan()
                );
                println!(
                    "  {} Open with: {}",
                    "→".dimmed(),
                    "praxis dashboard".yellow()
                );
            }
            McpCommands::Test { name } => {
                println!("{} Testing MCP server: {}", "→".cyan(), name);
                let config = load_project_config(None);
                if let Some(server) = config.mcp_servers.iter().find(|s| s.name == name) {
                    println!(
                        "  {} Command: {} {:?}",
                        "→".dimmed(),
                        server.command,
                        server.args
                    );
                    println!("  {} (would spawn and list tools)", "→".dimmed());
                } else {
                    println!("  {} MCP server '{}' not found in config", "✗".red(), name);
                }
            }
        },

        Commands::Context(cmd) => match cmd {
            ContextCommands::Inspect { session } => {
                println!("{} Context inspection: {}", "→".cyan(), session);
                println!();

                let data_dir = get_data_dir();
                let db_path = data_dir.join("state.db");

                // Try to load real session data from SQLite
                let session_data: Option<serde_json::Value> = if db_path.exists() {
                    let sid = uuid::Uuid::parse_str(&session).ok();
                    if let Some(sid) = sid {
                        let store = praxis_persistence::SqliteEventStore::new(&db_path)
                            .map_err(|e| anyhow::anyhow!(e))?;
                        store
                            .get_snapshot(sid)
                            .await
                            .map_err(|e| anyhow::anyhow!(e))?
                            .map(|s| s.state)
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(ref state) = session_data {
                    let phase = state.get("phase").and_then(|v| v.as_str()).unwrap_or("?");
                    let iteration = state.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0);
                    let pressure = state
                        .get("context_pressure")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let goal = state.get("goal").and_then(|v| v.as_str()).unwrap_or("?");

                    println!("  {} Session: {}", "→".cyan(), session);
                    println!("  {} Goal: {}", "→".cyan(), goal);
                    println!("  {} Phase: {}", "→".cyan(), phase);
                    println!("  {} Iteration: {}", "→".cyan(), iteration);
                    println!(
                        "  {} Context pressure: {:.1}%",
                        "→".cyan(),
                        pressure * 100.0
                    );
                    println!();
                } else {
                    println!(
                        "  {} No checkpoint found for session {}",
                        "⚠".yellow(),
                        session
                    );
                    println!("  Showing default context budget breakdown:");
                    println!();
                }

                // Show context budget breakdown
                let ctx_mgr = praxis_memory::ContextManager::new(
                    128_000,
                    praxis_memory::BudgetProfile::Balanced,
                );

                println!("  {} Model: gpt-5 (128k max)", "→".cyan());
                println!(
                    "  {} Hard limit: {} tokens (70%)",
                    "→".cyan(),
                    ctx_mgr.budget.hard_limit
                );
                println!("  {} Profile: balanced", "→".cyan());
                println!();

                println!("  {} Budget Allocation:", "→".cyan());
                let sections = [
                    ("System Prompt", praxis_memory::Section::SystemPrompt),
                    ("Goal Definition", praxis_memory::Section::GoalDefinition),
                    ("Active Task", praxis_memory::Section::ActiveTask),
                    ("Tool Results", praxis_memory::Section::ToolResults),
                    ("Recent History", praxis_memory::Section::RecentHistory),
                    ("Memory (RAG)", praxis_memory::Section::MemoryRag),
                    ("Project Context", praxis_memory::Section::ProjectContext),
                ];

                for (name, section) in sections {
                    let budget = ctx_mgr.budget.section_budget(section);
                    let bar_len =
                        (budget as f32 / ctx_mgr.budget.hard_limit as f32 * 30.0) as usize;
                    let bar: String = "█".repeat(bar_len) + &"░".repeat(30 - bar_len);
                    println!("    {:<20} {} {} tokens", name.dimmed(), bar, budget);
                }

                println!();
                println!("  {} Compression Pipeline:", "→".cyan());
                println!("    {} 1. Truncate tool results", "•".dimmed());
                println!("    {} 2. Compress history (summarize)", "•".dimmed());
                println!("    {} 3. Reduce RAG chunks (K=10→5→3→1)", "•".dimmed());
                println!("    {} 4. Prune project context", "•".dimmed());
                println!("    {} 5. Emergency consolidation", "•".dimmed());

                println!();
                println!("  {} Health: {:?}", "→".cyan(), ctx_mgr.health_status());
            }
            ContextCommands::History { session } => {
                println!(
                    "{} Compression history for session: {}",
                    "→".cyan(),
                    session
                );

                let data_dir = get_data_dir();
                let db_path = data_dir.join("state.db");
                let store = match praxis_persistence::SqliteEventStore::new(&db_path) {
                    Ok(s) => s,
                    Err(e) => {
                        println!("  {} Cannot open store: {}", "✗".red(), e);
                        return Ok(());
                    }
                };

                let conn = store
                    .conn()
                    .map_err(|e| anyhow::anyhow!("Pool error: {}", e))?;
                let mut stmt = conn.prepare(
                    "SELECT id, session_id, iteration, pressure_before, pressure_after, created_at \
                     FROM context_snapshots WHERE session_id = ?1 ORDER BY created_at DESC LIMIT 50"
                ).map_err(|e| anyhow::anyhow!("Query error: {}", e))?;

                let rows = stmt
                    .query_map(rusqlite_params![session], |row| {
                        Ok((
                            row.get::<_, String>("id")?,
                            row.get::<_, i64>("iteration")?,
                            row.get::<_, f64>("pressure_before").ok(),
                            row.get::<_, f64>("pressure_after").ok(),
                            row.get::<_, String>("created_at")?,
                        ))
                    })
                    .map_err(|e| anyhow::anyhow!("Query error: {}", e))?;

                let mut entries: Vec<_> = rows.filter_map(|r| r.ok()).collect();
                entries.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal)); // sort by iteration desc

                if entries.is_empty() {
                    println!(
                        "  {} No compression history found for this session",
                        "→".dimmed()
                    );
                } else {
                    println!(
                        "  {:<6} {:<12} {:<14} Time",
                        "Iteration", "Pressure In", "Pressure Out"
                    );
                    println!("  {}", "─".repeat(50));
                    for (_id, iteration, pressure_before, pressure_after, created_at) in
                        entries.iter()
                    {
                        let pb = pressure_before
                            .map_or("N/A".to_string(), |v| format!("{:.1}%", v * 100.0));
                        let pa = pressure_after
                            .map_or("N/A".to_string(), |v| format!("{:.1}%", v * 100.0));
                        println!("  {:<6} {:<12} {:<14} {}", iteration, pb, pa, created_at);
                    }
                    println!("\n  {} {} snapshots", "→".dimmed(), entries.len());
                }
            }
            ContextCommands::ForceCompress { session } => {
                println!(
                    "{} Forcing compression for session: {}",
                    "→".cyan(),
                    session
                );

                let mut ctx_mgr = praxis_memory::ContextManager::new(
                    128_000,
                    praxis_memory::BudgetProfile::Balanced,
                );
                let mut ctx_window = praxis_memory::ContextWindow::new();

                // Simulate over-budget context
                for i in 0..20 {
                    ctx_window.push(praxis_memory::Message {
                        role: "user".to_string(),
                        content: format!("Message {} with some content to test compression", i),
                    });
                }

                let result = ctx_mgr.force_consolidation(&mut ctx_window);
                println!("  {} Before: {} tokens", "→".cyan(), result.before_tokens);
                println!("  {} After:  {} tokens", "→".cyan(), result.after_tokens);
                println!("  {} Ratio:  {:.1}%", "→".cyan(), result.ratio * 100.0);
                println!("  {} Health: {:?}", "→".cyan(), ctx_mgr.health_status());
            }
        },

        Commands::Memory(cmd) => match cmd {
            MemoryCommands::Stats => {
                let data_dir = get_data_dir();
                let db_path = data_dir.join("state.db");
                let projects_path = data_dir.join("projects.json");

                println!("{} Memory & Persistence Stats", "→".cyan().bold());
                println!("{}", "─".repeat(50).dimmed());

                // Projects
                let project_count = std::fs::read_to_string(&projects_path)
                    .ok()
                    .and_then(|c| serde_json::from_str::<Vec<serde_json::Value>>(&c).ok())
                    .map(|p| p.len())
                    .unwrap_or(0);
                println!("  {} Projects: {}", "→".cyan(), project_count);

                // Sessions and checkpoints
                if db_path.exists() {
                    let store = praxis_persistence::SqliteEventStore::new(&db_path)
                        .map_err(|e| anyhow::anyhow!(e))?;
                    let session_ids = store
                        .list_aggregates("session")
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                    println!("  {} Sessions: {}", "→".cyan(), session_ids.len());

                    let mut snapshots = 0;
                    for sid in &session_ids {
                        if let Ok(Some(_)) = store.get_snapshot(*sid).await {
                            snapshots += 1;
                        }
                    }
                    println!("  {} Checkpoints: {}", "→".cyan(), snapshots);
                    println!(
                        "  {} Database: {}",
                        "→".cyan(),
                        db_path.display().to_string().dimmed()
                    );
                } else {
                    println!("  {} No database found. Run a session first.", "→".cyan());
                }

                // Data directory
                println!(
                    "  {} Data dir: {}",
                    "→".cyan(),
                    data_dir.display().to_string().dimmed()
                );

                // Injections
                let injections_dir = data_dir.join("injections");
                if injections_dir.is_dir() {
                    let count = std::fs::read_dir(&injections_dir)
                        .map(|entries| entries.filter_map(|e| e.ok()).count())
                        .unwrap_or(0);
                    println!("  {} Pending injections: {}", "→".cyan(), count);
                }
            }
            MemoryCommands::Sessions => {
                let data_dir = get_data_dir();
                let db_path = data_dir.join("state.db");
                if !db_path.exists() {
                    println!("{} No database found. Run a session first.", "→".cyan());
                } else {
                    let store = praxis_persistence::SqliteEventStore::new(&db_path)
                        .map_err(|e| anyhow::anyhow!(e))?;
                    let session_ids = store
                        .list_aggregates("session")
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                    if session_ids.is_empty() {
                        println!("{} No sessions found", "→".cyan());
                    } else {
                        println!("{} Sessions ({})", "→".cyan(), session_ids.len());
                        println!("{}", "─".repeat(80).dimmed());
                        for sid in &session_ids {
                            if let Ok(Some(snap)) = store.get_snapshot(*sid).await {
                                let goal = snap
                                    .state
                                    .get("goal")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("?");
                                let project = snap
                                    .state
                                    .get("project")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("default");
                                let phase = snap
                                    .state
                                    .get("phase")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("?");
                                let iteration = snap
                                    .state
                                    .get("iteration")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let pressure = snap
                                    .state
                                    .get("context_pressure")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                println!(
                                    "  {} {} — {} (iter {}) — {} — pressure {:.1}%",
                                    "•".dimmed(),
                                    sid.to_string().dimmed(),
                                    goal.cyan(),
                                    iteration,
                                    phase.dimmed(),
                                    pressure * 100.0,
                                );
                                println!("    project: {}", project);
                            }
                        }
                    }
                }
            }
            MemoryCommands::Events { id } => {
                let data_dir = get_data_dir();
                let db_path = data_dir.join("state.db");
                if !db_path.exists() {
                    println!("{} No database found.", "→".cyan());
                } else {
                    let sid = uuid::Uuid::parse_str(&id)
                        .map_err(|e| anyhow::anyhow!("Invalid session ID: {}", e))?;
                    let store = praxis_persistence::SqliteEventStore::new(&db_path)
                        .map_err(|e| anyhow::anyhow!(e))?;
                    let events = store
                        .read_events(sid, None)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                    if events.is_empty() {
                        println!("{} No events found for session {}", "→".cyan(), id);
                    } else {
                        println!(
                            "{} Events for session {} ({})",
                            "→".cyan(),
                            id,
                            events.len()
                        );
                        println!("{}", "─".repeat(80).dimmed());
                        for event in &events {
                            println!(
                                "  {} [{}] {} (v{})",
                                "•".dimmed(),
                                event.created_at.dimmed(),
                                event.event_type.cyan(),
                                event.version,
                            );
                            if let Ok(pretty) = serde_json::to_string_pretty(&event.payload) {
                                for line in pretty.lines().take(5) {
                                    println!("    {}", line.dimmed());
                                }
                            }
                        }
                    }
                }
            }
            MemoryCommands::Checkpoint { id } => {
                let data_dir = get_data_dir();
                let db_path = data_dir.join("state.db");
                if !db_path.exists() {
                    println!("{} No database found.", "→".cyan());
                } else {
                    let sid = uuid::Uuid::parse_str(&id)
                        .map_err(|e| anyhow::anyhow!("Invalid session ID: {}", e))?;
                    let store = praxis_persistence::SqliteEventStore::new(&db_path)
                        .map_err(|e| anyhow::anyhow!(e))?;
                    match store
                        .get_snapshot(sid)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?
                    {
                        Some(snap) => {
                            println!("{} Checkpoint for session {}", "→".cyan(), id);
                            println!("{}", "─".repeat(50).dimmed());
                            println!("  {} Type: {}", "→".cyan(), snap.aggregate_type);
                            println!("  {} Version: {}", "→".cyan(), snap.version);
                            println!("  {} Updated: {}", "→".cyan(), snap.updated_at);
                            if let Ok(pretty) = serde_json::to_string_pretty(&snap.state) {
                                println!();
                                println!("  {} State:", "→".cyan());
                                println!("{}", pretty.dimmed());
                            }
                        }
                        None => {
                            println!("{} No checkpoint found for session {}", "→".cyan(), id);
                        }
                    }
                }
            }
        },

        Commands::Inject {
            session: _,
            agent,
            message_type,
            message,
        } => {
            let data_dir = get_data_dir();
            let injections_dir = data_dir.join("injections");
            match std::fs::create_dir_all(&injections_dir) {
                Ok(()) => {
                    let injection = serde_json::json!({
                        "target_agent": agent,
                        "message_type": message_type,
                        "content": message,
                        "created_at": chrono::Utc::now().to_rfc3339(),
                    });
                    let filename = format!(
                        "{}_{}.json",
                        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
                        agent
                    );
                    let path = injections_dir.join(&filename);
                    match std::fs::write(&path, serde_json::to_string_pretty(&injection).unwrap()) {
                        Ok(()) => {
                            println!(
                                "{} Injection written for agent '{}'",
                                "✓".green(),
                                agent.cyan()
                            );
                            println!("  File: {}", path.display());
                            println!("  Type: {}", message_type.dimmed());
                            println!("  Message: {}", message.dimmed());
                            println!();
                            println!(
                                "  {} The running session will pick it up on the next iteration.",
                                "→".cyan()
                            );
                        }
                        Err(e) => {
                            println!("{} Failed to write injection: {}", "✗".red(), e);
                        }
                    }
                }
                Err(e) => {
                    println!("{} Failed to create injections directory: {}", "✗".red(), e);
                    println!("  Tried: {}", injections_dir.display());
                }
            }
        }

        Commands::Schedule {
            goal,
            project,
            every,
            until,
            max_runs,
            max_tokens,
            max_cost,
        } => {
            let interval = parse_duration(&every).unwrap_or_else(|| {
                eprintln!(
                    "{} Invalid duration '{}'. Use formats like: 30s, 5min, 1h, 2h30min",
                    "✗".red(),
                    every
                );
                std::process::exit(1);
            });

            println!("{} Scheduled goal: {}", "→".cyan(), goal.white().bold());
            println!("  {} Every: {}", "→".dimmed(), every);
            println!("  {} Until: {}", "→".dimmed(), until.cyan());
            println!("  {} Max runs: {}", "→".dimmed(), max_runs);
            println!();

            let vault = load_vault();

            let mut total_tokens: u64 = 0;
            let mut total_cost: f64 = 0.0;

            for run_num in 1..=max_runs {
                println!("{} Run {}/{}", "▶".cyan(), run_num, max_runs);

                // Check if the until-command already passes before running
                if check_until_command(&until) {
                    println!("{} Goal condition met: '{}' exits 0", "✓".green(), until);
                    println!(
                        "{} Schedule complete after {} run(s)",
                        "✓".green().bold(),
                        run_num - 1
                    );
                    break;
                }

                // Resolve project config path
                let config_path = resolve_config_path(project.as_deref());
                if let Some(ref path) = config_path {
                    println!(
                        "  {} Using project config: {}",
                        "→".dimmed(),
                        path.display()
                    );
                }

                let mut runtime = praxis_core::CoreRuntime::new()
                    .await?
                    .with_default_memory()
                    .with_state_file()
                    .with_skills();

                if let Some(ref name) = project {
                    runtime = runtime.with_project_name(name.clone());
                }

                // Apply the --until command as the completion criterion
                runtime = runtime.with_completion(
                    praxis_core::CompletionCriterion::from_until_command(until.clone()),
                );

                // Apply cumulative budget caps
                if max_tokens.is_some() || max_cost.is_some() {
                    let remaining_tokens = max_tokens.map(|mt| mt.saturating_sub(total_tokens));
                    let remaining_cost = max_cost.map(|mc| mc - total_cost);
                    runtime.loop_controller.limits.max_tokens = remaining_tokens;
                    runtime.loop_controller.limits.max_cost_usd = remaining_cost;
                }

                let result = runtime
                    .run_goal(&goal, config_path.as_deref(), vault.as_deref())
                    .await?;

                // Clean up temp config file
                if let Some(path) = config_path {
                    let _ = std::fs::remove_file(path);
                }

                total_tokens += runtime.loop_controller.tokens_used;
                total_cost += runtime.loop_controller.cost_usd;

                println!(
                    "  {} Run {} result: {}",
                    "→".dimmed(),
                    run_num,
                    if result.passed {
                        "✅ PASSED".green()
                    } else {
                        "❌ FAILED".red()
                    }
                );
                println!(
                    "  {} Tokens: {} | Cost: ${:.4}",
                    "→".dimmed(),
                    total_tokens,
                    total_cost
                );

                runtime.shutdown().await?;

                // Check if the until-command passes after this run
                if check_until_command(&until) {
                    println!();
                    println!("{} Goal condition met: '{}' exits 0", "✓".green(), until);
                    println!(
                        "{} Schedule complete after {} run(s)",
                        "✓".green().bold(),
                        run_num
                    );
                    break;
                }

                // Check cumulative budget
                if let Some(mt) = max_tokens
                    && total_tokens >= mt
                {
                    println!(
                        "{} Token budget exhausted: {}/{}",
                        "⚠".yellow(),
                        total_tokens,
                        mt
                    );
                    break;
                }
                if let Some(mc) = max_cost
                    && total_cost >= mc
                {
                    println!(
                        "{} Cost budget exhausted: ${:.4}/${:.4}",
                        "⚠".yellow(),
                        total_cost,
                        mc
                    );
                    break;
                }

                if run_num < max_runs {
                    println!("  {} Waiting {} before next run...", "⏳".dimmed(), every);
                    tokio::time::sleep(interval).await;
                }
            }

            println!();
            println!(
                "{} Schedule ended. Total tokens: {} | Cost: ${:.4}",
                "→".cyan(),
                total_tokens,
                total_cost
            );
        }

        Commands::Plan {
            goal,
            project,
            output,
        } => {
            println!("{} Planning goal: {}", "→".cyan(), goal.white().bold());

            let vault = load_vault();
            let config_path = resolve_config_path(project.as_deref());
            if let Some(ref path) = config_path {
                println!(
                    "  {} Using project config: {}",
                    "→".dimmed(),
                    path.display()
                );
            }

            // Determine output path
            let output_path = output.clone().unwrap_or_else(|| {
                let data_dir = get_data_dir();
                let proj_name = project.as_deref().unwrap_or("default");
                let plans_dir = data_dir.join("projects").join(proj_name).join("plans");
                let _ = std::fs::create_dir_all(&plans_dir);
                let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
                plans_dir.join(format!("plan-{}.md", timestamp))
            });

            // Create a runtime and run only the Planning phase
            let mut runtime = praxis_core::CoreRuntime::new()
                .await?
                .with_default_memory()
                .with_state_file()
                .with_skills();

            if let Some(ref name) = project {
                runtime = runtime.with_project_name(name.clone());
            }

            // For plan mode, we run the goal but with a manual completion criterion
            // so it stops after the first Planning + Designing iteration.
            runtime = runtime
                .with_completion(praxis_core::CompletionCriterion::from_string("manual").unwrap());

            println!("  {} Running Planning + Designing phases...", "→".dimmed());

            let result = runtime
                .run_goal(&goal, config_path.as_deref(), vault.as_deref())
                .await?;

            // Clean up temp config
            if let Some(path) = config_path {
                let _ = std::fs::remove_file(path);
            }

            // Build the plan document from the agent results
            let mut plan_content = format!(
                "# Plan: {}\n\n\
                 ## Goal\n{}\n\n\
                 ## Status\n{}\n\n\
                 ## Agent Outputs\n\n",
                goal,
                goal,
                if result.passed {
                    "Planning complete"
                } else {
                    "Planning incomplete (review outputs)"
                },
            );

            for agent_result in &result.agent_results {
                plan_content.push_str(&format!(
                    "### {} ({})\n\n{}\n\n",
                    agent_result.agent_id, agent_result.role, agent_result.content,
                ));
            }

            plan_content.push_str("## Next Steps\n\n");
            plan_content.push_str("Review the plan above. When ready, execute with:\n\n");
            plan_content.push_str(&format!(
                "```bash\npraxis run --plan \"{}\"\n```\n",
                output_path.display()
            ));

            std::fs::write(&output_path, &plan_content)?;

            println!();
            println!("{} Plan saved to: {}", "✓".green(), output_path.display());
            println!("  {} Review the plan, then execute with:", "→".cyan());
            println!(
                "  {} praxis run --plan \"{}\"",
                "→".cyan(),
                output_path.display()
            );

            runtime.shutdown().await?;
        }

        Commands::Desktop => {
            println!("{} Building and launching desktop app...", "→".cyan());
            println!("{} This may take a moment on first run.", "→".dimmed());
            println!();

            // Determine the desktop directory path (relative to the binary)
            let desktop_dir = std::env::current_dir()
                .map(|d| d.join("desktop"))
                .unwrap_or_else(|_| PathBuf::from("desktop"));

            // Try `cargo tauri dev` first (HMR + Vite dev server).
            // `cargo tauri dev` does NOT support --manifest-path, so we must set current_dir.
            let tauri_result = std::process::Command::new("cargo")
                .args(["tauri", "dev"])
                .current_dir(&desktop_dir)
                .spawn();

            match tauri_result {
                Ok(mut child) => {
                    let status = child
                        .wait()
                        .map_err(|e| anyhow::anyhow!("Desktop process error: {}", e))?;
                    if !status.success() {
                        eprintln!(
                            "{} Desktop exited with code: {}",
                            "⚠".yellow(),
                            status.code().unwrap_or(-1)
                        );
                    }
                }
                Err(_) => {
                    // tauri-cli not installed, fall back to `cargo run -p desktop`
                    println!(
                        "{} (tauri-cli not found, using cargo run -p desktop)",
                        "ℹ".dimmed()
                    );
                    let mut child = std::process::Command::new("cargo")
                        .args(["run", "-p", "desktop"])
                        .spawn()
                        .map_err(|e| anyhow::anyhow!("Failed to launch desktop: {}", e))?;

                    let status = child
                        .wait()
                        .map_err(|e| anyhow::anyhow!("Desktop process error: {}", e))?;
                    if !status.success() {
                        eprintln!(
                            "{} Desktop exited with code: {}",
                            "⚠".yellow(),
                            status.code().unwrap_or(-1)
                        );
                    }
                }
            }
        }
        Commands::Dashboard => {
            println!("{} Starting dashboard...", "→".cyan());

            let data_dir = get_data_dir();
            std::fs::create_dir_all(&data_dir)?;

            // Start the API server in the background
            let vault_password = std::env::var("VAULT_PASSWORD").ok();
            let server = praxis_core::api::ApiServer::new(praxis_core::api::ApiServerConfig {
                port: 8080,
                cors_origins: vec!["*".to_string()],
                vault_password,
                data_dir: data_dir.clone(),
                enable_pairing: false,
            });

            println!("{} API server: http://localhost:8080", "✓".green());
            println!("{} WebSocket:  ws://localhost:8080/ws", "✓".green());

            let server_handle = tokio::spawn(async move {
                let _ = server.start().await;
            });

            // Give the server a moment to bind
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // Try to open the dashboard in the browser
            let dashboard_url = "http://localhost:8080";
            println!(
                "{} Opening dashboard in browser: {}",
                "→".cyan(),
                dashboard_url
            );

            #[cfg(target_os = "windows")]
            let open_result = std::process::Command::new("cmd")
                .args(["/C", "start", "", dashboard_url])
                .spawn();

            #[cfg(target_os = "macos")]
            let open_result = std::process::Command::new("open")
                .arg(dashboard_url)
                .spawn();

            #[cfg(all(unix, not(target_os = "macos")))]
            let open_result = std::process::Command::new("xdg-open")
                .arg(dashboard_url)
                .spawn();

            match open_result {
                Ok(_) => println!("{} Browser opened", "✓".green()),
                Err(_) => {
                    println!(
                        "{} Could not auto-open browser. Visit: {}",
                        "⚠".yellow(),
                        dashboard_url
                    );
                }
            }

            println!();
            println!("{} Press Ctrl+C to stop the server", "→".dimmed());
            println!("{}", "─".repeat(50).dimmed());

            // Wait for Ctrl+C
            tokio::signal::ctrl_c().await?;
            println!();
            println!("{} Shutting down...", "→".dimmed());
            server_handle.abort();
        }
        Commands::Server { pair } => {
            // Read vault password from env if set
            let vault_password = std::env::var("VAULT_PASSWORD").ok();
            let data_dir = get_data_dir();

            println!("{} Starting API server...", "→".cyan());
            println!("{} Data directory: {}", "→".cyan(), data_dir.display());

            // Ensure data directory exists
            std::fs::create_dir_all(&data_dir)?;

            let server = praxis_core::api::ApiServer::new(praxis_core::api::ApiServerConfig {
                port: 8080,
                cors_origins: vec!["*".to_string()],
                vault_password,
                data_dir: data_dir.clone(),
                enable_pairing: pair,
            });

            println!("{} REST: http://localhost:8080", "✓".green());
            println!("{} WebSocket: ws://localhost:8080/ws", "✓".green());
            println!(
                "{} Vault: {}/credentials.vault.json",
                "✓".green(),
                data_dir.display()
            );
            println!(
                "{} Projects: {}/projects.json",
                "✓".green(),
                data_dir.display()
            );
            println!("{0}\n{} Press Ctrl+C to stop\n{0}", "─".repeat(50).dimmed());

            tokio::spawn(async move {
                let _ = server.start().await;
            });

            // Wait forever (Ctrl+C will shutdown)
            tokio::signal::ctrl_c().await?;
            println!();
            println!("{} Server shutting down...", "→".dimmed());
        }
        Commands::Monitor => {
            // List sessions and let the user pick one to watch
            let data_dir = get_data_dir();
            let db_path = data_dir.join("state.db");

            if !db_path.exists() {
                println!("{} No sessions found. Run a goal first:", "→".cyan());
                println!(
                    "  {} --project <name> --goal \"your goal\"",
                    "praxis run".yellow()
                );
                std::process::exit(0);
            }

            let store = praxis_persistence::SqliteEventStore::new(&db_path)
                .map_err(|e| anyhow::anyhow!(e))?;
            let session_ids = store
                .list_aggregates("session")
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            if session_ids.is_empty() {
                println!("{} No sessions found. Run a goal first:", "→".cyan());
                println!(
                    "  {} --project <name> --goal \"your goal\"",
                    "praxis run".yellow()
                );
                std::process::exit(0);
            }

            println!("{} Sessions:", "→".cyan());
            for (i, sid) in session_ids.iter().enumerate() {
                let snapshot = store
                    .get_snapshot(*sid)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                let goal = snapshot
                    .as_ref()
                    .and_then(|s| s.state.get("goal").and_then(|v| v.as_str()))
                    .unwrap_or("?");
                let phase = snapshot
                    .as_ref()
                    .and_then(|s| s.state.get("phase").and_then(|v| v.as_str()))
                    .unwrap_or("?");
                println!(
                    "  [{}] {} — {} ({})",
                    i + 1,
                    sid,
                    goal.cyan(),
                    phase.dimmed()
                );
            }
            println!();

            // Pick the most recent session (last in list)
            let session_id = session_ids.last().unwrap();
            println!(
                "{} Watching most recent session: {}",
                "→".cyan(),
                session_id
            );
            println!();

            // Check if the API server is running, if not start it
            let api_url = "http://localhost:8080";
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .unwrap_or_default();
            let server_reachable = client
                .get(format!("{}/api/health", api_url))
                .send()
                .await
                .is_ok();

            if !server_reachable {
                println!("{} API server not running. Starting it...", "→".cyan());
                let vault_password = std::env::var("VAULT_PASSWORD").ok();
                let server = praxis_core::api::ApiServer::new(praxis_core::api::ApiServerConfig {
                    port: 8080,
                    cors_origins: vec!["*".to_string()],
                    vault_password,
                    data_dir: data_dir.clone(),
                    enable_pairing: false,
                });
                tokio::spawn(async move {
                    let _ = server.start().await;
                });
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                println!("{} API server started", "✓".green());
            }

            commands::watch::run(&session_id.to_string(), api_url, 2).await;
        }

        Commands::Watch {
            session_id,
            api,
            interval,
        } => {
            commands::watch::run(&session_id, &api, interval).await;
        }

        Commands::Update { channel } => {
            let current_version = env!("CARGO_PKG_VERSION");
            println!(
                "{} Checking for updates (channel: {})...",
                "→".cyan(),
                channel
            );
            println!("  Current version: v{}", current_version);

            // Check GitHub releases for the latest version
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default();

            let resp = client
                .get("https://api.github.com/repos/praxis-ai/praxis/releases/latest")
                .header("User-Agent", format!("praxis/{}", current_version))
                .send()
                .await
                .ok();

            let latest = match resp {
                Some(r) => r.json::<serde_json::Value>().await.ok(),
                None => None,
            };

            let latest = latest.and_then(|v| {
                v.get("tag_name")
                    .and_then(|t| t.as_str())
                    .map(|s| s.trim_start_matches('v').to_string())
            });

            match latest {
                Some(latest_version) => {
                    println!("  Latest version:  v{}", latest_version);
                    if latest_version == current_version {
                        println!("{} Already up to date", "✓".green());
                    } else {
                        println!(
                            "{} Update available: v{} → v{}",
                            "↑".yellow(),
                            current_version,
                            latest_version
                        );
                        println!();
                        println!("  To update:");
                        println!("    {} install praxis-ai/praxis", "cargo binstall".yellow());
                        println!(
                            "    or: {} clone https://github.com/praxis-ai/praxis && cd praxis && cargo install --path crates/cli",
                            "git".yellow()
                        );
                    }
                }
                None => {
                    println!(
                        "{} Could not check for updates (network error or rate limited)",
                        "⚠".yellow()
                    );
                    println!("  Check manually: https://github.com/praxis-ai/praxis/releases");
                }
            }
        }

        Commands::Deploy(cmd) => match cmd {
            DeployCommands::Setup { host } => {
                println!("{} Setting up VPS deployment...", "→".cyan());
                commands::deploy::setup(&host)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                println!("{} Deployment configured", "✓".green());
            }
            DeployCommands::Push => {
                println!("{} Pushing to VPS...", "→".cyan());
                commands::deploy::push()
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                println!("{} Push complete", "✓".green());
            }
            DeployCommands::Status => {
                println!("{} Checking VPS status...", "→".cyan());
                commands::deploy::status()
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
            }
            DeployCommands::Logs { tail } => {
                commands::deploy::logs(tail)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
            }
        },

        // ─── Doctor ────────────────────────────────────────
        Commands::Doctor => {
            println!("{} Diagnosing praxis setup...", "→".cyan());
            println!("{}", "═".repeat(50).dimmed());
            println!();

            let data_dir = get_data_dir();
            let mut all_ok = true;

            // 1. Data directory
            print!("  {} Data directory... ", "1.".cyan());
            if data_dir.exists() {
                println!("{} {}", "✓".green(), data_dir.display());
            } else {
                println!("{} not found at {}", "✗".red(), data_dir.display());
                println!("    Run {} to create it", "praxis init <name>".yellow());
                all_ok = false;
            }

            // 2. Projects
            print!("  {} Projects...       ", "2.".cyan());
            let projects_path = data_dir.join("projects.json");
            match std::fs::read_to_string(&projects_path) {
                Ok(content) => {
                    let projects: Vec<serde_json::Value> =
                        serde_json::from_str(&content).unwrap_or_default();
                    if projects.is_empty() {
                        println!("{} no projects found", "⚠".yellow());
                        println!("    Run {} to create one", "praxis init <name>".yellow());
                    } else {
                        println!("{} {} project(s)", "✓".green(), projects.len());
                    }
                }
                Err(_) => {
                    println!("{} no projects.json", "✗".red());
                    println!("    Run {} to create one", "praxis init <name>".yellow());
                    all_ok = false;
                }
            }

            // 3. Vault
            print!("  {} Vault...          ", "3.".cyan());
            let vault_path = data_dir.join("credentials.vault.json");
            if vault_path.exists() {
                let vault = praxis_vault::VaultService::with_path(vault_path.clone(), None);
                if vault.init().is_ok() {
                    match vault.list_keys() {
                        Ok(keys) => {
                            if keys.is_empty() {
                                println!("{} empty (no API keys stored)", "⚠".yellow());
                                println!(
                                    "    Run: {} openai https://api.openai.com/v1 --api-key-stdin",
                                    "praxis provider add".yellow()
                                );
                            } else {
                                println!("{} {} credential(s) stored", "✓".green(), keys.len());
                            }
                        }
                        Err(_) => {
                            println!("{} could not read vault", "⚠".yellow());
                        }
                    }
                } else {
                    println!("{} could not initialize vault", "✗".red());
                    all_ok = false;
                }
            } else {
                println!("{} no vault file", "⚠".yellow());
                println!(
                    "    Run: {} <name> <base_url> --api-key-stdin",
                    "praxis provider add".yellow()
                );
            }

            // 4. Database
            print!("  {} Database...       ", "4.".cyan());
            let db_path = data_dir.join("state.db");
            if db_path.exists() {
                match praxis_persistence::SqliteEventStore::new(&db_path) {
                    Ok(store) => match store.list_aggregates("session").await {
                        Ok(sessions) => {
                            println!("{} {} session(s) recorded", "✓".green(), sessions.len());
                        }
                        Err(e) => {
                            println!("{} could not read sessions: {}", "✗".red(), e);
                            all_ok = false;
                        }
                    },
                    Err(e) => {
                        println!("{} could not open: {}", "✗".red(), e);
                        all_ok = false;
                    }
                }
            } else {
                println!("{} no database (run a session first)", "⚠".yellow());
            }

            // 5. API server
            print!("  {} API server...     ", "5.".cyan());
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .unwrap_or_default();
            match client.get("http://localhost:8080/api/health").send().await {
                Ok(resp) if resp.status().is_success() => {
                    println!("{} running on http://localhost:8080", "✓".green());
                }
                _ => {
                    println!("{} not running", "⚠".yellow());
                    println!("    Start with: {}", "praxis server".cyan());
                }
            }

            // 6. Agents
            print!("  {} Agents...         ", "6.".cyan());
            let global_dir = praxis_core::agents::AgentRegistry::global_dir();
            let registry = praxis_core::agents::AgentRegistry::load(Some(&global_dir), None);
            println!(
                "{} {} agent(s) ({} builtin, {} global)",
                "✓".green(),
                registry.list().len(),
                registry
                    .list_by_scope(praxis_core::agents::AgentScope::Builtin)
                    .len(),
                registry
                    .list_by_scope(praxis_core::agents::AgentScope::Global)
                    .len(),
            );

            println!();
            println!("{}", "═".repeat(50).dimmed());
            if all_ok {
                println!("{} All checks passed!", "✓".green().bold());
            } else {
                println!(
                    "{} Some checks need attention (see above)",
                    "⚠".yellow().bold()
                );
            }
        }

        // ─── Quickstart ────────────────────────────────────
        Commands::Quickstart => {
            println!("{} {} — Quickstart Guide", "→".cyan(), "praxis".bold());
            println!("{}", "═".repeat(50).dimmed());
            println!();

            let data_dir = get_data_dir();
            let projects_path = data_dir.join("projects.json");
            let has_projects = std::fs::read_to_string(&projects_path)
                .ok()
                .and_then(|c| serde_json::from_str::<Vec<serde_json::Value>>(&c).ok())
                .map(|p| !p.is_empty())
                .unwrap_or(false);

            if !has_projects {
                println!("{} Step 1: Create your first project", "→".cyan().bold());
                println!();
                println!("  {} my-app", "praxis init".yellow());
                println!();
                println!(
                    "  This creates a project at: {}/projects/my-app/",
                    data_dir.display()
                );
                println!();
                println!("{} Step 2: Add an LLM provider", "→".cyan().bold());
                println!();
                println!(
                    "  {} openai https://api.openai.com/v1 --api-key-stdin",
                    "praxis provider add".yellow()
                );
                println!("  (then paste your API key and press Enter)");
                println!();
                println!("  Or use an environment variable in config.toml:");
                println!("    [providers.openai]");
                println!("    base_url = \"https://api.openai.com/v1\"");
                println!("    api_key = \"env:OPENAI_API_KEY\"");
                println!();
                println!("{} Step 3: Run your first goal", "→".cyan().bold());
                println!();
                println!(
                    "  {} --project my-app --goal \"Build a hello world CLI\"",
                    "praxis run".yellow()
                );
                println!();
                println!("{} Step 4: Watch it run in real-time", "→".cyan().bold());
                println!();
                println!("  {}", "praxis monitor".yellow());
                println!();
                println!("{} Step 5: Open the dashboard", "→".cyan().bold());
                println!();
                println!("  {}", "praxis dashboard".yellow());
                println!();
            } else {
                // Has projects — show status
                println!("{} You have projects set up!", "✓".green().bold());
                println!();

                // List projects
                let projects: Vec<serde_json::Value> = std::fs::read_to_string(&projects_path)
                    .ok()
                    .and_then(|c| serde_json::from_str(&c).ok())
                    .unwrap_or_default();
                println!("  {} Projects:", "→".cyan());
                for p in &projects {
                    let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    println!("    {} {}", "•".dimmed(), name.cyan());
                }
                println!();

                // Check vault
                let vault_path = data_dir.join("credentials.vault.json");
                let has_vault = vault_path.exists();
                if !has_vault {
                    println!("  {} No API keys in vault. Add one:", "⚠".yellow());
                    println!(
                        "    {} openai https://api.openai.com/v1 --api-key-stdin",
                        "praxis provider add".yellow()
                    );
                    println!();
                }

                println!("  {} Next steps:", "→".cyan());
                println!(
                    "    {} --project <name> --goal \"your goal\"",
                    "praxis run".yellow()
                );
                println!("    {} (watch in real-time)", "praxis monitor".yellow());
                println!("    {} (web dashboard)", "praxis dashboard".yellow());
            }

            println!("{}", "═".repeat(50).dimmed());
            println!("  Docs: https://github.com/praxis-ai/praxis");
            println!("  Help: {}", "praxis help".yellow());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_seconds() {
        assert_eq!(
            parse_duration("30s"),
            Some(std::time::Duration::from_secs(30))
        );
        assert_eq!(
            parse_duration("90s"),
            Some(std::time::Duration::from_secs(90))
        );
        assert_eq!(
            parse_duration("5sec"),
            Some(std::time::Duration::from_secs(5))
        );
    }

    #[test]
    fn test_parse_duration_minutes() {
        assert_eq!(
            parse_duration("5min"),
            Some(std::time::Duration::from_secs(300))
        );
        assert_eq!(
            parse_duration("1minute"),
            Some(std::time::Duration::from_secs(60))
        );
    }

    #[test]
    fn test_parse_duration_hours() {
        assert_eq!(
            parse_duration("1h"),
            Some(std::time::Duration::from_secs(3600))
        );
        assert_eq!(
            parse_duration("2h"),
            Some(std::time::Duration::from_secs(7200))
        );
    }

    #[test]
    fn test_parse_duration_compound() {
        assert_eq!(
            parse_duration("2h30min"),
            Some(std::time::Duration::from_secs(9000))
        );
        assert_eq!(
            parse_duration("1h30min30s"),
            Some(std::time::Duration::from_secs(5430))
        );
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert_eq!(parse_duration("invalid"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("0s"), None);
    }

    #[test]
    fn test_check_until_command_success() {
        // "exit 0" on Windows, "true" on Unix
        let cmd = if cfg!(windows) { "exit 0" } else { "true" };
        assert!(check_until_command(cmd));
    }

    #[test]
    fn test_check_until_command_failure() {
        // "exit 1" on Windows, "false" on Unix
        let cmd = if cfg!(windows) { "exit 1" } else { "false" };
        assert!(!check_until_command(cmd));
    }
}
