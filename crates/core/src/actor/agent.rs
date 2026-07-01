//! Agent trait — Contract for all specialized agents in the system.
//!
//! Every agent (Architect, Coder, Reviewer, Security, etc.) implements
//! this trait to provide a uniform interface for the Orchestrator.

use async_trait::async_trait;

/// A human-readable identifier for the agent's role.
pub type AgentRole = &'static str;

/// The current state of an agent's execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Running,
    Blocked(String),
    Degraded(String),
    Failed(String),
}

/// Information about an agent for monitoring/observability.
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub id: String,
    pub role: AgentRole,
    pub model: String,
    pub status: AgentStatus,
    pub asi_score: f32,
    pub context_pressure: f32,
    pub current_action: String,
    pub message_count: u64,
    pub uptime_seconds: u64,
}

/// A task to be executed by an agent.
#[derive(Debug, Clone)]
pub struct AgentTask {
    pub id: String,
    pub description: String,
    pub context: String,
    pub max_iterations: u32,
}

/// Output produced by an agent after executing a task.
#[derive(Debug, Clone)]
pub struct AgentOutput {
    pub task_id: String,
    pub content: String,
    pub success: bool,
    pub token_usage: praxis_shared::types::TokenUsage,
}

/// The core Agent trait — all specialized agents implement this.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Return the agent's role name (e.g., "architect", "coder").
    fn role(&self) -> AgentRole;

    /// Return the model this agent uses.
    fn model(&self) -> &str;

    /// Return the current ASI score.
    fn asi_score(&self) -> f32;

    /// Return the current context pressure (0.0 – 1.0).
    fn context_pressure(&self) -> f32;

    /// Execute a task and return the output.
    async fn execute(&self, task: AgentTask) -> crate::Result<AgentOutput>;

    /// Handle feedback (e.g., from a reviewer) and produce revised output.
    async fn handle_feedback(&self, task: AgentTask, feedback: &str) -> crate::Result<AgentOutput>;

    /// Reset the agent's internal context (e.g., after drift detection).
    async fn reset_context(&self) -> crate::Result<()>;
}