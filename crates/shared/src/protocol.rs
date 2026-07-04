//! Inter-agent communication protocol messages.
//!
//! Every message exchanged between agents, the orchestrator, and
//! external interfaces uses these types.

use crate::types::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The fundamental unit of communication in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub source: String,
    pub target: String,
    pub priority: Priority,
    pub timestamp_iso: String,
    pub ttl_seconds: u64,
    pub kind: MessageKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageKind {
    // ─── Commands ──────────────────────────────────────────
    AssignTask(TaskAssignment),
    ExecuteTool(ToolCall),
    CancelTask(TaskId),

    // ─── Results ───────────────────────────────────────────
    TaskResult(TaskOutput),
    TaskError(TaskErrorInfo),
    ToolResult(ToolOutput),

    // ─── Queries ───────────────────────────────────────────
    RequestInfo(InfoQuery),
    RequestReview(ReviewRequest),
    HealthCheck,

    // ─── Responses ─────────────────────────────────────────
    InfoResponse(InfoResponse),
    ReviewResult(ReviewVerdict),
    HealthStatus(HealthStatus),

    // ─── Agent Lifecycle Events ────────────────────────────
    AgentStarted {
        agent: String,
        role: String,
        phase: Phase,
    },
    AgentOutput {
        agent: String,
        delta: String,
    },
    AgentCompleted {
        agent: String,
        role: String,
        status: String,
        duration_ms: u64,
        output_preview: String,
    },

    // ─── System Events ─────────────────────────────────────
    PhaseChanged(PhaseTransition),
    CheckpointSaved(CheckpointInfo),
    PathologyDetected(PathologyAlert),
    DriftAlert(DriftAlert),
    SessionHeartbeat,

    // ─── Injection Events ──────────────────────────────────
    InjectionTriggered {
        target: String,
        phase: Phase,
        iteration: u32,
    },

    // ─── Metrics Events ────────────────────────────────────
    TokenUsed {
        provider: String,
        model: String,
        input: u32,
        output: u32,
    },
    ToolCalled {
        agent: String,
        tool: String,
        duration_ms: u64,
        success: bool,
    },
    GateResult(GateResult),

    // ─── Context Events ────────────────────────────────────
    ContextPressureAlert {
        pressure: f32,
        agent_id: String,
        action: String,
    },
    ContextCompression {
        before_tokens: usize,
        after_tokens: usize,
        ratio: f32,
        technique: String,
    },
}

// ─── Sub-types for MessageKind variants ────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub id: TaskId,
    pub goal_fragment: String,
    pub context: String,
    pub phase: Phase,
    pub max_iterations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutput {
    pub id: TaskId,
    pub content: String,
    pub tool_calls_made: Vec<String>,
    pub token_usage: TokenUsage,
    pub artifacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskErrorInfo {
    pub id: TaskId,
    pub error_type: String,
    pub message: String,
    pub recoverable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub input: serde_json::Value,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub success: bool,
    pub output: serde_json::Value,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoQuery {
    pub question: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoResponse {
    pub answer: String,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub code_diff: String,
    pub context: String,
    pub focus_areas: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewVerdict {
    pub passed: bool,
    pub comments: Vec<ReviewComment>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewComment {
    pub severity: ReviewSeverity,
    pub file: String,
    pub line: Option<u32>,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReviewSeverity {
    Critical,
    Major,
    Minor,
    Nit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub ok: bool,
    pub uptime_seconds: u64,
    pub last_action: String,
    pub asi_score: f32,
    pub context_pressure: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseTransition {
    pub from: Phase,
    pub to: Phase,
    pub condition: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub session_id: SessionId,
    pub phase: Phase,
    pub iteration: u32,
    pub token_usage: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftAlert {
    pub agent_id: Option<String>,
    pub old_asi: f32,
    pub new_asi: f32,
    pub dimension: String,
    pub severity: DriftSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DriftSeverity {
    Warning,
    Critical,
    Severe,
}

/// Alert from the loop pathology detector, published on the EventBus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathologyAlert {
    pub kind: String,
    pub severity: String,
    pub details: String,
    pub action: String,
    pub iteration: u32,
}

/// Wrapper for system-wide events (published on EventBus).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    pub id: Uuid,
    pub timestamp: String,
    pub kind: MessageKind,
    pub source: String,
    pub metadata: serde_json::Value,
}
