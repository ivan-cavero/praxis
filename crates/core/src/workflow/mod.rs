//! Workflow definitions: goal-to-phase mapping, agent assignment.

pub mod goal;
#[allow(clippy::module_inception)]
pub mod workflow;

pub use goal::*;
pub use workflow::*;
