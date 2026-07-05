//! Agent CLI commands — list, show, add, edit, remove.
//!
//! Agents are stored as Markdown+YAML `.md` files in 3 scopes:
//!   - builtin:  compiled into the binary (read-only)
//!   - global:   ~/.config/praxis/agents/*.md
//!   - project:  <project_dir>/agents/*.md

use colored::Colorize;
use praxis_core::agents::{AgentRegistry, AgentScope};

use crate::AgentCommands;

/// Resolve the project agents directory from the current working directory.
/// Uses `./agents/` if it exists, otherwise falls back to the data dir.
fn project_agents_dir() -> std::path::PathBuf {
    let local = std::path::PathBuf::from("agents");
    if local.exists() {
        return local;
    }
    // Fall back to data dir
    let data_dir = crate::get_data_dir();
    data_dir.join("agents")
}

/// Build a registry from all scopes.
fn build_registry() -> AgentRegistry {
    let global_dir = AgentRegistry::global_dir();
    let project_dir = project_agents_dir();
    AgentRegistry::load(Some(&global_dir), Some(&project_dir))
}

/// Serialize an AgentDefinition to Markdown+YAML for disk.
fn serialize_agent_md(
    name: &str,
    model: &str,
    temperature: f32,
    max_tokens: u32,
    max_turns: u32,
    max_depth: u8,
    tools: &[String],
    can_spawn: &[String],
    system_prompt: &str,
) -> String {
    let mut md = String::new();
    md.push_str("---\n");
    md.push_str(&format!("name: {name}\n"));
    md.push_str(&format!("model: {model}\n"));
    md.push_str(&format!("temperature: {temperature}\n"));
    md.push_str(&format!("max_tokens: {max_tokens}\n"));
    if tools.is_empty() {
        md.push_str("tools: []\n");
    } else {
        md.push_str("tools:\n");
        for t in tools {
            md.push_str(&format!("  - {t}\n"));
        }
    }
    md.push_str(&format!("max_turns: {max_turns}\n"));
    md.push_str(&format!("max_depth: {max_depth}\n"));
    if can_spawn.is_empty() {
        md.push_str("can_spawn: []\n");
    } else {
        md.push_str("can_spawn:\n");
        for s in can_spawn {
            md.push_str(&format!("  - {s}\n"));
        }
    }
    md.push_str("---\n\n");
    md.push_str(system_prompt);
    md
}

/// Parse a comma-separated string into a Vec<String>.
fn parse_csv(s: Option<String>) -> Vec<String> {
    s.map(|v| v.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
        .unwrap_or_default()
}

pub fn handle(cmd: AgentCommands) {
    match cmd {
        AgentCommands::List { scope } => list(&scope),
        AgentCommands::Show { name } => show(&name),
        AgentCommands::Add {
            name, model, temperature, max_tokens, max_turns, max_depth,
            tools, can_spawn, prompt, prompt_file, scope,
        } => add(
            &name, &model, temperature, max_tokens, max_turns, max_depth,
            &parse_csv(tools), &parse_csv(can_spawn), prompt, prompt_file, &scope,
        ),
        AgentCommands::Edit { name, scope } => edit(&name, &scope),
        AgentCommands::Remove { name, scope } => remove(&name, scope),
    }
}

fn list(scope_filter: &str) {
    let registry = build_registry();
    let agents: Vec<_> = if scope_filter == "all" {
        registry.list().into_iter().collect()
    } else {
        let scope = match scope_filter {
            "builtin" => AgentScope::Builtin,
            "global" => AgentScope::Global,
            "project" => AgentScope::Project,
            _ => {
                eprintln!("{}: unknown scope '{}'. Use: all, builtin, global, project", "error".red(), scope_filter);
                return;
            }
        };
        registry.list_by_scope(scope).into_iter().collect()
    };

    if agents.is_empty() {
        println!("No agents found in scope '{}'.", scope_filter);
        return;
    }

    println!("{} ({})", "Agents".bold(), scope_filter);
    println!("{}", "─".repeat(80));
    println!("{:<20} {:<10} {:<10} {:<6} {:<10} {}", "NAME", "SCOPE", "MODEL", "DEPTH", "TOOLS", "CAN_SPAWN");
    println!("{}", "─".repeat(80));
    for a in &agents {
        let tools = if a.definition.tools().is_empty() {
            "—".to_string()
        } else {
            a.definition.tools().join(", ")
        };
        let can_spawn = if a.definition.can_spawn().is_empty() {
            "—".to_string()
        } else {
            a.definition.can_spawn().join(", ")
        };
        println!(
            "{:<20} {:<10} {:<10} {:<6} {:<10} {}",
            a.name(),
            a.scope.as_str(),
            a.definition.model(),
            a.definition.max_depth(),
            tools,
            can_spawn,
        );
    }
    println!("\n{} agent(s) total.", agents.len());
}

fn show(name: &str) {
    let registry = build_registry();
    let Some(agent) = registry.resolve(name) else {
        eprintln!("{}: agent '{}' not found.", "error".red(), name);
        return;
    };

    println!("{}", agent.name().bold());
    println!("{}", "─".repeat(60));
    println!("Scope:          {}", agent.scope.as_str());
    println!("Model:          {}", agent.definition.model());
    println!("Temperature:    {}", agent.definition.frontmatter.temperature);
    println!("Max tokens:     {}", agent.definition.frontmatter.max_tokens);
    println!("Max turns:      {}", agent.definition.max_turns());
    println!("Max depth:      {}", agent.definition.max_depth());
    println!("Tools:          {}", if agent.definition.tools().is_empty() { "—".into() } else { agent.definition.tools().join(", ") });
    println!("Can spawn:      {}", if agent.definition.can_spawn().is_empty() { "—".into() } else { agent.definition.can_spawn().join(", ") });
    if let Some(path) = &agent.path {
        println!("File:           {}", path.display());
    }
    println!();
    println!("{}", "System Prompt:".bold());
    println!();
    println!("{}", agent.definition.system_prompt());
}

fn add(
    name: &str,
    model: &str,
    temperature: f32,
    max_tokens: u32,
    max_turns: u32,
    max_depth: u8,
    tools: &[String],
    can_spawn: &[String],
    prompt: Option<String>,
    prompt_file: Option<std::path::PathBuf>,
    scope: &str,
) {
    // Get system prompt
    let system_prompt = if let Some(file) = prompt_file {
        match std::fs::read_to_string(&file) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("{}: failed to read prompt file {}: {}", "error".red(), file.display(), e);
                return;
            }
        }
    } else if let Some(p) = prompt {
        p
    } else {
        eprintln!("{}: --prompt or --prompt-file is required.", "error".red());
        return;
    };

    if system_prompt.is_empty() {
        eprintln!("{}: system prompt is empty.", "error".red());
        return;
    }

    // Determine target directory
    let dir = if scope == "global" {
        AgentRegistry::global_dir()
    } else if scope == "project" {
        project_agents_dir()
    } else {
        eprintln!("{}: scope must be 'project' or 'global', got '{}'.", "error".red(), scope);
        return;
    };

    // Create directory
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("{}: failed to create agents dir {}: {}", "error".red(), dir.display(), e);
        return;
    }

    let file_path = dir.join(format!("{name}.md"));
    if file_path.exists() {
        eprintln!("{}: agent '{}' already exists in {} scope ({}).", "error".red(), name, scope, file_path.display());
        eprintln!("Use {} to modify it.", "praxis agent edit".cyan());
        return;
    }

    let content = serialize_agent_md(name, model, temperature, max_tokens, max_turns, max_depth, tools, can_spawn, &system_prompt);
    if let Err(e) = std::fs::write(&file_path, &content) {
        eprintln!("{}: failed to write agent file: {}", "error".red(), e);
        return;
    }

    println!("{}: agent '{}' created in {} scope ({}).", "done".green(), name, scope, file_path.display());
}

