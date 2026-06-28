//! Memory subsystem — three-tier memory architecture.
//!
//! - Hot memory: in-process DashMap + moka cache (session state, context window)
//! - Episodic memory: Qdrant embedded (vector search, cross-session recall)
//! - Consolidated memory: compressed summaries (long-term retention)

pub mod hot;
pub mod episodic;
pub mod consolidated;

pub use hot::*;
pub use episodic::*;
pub use consolidated::*;
