//! State machine: phase definitions, transitions, gates.

pub mod gate;
pub mod phase;
pub mod transition;

pub use gate::{
    Gate, GateEvaluator, GateRegistry, GateVerdict, ReviewComment, ReviewResult, Severity,
};
pub use phase::{Phase, PhaseTransition, StateMachine};
pub use transition::{Transition, TransitionCondition};
