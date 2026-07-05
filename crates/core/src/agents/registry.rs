//! Agent registry — resolves agent definitions from 3 scopes.
//!
//! Resolution order (first wins): **project** > **global** > **built-in**.
//! A project can override a global or built-in agent by creating a `.md`
//! file with the same `name` in the project's `agents/` directory.

use std::path::{Path, PathBuf};
use std::collections::HashMap;

use super::definition::{AgentDefinition, BUILTIN_AGENTS, parse_agent_md};

/// The scope an agent definition was loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentScope {
    /// Compiled into the binary (read-only).
    Builtin,
    /// `~/.config/praxis/agents/*.md` — user's personal agents.
    Global,
    /// `<project_dir>/agents/*.md` — project-specific, checked into VCS.
    Project,
}

impl AgentScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Global => "global",
            Self::Project => "project",
        }
    }
}

/// A resolved agent definition with its source scope.
#[derive(Debug, Clone)]
pub struct ScopedAgent {
    pub definition: AgentDefinition,
    pub scope: AgentScope,
    /// File path if loaded from disk (None for built-in).
    pub path: Option<PathBuf>,
}

impl ScopedAgent {
    pub fn name(&self) -> &str { self.definition.name() }
}

/// Registry of agent definitions loaded from all scopes.
///
/// Resolution: project > global > built-in. Same name = closer scope wins.
#[derive(Debug, Clone)]
pub struct AgentRegistry {
    /// name → scoped agent (winning definition per name)
    agents: HashMap<String, ScopedAgent>,
}

impl AgentRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { agents: HashMap::new() }
    }

    /// Build a registry with only built-in agents.
    pub fn builtin_only() -> Self {
        let mut registry = Self::new();
        for (name, content) in BUILTIN_AGENTS {
            let def = parse_agent_md(content)
                .unwrap_or_else(|e| panic!("built-in agent '{name}' failed to parse: {e}"));
            registry.agents.insert(
                name.to_string(),
                ScopedAgent {
                    definition: def,
                    scope: AgentScope::Builtin,
                    path: None,
                },
            );
        }
        registry
    }

    /// Build a registry from built-in + global + project scopes.
    ///
    /// - `global_dir`: `~/.config/praxis/agents/` (None = skip global scope)
    /// - `project_dir`: `<project>/agents/` (None = skip project scope)
    pub fn load(global_dir: Option<&Path>, project_dir: Option<&Path>) -> Self {
        let mut registry = Self::builtin_only();

        // Load global scope (overrides built-in)
        if let Some(dir) = global_dir {
            load_dir_into(&mut registry, dir, AgentScope::Global);
        }

        // Load project scope (overrides global + built-in)
        if let Some(dir) = project_dir {
            load_dir_into(&mut registry, dir, AgentScope::Project);
        }

        registry
    }

    /// Resolve an agent by name. Returns the winning scoped definition.
    pub fn resolve(&self, name: &str) -> Option<&ScopedAgent> {
        self.agents.get(name)
    }

    /// List all registered agents.
    pub fn list(&self) -> Vec<&ScopedAgent> {
        let mut all: Vec<_> = self.agents.values().collect();
        all.sort_by(|a, b| a.name().cmp(b.name()));
        all
    }

    /// List agents filtered by scope.
    pub fn list_by_scope(&self, scope: AgentScope) -> Vec<&ScopedAgent> {
        let mut filtered: Vec<_> = self.agents.values().filter(|a| a.scope == scope).collect();
        filtered.sort_by(|a, b| a.name().cmp(b.name()));
        filtered
    }

    /// Insert or override an agent definition (used by CRUD API).
    pub fn insert(&mut self, agent: ScopedAgent) {
        self.agents.insert(agent.name().to_string(), agent);
    }

    /// Remove an agent from the registry (only non-builtin can be removed).
    pub fn remove(&mut self, name: &str) -> Option<ScopedAgent> {
        match self.agents.get(name) {
            Some(a) if a.scope == AgentScope::Builtin => None,
            _ => self.agents.remove(name),
        }
    }

    /// Check if an agent name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.agents.contains_key(name)
    }

    /// Resolve an agent to a `ResolvedRole` (for use with AgentFactory).
    /// Falls back to built-in if not found in project/global scope.
    pub fn resolve_role(&self, name: &str) -> Option<crate::orchestrator::roles::ResolvedRole> {
        self.resolve(name).map(|a| a.definition.to_resolved_role())
    }

    /// Get the global agents directory: `~/.config/praxis/agents/`.
    pub fn global_dir() -> PathBuf {
        if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
            PathBuf::from(config).join("praxis").join("agents")
        } else if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".config").join("praxis").join("agents")
        } else if let Some(appdata) = std::env::var_os("APPDATA") {
            PathBuf::from(appdata).join("praxis").join("agents")
        } else {
            PathBuf::from(".config").join("praxis").join("agents")
        }
    }

    /// Get the project agents directory: `<project_dir>/agents/`.
    pub fn project_dir(project_dir: &Path) -> PathBuf {
        project_dir.join("agents")
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::builtin_only()
    }
}

// ─── Helpers ──────────────────────────────────────────────────

