//! Loop controller — the main execution loop.

pub mod controller;
pub mod limits;
pub mod pathology;

pub use controller::{LoopController, Limits, LimitViolation, LoopEvent, PhaseInfo};
pub use pathology::{
    LoopPathologyDetector, PathologyAlert, PathologyKind, PathologySeverity, PathologyAction,
};