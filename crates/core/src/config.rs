//! Forge configuration parsing and runtime config types.
//!
//! `praxis_shared::config::ProjectConfig` is the canonical serde schema for
//! `forge.toml`. [`ForgeConfig`] is the runtime view used by the loop. The
//! two are unified via [`ForgeConfig::from_project_config`], the single
//! source of truth for the mapping.

use crate::r#loop::Limits;
use crate::orchestrator;
use crate::{CoreError, Result};

// ─── Config ───────────────────────────────────────────────────

/// Parsed forge.toml configuration.
pub struct ForgeConfig {
    pub roles: std::collections::HashMap<String, orchestrator::RoleConfig>,
    pub goals: Vec<orchestrator::GoalConfig>,
    pub mcp_servers: Vec<McpServerConfig>,
    /// Provider definitions from [providers.*] sections. Key is provider name.
    pub providers: std::collections::HashMap<String, ProviderConfig>,
    /// Hard limits from [limits] section. Applied to LoopController in run_goal.
    pub limits: Option<Limits>,
    /// Named workflow definitions from [[workflows]] sections.
    /// Goals reference a workflow by name. When empty, the default linear
    /// phase sequence is used.
    pub workflows: Vec<praxis_shared::config::WorkflowDefinition>,
}

/// Provider configuration from forge.toml [providers.*].
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key_ref: String, // "env:VAR" | "vault:provider_name" | "literal-key"
    pub default_model: String,
}

/// MCP server configuration from forge.toml.
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

impl Default for ForgeConfig {
    fn default() -> Self {
        // Deprecated — use ForgeConfig::empty() instead.
        // Default impl kept for backward compatibility with tests.
        Self::empty()
    }
}

impl ForgeConfig {
    /// Create an empty config (no roles, no providers, no goals).
    /// Used when no forge.toml exists — agents run in mock mode.
    pub fn empty() -> Self {
        Self {
            roles: std::collections::HashMap::new(),
            goals: Vec::new(),
            mcp_servers: Vec::new(),
            providers: std::collections::HashMap::new(),
            limits: None,
            workflows: Vec::new(),
        }
    }
}

/// Load forge.toml configuration from a file.
///
/// Uses `praxis_shared::config::ProjectConfig` (serde-based) for parsing,
/// then converts to `ForgeConfig` for internal use. This unifies the two
/// config systems: `shared::config` defines the schema, `core::ForgeConfig`
/// is the runtime view.
pub fn load_forge_config(path: &std::path::Path) -> Result<ForgeConfig> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| CoreError::Config(format!("Failed to read {}: {}", path.display(), e)))?;

    let project_config: praxis_shared::config::ProjectConfig = toml::from_str(&content)
        .map_err(|e| CoreError::Config(format!("Failed to parse {}: {}", path.display(), e)))?;

    Ok(ForgeConfig::from_project_config(project_config))
}

impl ForgeConfig {
    /// Convert from `praxis_shared::config::ProjectConfig` (the canonical serde type)
    /// to `ForgeConfig` (the runtime view used by the loop).
    ///
    /// This is the single source of truth for the mapping between the two systems.
    pub fn from_project_config(pc: praxis_shared::config::ProjectConfig) -> Self {
        // Roles: shared RoleConfig (Option fields) → orchestrator RoleConfig (with defaults)
        let roles = pc
            .roles
            .unwrap_or_default()
            .into_iter()
            .map(|(name, shared_role)| {
                let role = orchestrator::RoleConfig {
                    name: name.clone(),
                    description: shared_role.description,
                    model: shared_role.model.unwrap_or_else(|| "gpt-4o".to_string()),
                    temperature: shared_role.temperature.unwrap_or(0.3),
                    max_tokens: shared_role.max_tokens.unwrap_or(4096),
                    system_prompt: shared_role.system_prompt,
                    tools: shared_role.tools.unwrap_or_default(),
                    context_profile: shared_role.context_profile,
                    context_priority: shared_role.context_priority,
                };
                (name, role)
            })
            .collect();

        // Providers: shared ProviderConfig (Option fields) → core ProviderConfig
        let providers = pc
            .providers
            .unwrap_or_default()
            .into_iter()
            .map(|(name, shared_provider)| {
                let provider = ProviderConfig {
                    name: name.clone(),
                    base_url: shared_provider
                        .base_url
                        .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                    api_key_ref: shared_provider.api_key.unwrap_or_default(),
                    default_model: shared_provider
                        .default_model
                        .unwrap_or_else(|| "gpt-4o".to_string()),
                };
                (name, provider)
            })
            .collect();

        // Goals: shared GoalConfig → orchestrator GoalConfig
        let goals = pc
            .goals
            .unwrap_or_default()
            .into_iter()
            .map(|shared_goal| orchestrator::GoalConfig {
                name: shared_goal.name,
                description: shared_goal.description,
                agents: shared_goal.agents.unwrap_or_default(),
                gates: shared_goal.gates.unwrap_or_default(),
                max_iterations: shared_goal.max_iterations,
                parallel_reviewers: shared_goal.parallel_reviewers,
                workflow: shared_goal.workflow,
                agent_overrides: None, // TODO: wire agent_overrides from shared
            })
            .collect();

        // MCP servers: shared McpServerConfig → core McpServerConfig
        let mcp_servers = pc
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .map(|shared_server| McpServerConfig {
                name: shared_server.name,
                command: shared_server.command,
                args: shared_server.args.unwrap_or_default(),
            })
            .collect();

        // Limits: shared LimitsConfig → loop::Limits
        let limits = pc.limits.map(|shared_limits| {
            let defaults = Limits::default();
            Limits {
                max_iterations_per_goal: shared_limits
                    .max_iterations_per_goal
                    .unwrap_or(defaults.max_iterations_per_goal),
                max_iterations_per_phase: shared_limits
                    .max_iterations_per_phase
                    .unwrap_or(defaults.max_iterations_per_phase),
                session_ttl_seconds: shared_limits
                    .session_ttl_seconds
                    .unwrap_or(defaults.session_ttl_seconds),
                phase_timeout_seconds: shared_limits
                    .phase_timeout_seconds
                    .unwrap_or(defaults.phase_timeout_seconds),
                cycle_detection_window: defaults.cycle_detection_window,
                max_tokens: shared_limits.max_tokens,
                max_cost_usd: shared_limits.max_cost_usd,
            }
        });

        ForgeConfig {
            roles,
            goals,
            mcp_servers,
            providers,
            limits,
            workflows: pc.workflows.unwrap_or_default(),
        }
    }
}