/// Load all `.md` files from a directory into the registry, overriding
/// any existing entries with the same name.
fn load_dir_into(registry: &mut AgentRegistry, dir: &Path, scope: AgentScope) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return, // directory doesn't exist — fine
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to read agent file {}: {}", path.display(), e);
                continue;
            }
        };

        match parse_agent_md(&content) {
            Ok(def) => {
                let name = def.name().to_string();
                tracing::debug!("Loaded agent '{name}' from {} ({})", scope.as_str(), path.display());
                registry.insert(ScopedAgent {
                    definition: def,
                    scope,
                    path: Some(path),
                });
            }
            Err(e) => {
                tracing::warn!("Failed to parse agent file {}: {}", path.display(), e);
            }
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_builtin_only_has_all_agents() {
        let registry = AgentRegistry::builtin_only();
        assert_eq!(registry.list().len(), 8);
        for name in &["architect", "coder", "reviewer", "security", "tester", "git", "researcher", "explorer"] {
            assert!(registry.contains(name), "missing built-in agent: {name}");
        }
    }

    #[test]
    fn test_resolve_builtin() {
        let registry = AgentRegistry::builtin_only();
        let agent = registry.resolve("coder").unwrap();
        assert_eq!(agent.name(), "coder");
        assert_eq!(agent.scope, AgentScope::Builtin);
        assert!(agent.path.is_none());
    }

    #[test]
    fn test_resolve_unknown() {
        let registry = AgentRegistry::builtin_only();
        assert!(registry.resolve("nonexistent").is_none());
    }

    #[test]
    fn test_project_overrides_builtin() {
        let dir = temp_dir();
        let agents_dir = dir.path().join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let custom_coder = "---\nname: coder\nmodel: claude-sonnet-4-20250514\ntemperature: 0.1\n---\nYou are a custom coder.";
        let mut file = std::fs::File::create(agents_dir.join("coder.md")).unwrap();
        file.write_all(custom_coder.as_bytes()).unwrap();

        let registry = AgentRegistry::load(None, Some(&agents_dir));
        let agent = registry.resolve("coder").unwrap();
        assert_eq!(agent.scope, AgentScope::Project);
        assert_eq!(agent.definition.model(), "claude-sonnet-4-20250514");
        assert!(agent.path.is_some());
    }

    #[test]
    fn test_global_overrides_builtin() {
        let dir = temp_dir();
        let agents_dir = dir.path().join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let custom_reviewer = "---\nname: reviewer\nmodel: gpt-5\ntemperature: 0.05\n---\nYou are a strict reviewer.";
        let mut file = std::fs::File::create(agents_dir.join("reviewer.md")).unwrap();
        file.write_all(custom_reviewer.as_bytes()).unwrap();

        let registry = AgentRegistry::load(Some(&agents_dir), None);
        let agent = registry.resolve("reviewer").unwrap();
        assert_eq!(agent.scope, AgentScope::Global);
        assert_eq!(agent.definition.frontmatter.temperature, 0.05);
    }

    #[test]
    fn test_project_overrides_global() {
        let global_dir = temp_dir();
        let project_dir = temp_dir();

        let global_agents = global_dir.path().join("agents");
        std::fs::create_dir_all(&global_agents).unwrap();
        std::fs::write(
            global_agents.join("tester.md"),
            "---\nname: tester\nmodel: gpt-5\n---\nGlobal tester.",
        ).unwrap();

        let project_agents = project_dir.path().join("agents");
        std::fs::create_dir_all(&project_agents).unwrap();
        std::fs::write(
            project_agents.join("tester.md"),
            "---\nname: tester\nmodel: claude-sonnet-4-20250514\n---\nProject tester.",
        ).unwrap();

        let registry = AgentRegistry::load(Some(&global_agents), Some(&project_agents));
        let agent = registry.resolve("tester").unwrap();
        assert_eq!(agent.scope, AgentScope::Project);
        assert_eq!(agent.definition.model(), "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_list_by_scope() {
        let dir = temp_dir();
        let agents_dir = dir.path().join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(
            agents_dir.join("custom.md"),
            "---\nname: custom\nmodel: gpt-5\n---\nCustom agent.",
        ).unwrap();

        let registry = AgentRegistry::load(None, Some(&agents_dir));
        let builtin_count = registry.list_by_scope(AgentScope::Builtin).len();
        let project_count = registry.list_by_scope(AgentScope::Project).len();
        assert_eq!(builtin_count, 8);
        assert_eq!(project_count, 1);
    }

    #[test]
    fn test_remove_builtin_fails() {
        let mut registry = AgentRegistry::builtin_only();
        assert!(registry.remove("coder").is_none());
        assert!(registry.contains("coder"));
    }

    #[test]
    fn test_remove_project_succeeds() {
        let dir = temp_dir();
        let agents_dir = dir.path().join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(
            agents_dir.join("custom.md"),
            "---\nname: custom\nmodel: gpt-5\n---\nCustom.",
        ).unwrap();

        let mut registry = AgentRegistry::load(None, Some(&agents_dir));
        assert!(registry.remove("custom").is_some());
        assert!(!registry.contains("custom"));
    }

    #[test]
    fn test_load_nonexistent_dir_ok() {
        // Loading from a nonexistent directory should not panic
        let registry = AgentRegistry::load(
            Some(Path::new("/nonexistent/global")),
            Some(Path::new("/nonexistent/project")),
        );
        // Should still have all built-ins
        assert_eq!(registry.list().len(), 8);
    }

    #[test]
    fn test_global_dir_uses_env() {
        let dir = AgentRegistry::global_dir();
        assert!(dir.to_string_lossy().contains("praxis"));
        assert!(dir.to_string_lossy().contains("agents"));
    }
}
