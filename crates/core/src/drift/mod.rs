//! Drift detection: Agent Stability Index (ASI) and recovery.
//!
//! Monitors agent behavior across 8 dimensions to detect quality degradation.
//! Based on research: https://arxiv.org/abs/2506.06190 (Quantifying Behavioral
//! Degradation in Multi-Agent LLM Systems).

pub mod asi;
pub mod metrics;
pub mod recovery;

pub use asi::ASICalculator;
pub use metrics::*;
pub use recovery::*;
