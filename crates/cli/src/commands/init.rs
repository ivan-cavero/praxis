//! Init command — create a new project in AppData.

use std::path::PathBuf;

/// Get the central data directory.
fn get_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PRAXIS_DATA_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("praxis");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config").join("praxis");
        }
    }
    PathBuf::from(".praxis-data")
}

/// Default forge.toml template for new projects.
const FORGE_TOML: &str = r#"[project]
name = "{name}"
version = "0.1.0"

[providers.nan]
base_url = "https://api.nan.builders/v1"
api_key = "env:NAN_API_KEY"
default_model = "qwen3.6"

[roles.architect]
description = "System design and architecture"
model = "claude-sonnet-4-20250514"
temperature = 0.2
system_prompt = "You are a senior software architect specialized in Rust systems."
tools = ["filesystem", "web_search"]

[roles.coder]
description = "Code generation and implementation"
model = "gpt-4o"
temperature = 0.3
system_prompt = "You are an expert Rust engineer. Write production-quality code."
tools = ["filesystem", "execute_command"]

[roles.reviewer]
description = "Code review and quality assurance"
model = "gpt-4o"
temperature = 0.2
system_prompt = "You are a senior code reviewer. Analyze code critically."
tools = ["filesystem"]

[roles.security]
description = "Security audit and vulnerability scanning"
model = "claude-sonnet-4-20250514"
temperature = 0.1
system_prompt = "You are a security auditor. Check for vulnerabilities."
tools = ["filesystem", "grep"]

[roles.tester]
description = "Test generation and execution"
model = "gpt-4o"
temperature = 0.2
system_prompt = "You are a QA engineer. Generate comprehensive tests."
tools = ["filesystem", "execute_command"]

[roles.researcher]
description = "Technical research and documentation"
model = "gpt-4o"
temperature = 0.3
system_prompt = "You are a technical researcher. Find best practices."
tools = ["web_search", "read_url"]

[[goals]]
name = "full-feature"
description = "Complete feature development"
agents = ["architect", "coder", "reviewer", "security", "tester"]
gates = ["review.pass", "security.no_critical", "test.pass"]
max_iterations = 10

[[goals]]
name = "quick-fix"
description = "Fast bug fix"
agents = ["coder", "reviewer"]
max_iterations = 3

[limits]
max_iterations_per_goal = 50
max_iterations_per_phase = 5
session_ttl_seconds = 3600
phase_timeout_seconds = 300
"#;

/// Create a new project in AppData.
pub fn init_project(name: &str) -> anyhow::Result<()> {
    let data_dir = get_data_dir();
    std::fs::create_dir_all(&data_dir)?;

    let projects_path = data_dir.join("projects.json");

    // Load existing projects
    let mut projects: Vec<serde_json::Value> = match std::fs::read_to_string(&projects_path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    // Check if name already taken
    if projects.iter().any(|p| p.get("name").and_then(|v| v.as_str()) == Some(name)) {
        anyhow::bail!("Project '{}' already exists", name);
    }

    // Create project entry
    let forge_toml = FORGE_TOML.replace("{name}", name);
    let now = chrono::Utc::now().to_rfc3339();
    let project = serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "name": name,
        "description": "",
        "created_at": now,
        "last_active": now,
        "forge_toml": forge_toml,
    });

    projects.push(project);
    std::fs::write(&projects_path, serde_json::to_string_pretty(&projects)?)?;

    println!("  Created project '{}' in AppData", name);
    println!("  Data directory: {}", data_dir.display());

    Ok(())
}

