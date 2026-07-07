//! Memory subsystem — three-tier memory architecture.
//!
//! - Hot memory: in-process DashMap + moka cache (session state, context window)
//! - Episodic memory: Qdrant embedded (vector search, cross-session recall)
//! - Consolidated memory: compressed summaries (long-term retention)
//!
//! # Phase 5 additions
//!
//! - EmbeddingService: wraps an LLM provider's `embed()` with batching + caching.
//! - Chunker: splits long text into chunks for embedding + indexing.
//! - Qdrant-backed EpisodicMemory: replaces the in-memory store with Qdrant.

pub mod cache;
pub mod chunker;
pub mod consolidated;
pub mod context;
pub mod embedding;
pub mod episodic;
pub mod hot;

pub use cache::{CacheStats, CachedResponse, LLMCache};
pub use chunker::{Chunk, ChunkBoundary, ChunkStrategy, Chunker, chunk_text, chunk_text_auto};
pub use consolidated::{
    ConsolidatedMemory, ConsolidatedStats, ErrorLearned, InteractionKind, InteractionSummary,
    KeyDecision, MemorySummary, Summarizer,
};
pub use context::{
    BudgetProfile, CompressionPipeline, CompressionResult, ContextBudget, ContextHealth,
    ContextManager, ContextWindow, HealthStatus, Message, Section, TokenCounter,
};
pub use embedding::{EmbeddingService, EmbeddingStats};
pub use episodic::{
    ChunkMetadata, ChunkType, EpisodicMemory, EpisodicStats, MemoryChunk,
    SearchResult as EpisodicSearchResult, cosine_similarity,
};
pub use hot::{HotMemory, HotMemoryStats, Interaction, SessionState, SessionStatus, SlidingWindow};
