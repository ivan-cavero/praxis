//! Init command — create a new project in the praxis data directory.

/// Default forge.toml template for new projects.
/// Must stay in sync with `DEFAULT_FORGE_TOML` in `crates/core/src/api/routes.rs`.
const FORGE_TOML: &str = r#"[project]
name = "{name}"
version = "0.1.0"

[providers.nan]
base_url = "https://api.nan.builders/v1"
api_key = "env:NAN_API_KEY"
default_model = "qwen3.6"

[roles.architect]
model = "claude-sonnet-4-20250514"
temperature = 0.2
tools = ["filesystem", "web_search"]
system_prompt = "You are a senior software architect."

[roles.coder]
model = "gpt-4o"
temperature = 0.3
tools = ["filesystem", "execute_command"]
system_prompt = "You implement clean, maintainable code."

[roles.reviewer]
model = "gpt-4o"
temperature = 0.2
tools = ["filesystem"]
system_prompt = "Review for correctness and quality."

[roles.security]
model = "claude-sonnet-4-20250514"
temperature = 0.1
tools = ["filesystem", "grep"]
system_prompt = "Audit for security vulnerabilities."

[roles.tester]
model = "gpt-4o"
temperature = 0.2
tools = ["filesystem", "execute_command"]
system_prompt = "Generate and execute tests."

[roles.researcher]
model = "gpt-4o"
temperature = 0.3
tools = ["web_search", "read_url"]
system_prompt = "Research technical topics."

[[goals]]
name = "full-feature"
agents = ["architect", "coder", "reviewer", "security", "tester"]
gates = ["review.pass", "security.no_critical", "test.pass"]
max_iterations = 10

[[goals]]
name = "quick-fix"
agents = ["coder", "reviewer"]
max_iterations = 3

[limits]
max_iterations_per_goal = 50
max_iterations_per_phase = 5
session_ttl_seconds = 3600
phase_timeout_seconds = 300
# max_tokens = 1000000      # stop after 1M total tokens (comment to disable)
# max_cost_usd = 5.0        # stop after $5 estimated cost (comment to disable)
"#;

/// Create a new project.
///
/// Creates a per-project directory structure:
/// ```text
/// ~/.config/praxis/
/// ├── projects.json          ← lightweight index
/// └── projects/
///     └── <name>/
///         ├── config.toml    ← project config (was forge.toml)
///         ├── state.db       ← SQLite event store (created on first run)
///         ├── STATE.md       ← live state file (created during runs)
///         ├── skills/        ← SKILL.md files
///         ├── plans/         ← saved plans
///         └── injections/    ← mid-loop injection files
/// ```
pub fn init_project(name: &str) -> anyhow::Result<()> {
    let data_dir = crate::get_data_dir();
    std::fs::create_dir_all(&data_dir)?;

    let projects_path = data_dir.join("projects.json");

    // Load existing projects
    let mut projects: Vec<serde_json::Value> = match std::fs::read_to_string(&projects_path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    // Check if name already taken
    if projects
        .iter()
        .any(|p| p.get("name").and_then(|v| v.as_str()) == Some(name))
    {
        anyhow::bail!("Project '{}' already exists", name);
    }

    // Create per-project directory structure
    let project_dir = data_dir.join("projects").join(name);
    std::fs::create_dir_all(&project_dir)?;
    std::fs::create_dir_all(project_dir.join("skills"))?;
    std::fs::create_dir_all(project_dir.join("plans"))?;
    std::fs::create_dir_all(project_dir.join("injections"))?;

    // Write config.toml into the project directory
    let config_toml = FORGE_TOML.replace("{name}", name);
    let config_path = project_dir.join("config.toml");
    std::fs::write(&config_path, &config_toml)?;

    // Create project entry
    let now = chrono::Utc::now().to_rfc3339();
    let project = serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "name": name,
        "description": "",
        "created_at": now,
        "last_active": now,
        "forge_toml": config_toml,
        "path": project_dir.display().to_string(),
    });

    projects.push(project);
    std::fs::write(&projects_path, serde_json::to_string_pretty(&projects)?)?;

    println!("  Created project '{}'", name);
    println!("  Data directory: {}", data_dir.display());
    println!("  Project directory: {}", project_dir.display());
    println!("  Config: {}", config_path.display());

    Ok(())
}
