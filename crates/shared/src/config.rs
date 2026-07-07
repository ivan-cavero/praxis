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
    /// Named workflow definitions. Goals reference a workflow by name via
    /// the `workflow` field. When absent, the default linear phase sequence
    /// (Planning → Designing → Implementing → Reviewing → Testing → …) is used.
    pub workflows: Option<Vec<WorkflowDefinition>>,
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
    /// Maximum total tokens across all agents in the session. None = no cap.
    pub max_tokens: Option<u64>,
    /// Maximum estimated cost in USD across all agents. None = no cap.
    pub max_cost_usd: Option<f64>,
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

// ─── Workflow Definitions ──────────────────────────────────────

/// A named workflow: an ordered list of phases with optional conditional
/// branching based on gate results.
///
/// Example TOML:
/// ```toml
/// [[workflows]]
/// name = "standard"
/// phases = [
///   { name = "Planning", agents = ["architect"] },
///   { name = "Implementing", agents = ["coder"] },
///   { name = "Reviewing", agents = ["reviewer"], gate = "review" },
///   { name = "Testing", agents = ["tester"], gate = "tests" },
/// ]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// Unique name referenced by `goal.workflow`.
    pub name: String,
    /// Ordered phase sequence. The first phase is the entry point.
    pub phases: Vec<WorkflowPhase>,
}

/// A single phase within a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPhase {
    /// Phase name — must match a `Phase` variant (e.g. "Planning", "Implementing").
    pub name: String,
    /// Agent role names to run in this phase.
    #[serde(default)]
    pub agents: Vec<String>,
    /// Gate name guarding entry to this phase. The gate must pass before
    /// the phase executes.
    pub gate: Option<String>,
    /// Branch rules evaluated after this phase completes. The first matching
    /// branch determines the next phase. If none match, the next phase in the
    /// list is used.
    #[serde(default)]
    pub branches: Vec<WorkflowBranch>,
    /// Run this phase's agents in parallel (default: false = sequential).
    #[serde(default)]
    pub parallel: bool,
}

/// A conditional branch from one phase to another.
///
/// The `on` condition is evaluated against the gate result of the phase
/// that owns this branch. If `on` is `GatePassed` and the phase's gate
/// passed, the workflow transitions to `to`. If `on` is `GateFailed` and
/// the gate failed, it transitions to `to`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowBranch {
    /// Condition to evaluate.
    pub on: BranchCondition,
    /// Target phase name.
    pub to: String,
}

/// Condition for a workflow branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BranchCondition {
    /// Branch taken when the phase's gate passed.
    GatePassed,
    /// Branch taken when the phase's gate failed.
    GateFailed,
    /// Branch taken unconditionally (always).
    Always,
}
