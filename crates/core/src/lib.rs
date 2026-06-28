//! Project-X Core Runtime
//!
//! The heart of the system: actor model, state machine, orchestrator,
//! loop controller, drift detection, and context management.

pub mod actor;
pub mod machine;
pub mod r#loop;
pub mod drift;
pub mod workflow;

pub use actor::*;
pub use machine::*;
pub use r#loop::*;
pub use drift::*;
pub use workflow::*;
