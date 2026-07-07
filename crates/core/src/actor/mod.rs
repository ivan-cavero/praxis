//! Actor definitions: Supervisor, EchoAgent, and role stubs.

pub mod agent;
pub mod echo;
pub mod roles;
pub mod supervisor;

pub use agent::{Agent, AgentInfo, AgentOutput, AgentRole, AgentStatus, AgentTask};
pub use echo::{EchoAgent, EchoMessage, EchoStats, echo, get_stats, ping};
pub use supervisor::{
    AgentHandle, Supervisor, SupervisorMessage, SupervisorState, list_children, shutdown_all,
    spawn_echo, supervisor_echo_to,
};
