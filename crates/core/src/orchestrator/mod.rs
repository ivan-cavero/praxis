//! Orchestrator — manages role-based agent spawning and task dispatch.
//!
//! The Orchestrator is the root actor that:
//! 1. Reads role configuration from TOML
//! 2. Spawns agents based on goal requirements
//! 3. Dispatches tasks to agents with timeout
//! 4. Collects and consolidates results
//! 5. Monitors agent health

pub mod injection;
pub mod roles;
pub mod task;
pub mod verification;

pub use injection::{
    Injection, InjectionChannel, InjectionError, InjectionPriority, InjectionSource,
    InjectionStats, InjectionType,
};
pub use roles::{GoalConfig, ResolvedRole, RoleConfig, RoleOverride};
pub use task::{Task, TaskResult, TaskStatus};
pub use verification::{
    AgentContextInfo, ConsensusConsolidator, ConsensusStrategy, ConsensusVerdict,
    CrossModelFeedbackLoop, ParallelExecutor, PerAgentContextTracker,
};
