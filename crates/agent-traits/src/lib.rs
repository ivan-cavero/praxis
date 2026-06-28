//! Agent traits: LLMProvider, Tool, Memory, and Persistence backends.
//!
//! These traits define the contracts that all implementations must satisfy.

pub mod provider;
pub mod tool;
pub mod memory;
pub mod persistence;

pub mod prelude {
    pub use crate::memory::MemoryBackend;
    pub use crate::persistence::EventStore;
    pub use crate::provider::LLMProvider;
    pub use crate::tool::Tool;
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentTraitsError {
    #[error("Provider error: {0}")]
    Provider(String),
    #[error("Tool error: {0}")]
    Tool(String),
    #[error("Memory error: {0}")]
    Memory(String),
    #[error("Persistence error: {0}")]
    Persistence(String),
}

pub type Result<T> = std::result::Result<T, AgentTraitsError>;