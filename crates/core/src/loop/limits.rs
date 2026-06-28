//! Configurable hard limits for the execution loop.

pub struct Limits {
    pub max_iterations_per_goal: u32,
    pub max_iterations_per_phase: u32,
    pub session_ttl_seconds: u64,
    pub phase_timeout_seconds: u64,
    pub tool_timeout_seconds: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_iterations_per_goal: 50,
            max_iterations_per_phase: 5,
            session_ttl_seconds: 3600,
            phase_timeout_seconds: 300,
            tool_timeout_seconds: 30,
        }
    }
}