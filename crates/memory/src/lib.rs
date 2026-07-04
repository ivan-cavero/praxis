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

pub mod hot;
pub mod episodic;
pub mod consolidated;
pub mod cache;
pub mod context;
pub mod embedding;
pub mod chunker;

pub use hot::{HotMemory, Interaction, SessionState, SessionStatus, SlidingWindow, HotMemoryStats};
pub use cache::{LLMCache, CachedResponse, CacheStats};
pub use context::{
    TokenCounter, BudgetProfile, ContextBudget, ContextManager,
    CompressionPipeline, CompressionResult, ContextHealth, HealthStatus,
    ContextWindow, Message, Section,
};
pub use episodic::{
    EpisodicMemory, MemoryChunk, ChunkMetadata, ChunkType,
    SearchResult as EpisodicSearchResult, EpisodicStats, cosine_similarity,
};
pub use consolidated::{
    ConsolidatedMemory, MemorySummary, KeyDecision, ErrorLearned,
    Summarizer, InteractionSummary, InteractionKind, ConsolidatedStats,
};
pub use embedding::{EmbeddingService, EmbeddingStats};
pub use chunker::{Chunker, Chunk, ChunkBoundary, ChunkStrategy, chunk_text, chunk_text_auto};