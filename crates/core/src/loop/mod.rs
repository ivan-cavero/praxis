//! Loop controller — the main execution loop.

pub mod controller;
pub mod limits;
pub mod pathology;

pub use controller::{LimitViolation, Limits, LoopController, LoopEvent, PhaseInfo};
pub use pathology::{
    LoopPathologyDetector, PathologyAction, PathologyAlert, PathologyKind, PathologySeverity,
};
