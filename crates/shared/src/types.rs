//! Shared type aliases and core enums for the Project-X system.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── ID Type Aliases ───────────────────────────────────────────

pub type AgentId = String;
pub type SessionId = Uuid;
pub type ProjectId = Uuid;
pub type GoalId = String;
pub type TaskId = Uuid;
pub type PhaseId = String;
pub type ConversationId = Uuid;
pub type OrganizationId = Uuid;
pub type TeamId = Uuid;
pub type UserId = Uuid;
pub type EventId = Uuid;

// ─── Priority ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

impl Priority {
    pub fn ordering(&self) -> u8 {
        match self {
            Priority::Low => 0,
            Priority::Normal => 1,
            Priority::High => 2,
            Priority::Critical => 3,
        }
    }
}

// ─── Phases ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Idle,
    Planning,
    Researching,
    Designing,
    Implementing,
    Reviewing,
    Fixing,
    Testing,
    SecurityScan,
    Finalizing,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ─── Transition ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub from: Phase,
    pub to: Phase,
    pub gate: Option<String>,
    pub condition: TransitionCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransitionCondition {
    Automatic,
    AllAgentsComplete,
    GatePassed(String),
    UserApproval,
    MaxIterationsReached,
}

// ─── Gate ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_name: String,
    pub passed: bool,
    pub details: String,
    pub evaluator: String,
}

// ─── Agent Status ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: AgentId,
    pub role: String,
    pub model: String,
    pub status: AgentStatus,
    pub asi_score: f32,
    pub current_action: String,
    pub context_pressure: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Running,
    Blocked,
    Degraded,
    Failed,
}

// ─── Model Information ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub provider: String,
    pub context_window: usize,
    pub hard_limit_pct: f32,
    pub max_output_tokens: u32,
    pub supports_streaming: bool,
    pub supports_embeddings: bool,
}

// ─── Token Usage ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

impl TokenUsage {
    pub fn new(input: u32, output: u32) -> Self {
        Self {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
        }
    }
}

// ─── Session State ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub id: SessionId,
    pub project_id: ProjectId,
    pub status: SessionStatus,
    pub goal: String,
    pub current_phase: Phase,
    pub iteration: u32,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Paused,
    Completed,
    Failed,
    Cancelled,
}
