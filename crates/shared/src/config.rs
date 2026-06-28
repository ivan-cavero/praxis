//! Configuration types deserialized from TOML files.
//!
//! These structs map directly to forge.toml and config.toml.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectMeta,
    pub providers: Option<std::collections::HashMap<String, ProviderConfig>>,
    pub roles: Option<std::collections::HashMap<String, RoleConfig>>,
    pub goals: Option<Vec<GoalConfig>>,
    pub limits: Option<LimitsConfig>,
    pub mcp_servers: Option<Vec<McpServerConfig>>,
    pub storage: Option<StorageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub timeout_seconds: Option<u64>,
    pub max_retries: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleConfig {
    pub description: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub system_prompt: Option<String>,
    pub tools: Option<Vec<String>>,
    pub context_profile: Option<String>,
    pub context_priority: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalConfig {
    pub name: String,
    pub description: Option<String>,
    pub agents: Option<Vec<String>>,
    pub gates: Option<Vec<String>>,
    pub max_iterations: Option<u32>,
    pub parallel_reviewers: Option<u32>,
    pub workflow: Option<String>,
    pub agent_overrides: Option<std::collections::HashMap<String, RoleOverride>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleOverride {
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub system_prompt: Option<String>,
    pub context_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    pub max_iterations_per_goal: Option<u32>,
    pub max_iterations_per_phase: Option<u32>,
    pub session_ttl_seconds: Option<u64>,
    pub phase_timeout_seconds: Option<u64>,
    pub tool_timeout_seconds: Option<u64>,
    pub divergence_threshold: Option<f32>,
    pub consolidation_interval: Option<u32>,
    pub context_pressure_warning: Option<f32>,
    pub context_pressure_critical: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub mode: Option<String>,
    pub remote: Option<RemoteStorageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteStorageConfig {
    pub postgres_url: Option<String>,
    pub redis_url: Option<String>,
    pub qdrant_url: Option<String>,
}
