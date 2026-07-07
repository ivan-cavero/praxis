//! Workflow definitions: goal-to-phase mapping, agent assignment.
//!
//! - [`GoalEngine`]: resolves which workflow a goal uses.
//! - [`WorkflowEngine`]: drives the phase loop for a named workflow.
//! - [`GateOutcome`]: gate pass/fail result used for conditional branching.

pub mod goal;
pub mod engine;

pub use goal::GoalEngine;
pub use engine::{GateOutcome, WorkflowEngine, parse_phase};

// Re-export shared config types for convenience.
pub use praxis_shared::config::{
    BranchCondition, WorkflowBranch, WorkflowDefinition, WorkflowPhase,
};