fn edit(name: &str, scope: &str) {
    // Determine file path
    let dir = if scope == "global" {
        AgentRegistry::global_dir()
    } else {
        project_agents_dir()
    };
    let file_path = dir.join(format!("{name}.md"));

    if !file_path.exists() {
        // Maybe it's a built-in — clone it to the target scope first
        let registry = build_registry();
        if let Some(agent) = registry.resolve(name) {
            if agent.scope == AgentScope::Builtin {
                // Clone built-in to the target scope
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    eprintln!("{}: failed to create agents dir: {}", "error".red(), e);
                    return;
                }
                let content = serialize_agent_md(
                    name,
                    agent.definition.model(),
                    agent.definition.frontmatter.temperature,
                    agent.definition.frontmatter.max_tokens,
                    agent.definition.max_turns(),
                    agent.definition.max_depth(),
                    agent.definition.tools(),
                    agent.definition.can_spawn(),
                    agent.definition.system_prompt(),
                );
                if let Err(e) = std::fs::write(&file_path, &content) {
                    eprintln!("{}: failed to clone built-in agent: {}", "error".red(), e);
                    return;
                }
                println!("Cloned built-in '{}' to {} scope for editing.", name, scope);
            } else {
                eprintln!("{}: agent '{}' found in {} scope but file is at {}.",
                    "error".red(), name, agent.scope.as_str(), file_path.display());
                return;
            }
        } else {
            eprintln!("{}: agent '{}' not found.", "error".red(), name);
            return;
        }
    }

    // Open in $EDITOR
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
        if cfg!(windows) { "notepad".to_string() } else { "nano".to_string() }
    });

    println!("Opening {} in {}...", file_path.display(), editor);

    let status = std::process::Command::new(&editor)
        .arg(&file_path)
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("{}: agent '{}' updated.", "done".green(), name);
        }
        Ok(_) => {
            eprintln!("{}: editor exited with error.", "error".red());
        }
        Err(e) => {
            eprintln!("{}: failed to launch editor '{}': {}", "error".red(), editor, e);
        }
    }
}

fn remove(name: &str, scope: Option<String>) {
    // Try specified scope, or project first, then global
    let project_dir = project_agents_dir();
    let global_dir = AgentRegistry::global_dir();

    let (path, scope_name) = if let Some(s) = scope {
        let dir = if s == "global" { global_dir } else { project_dir };
        let path = dir.join(format!("{name}.md"));
        if !path.exists() {
            eprintln!("{}: agent '{}' not found in {} scope.", "error".red(), name, s);
            return;
        }
        (path, s)
    } else {
        let project_path = project_dir.join(format!("{name}.md"));
        if project_path.exists() {
            (project_path, "project".to_string())
        } else {
            let global_path = global_dir.join(format!("{name}.md"));
            if global_path.exists() {
                (global_path, "global".to_string())
            } else {
                eprintln!("{}: agent '{}' not found in project or global scope.", "error".red(), name);
                eprintln!("Built-in agents cannot be removed.");
                return;
            }
        }
    };

    match std::fs::remove_file(&path) {
        Ok(_) => {
            println!("{}: agent '{}' removed from {} scope ({}).", "done".green(), name, scope_name, path.display());
        }
        Err(e) => {
            eprintln!("{}: failed to remove agent: {}", "error".red(), e);
        }
    }
}
