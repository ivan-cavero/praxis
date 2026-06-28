//! MemoryBackend trait — abstraction for memory storage.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A single memory entry stored in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
    pub metadata: std::collections::HashMap<String, String>,
    pub timestamp: String,
    pub session_id: String,
    pub project_id: String,
}

/// Result of a memory search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entry: MemoryEntry,
    pub score: f32,
}

/// The memory backend trait.
#[async_trait]
pub trait MemoryBackend: Send + Sync {
    /// Store a new memory entry.
    async fn store(&self, entry: MemoryEntry) -> crate::Result<()>;

    /// Search for entries similar to the query vector.
    async fn search(&self, query: &[f32], limit: usize) -> crate::Result<Vec<SearchResult>>;

    /// Remove entries matching the given filter.
    async fn delete(&self, filter: &std::collections::HashMap<String, String>) -> crate::Result<usize>;

    /// Return statistics about stored memory.
    async fn stats(&self) -> crate::Result<MemoryStats>;

    /// Optimize the underlying storage (e.g., merge segments).
    async fn optimize(&self) -> crate::Result<()>;
}

/// Memory storage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_entries: u64,
    pub total_vectors: u64,
    pub disk_usage_bytes: u64,
}