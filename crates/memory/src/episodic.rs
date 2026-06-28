//! Episodic memory: Qdrant embedded vector database.
//!
//! Stores conversation chunks as embeddings for semantic search.
//! Enables cross-session memory recall via RAG.

use std::collections::VecDeque;

/// A chunk of content stored in episodic memory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryChunk {
    pub id: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub metadata: ChunkMetadata,
    pub score: Option<f32>,
}

/// Metadata attached to each memory chunk.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkMetadata {
    pub session_id: String,
    pub project_id: String,
    pub agent_id: String,
    pub chunk_type: ChunkType,
    pub timestamp: String,
    pub token_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChunkType {
    Conversation,
    Code,
    Decision,
    Research,
    Error,
    Summary,
}

/// Result of a semantic search.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub chunk: MemoryChunk,
    pub score: f32,
}

/// Episodic memory with vector search.
pub struct EpisodicMemory {
    /// In-memory vector storage (Qdrant embedded in production).
    chunks: Vec<MemoryChunk>,
    /// Maximum chunks to keep in memory.
    max_chunks: usize,
    /// Chunks ordered by recency.
    recency: VecDeque<String>,
}

impl EpisodicMemory {
    /// Create a new episodic memory store.
    pub fn new(max_chunks: usize) -> Self {
        Self {
            chunks: Vec::new(),
            max_chunks,
            recency: VecDeque::with_capacity(max_chunks),
        }
    }

    /// Create with default capacity (1000 chunks).
    pub fn default_store() -> Self {
        Self::new(1000)
    }

    /// Store a new memory chunk.
    pub fn store(&mut self, chunk: MemoryChunk) {
        // Add to chunks
        self.chunks.push(chunk.clone());
        self.recency.push_back(chunk.id.clone());

        // Evict oldest if over capacity
        while self.chunks.len() > self.max_chunks {
            if let Some(oldest_id) = self.recency.pop_front() {
                self.chunks.retain(|c| c.id != oldest_id);
            }
        }
    }

    /// Search for chunks similar to a query embedding.
    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Vec<SearchResult> {
        if self.chunks.is_empty() || query_embedding.is_empty() {
            return Vec::new();
        }

        // Calculate cosine similarity for each chunk
        let mut results: Vec<SearchResult> = self
            .chunks
            .iter()
            .map(|chunk| {
                let score = cosine_similarity(query_embedding, &chunk.embedding);
                SearchResult {
                    chunk: chunk.clone(),
                    score,
                }
            })
            .collect();

        // Sort by score (highest first)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Return top N
        results.truncate(limit);
        results
    }

    /// Search with text query (for simplicity, uses content matching).
    pub fn search_text(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut results: Vec<SearchResult> = self
            .chunks
            .iter()
            .filter_map(|chunk| {
                let content_lower = chunk.content.to_lowercase();
                let match_count = query_words
                    .iter()
                    .filter(|word| content_lower.contains(**word))
                    .count();

                if match_count > 0 {
                    let score = match_count as f32 / query_words.len() as f32;
                    Some(SearchResult {
                        chunk: chunk.clone(),
                        score,
                    })
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }

    /// Get recent chunks.
    pub fn recent(&self, limit: usize) -> Vec<&MemoryChunk> {
        self.recency
            .iter()
            .rev()
            .take(limit)
            .filter_map(|id| self.chunks.iter().find(|c| &c.id == id))
            .collect()
    }

    /// Get chunks by session.
    pub fn by_session(&self, session_id: &str) -> Vec<&MemoryChunk> {
        self.chunks
            .iter()
            .filter(|c| c.metadata.session_id == session_id)
            .collect()
    }

    /// Get chunks by type.
    pub fn by_type(&self, chunk_type: &ChunkType) -> Vec<&MemoryChunk> {
        self.chunks
            .iter()
            .filter(|c| c.metadata.chunk_type == *chunk_type)
            .collect()
    }

    /// Total chunks stored.
    pub fn count(&self) -> usize {
        self.chunks.len()
    }

    /// Clear all chunks.
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.recency.clear();
    }

    /// Get memory statistics.
    pub fn stats(&self) -> EpisodicStats {
        EpisodicStats {
            total_chunks: self.chunks.len(),
            max_chunks: self.max_chunks,
            by_type: self.chunks.iter().fold(
                std::collections::HashMap::new(),
                |mut acc, c| {
                    *acc.entry(format!("{:?}", c.metadata.chunk_type))
                        .or_insert(0) += 1;
                    acc
                },
            ),
        }
    }
}

/// Cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

/// Memory statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EpisodicStats {
    pub total_chunks: usize,
    pub max_chunks: usize,
    pub by_type: std::collections::HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_search() {
        let mut memory = EpisodicMemory::new(10);

        let chunk = MemoryChunk {
            id: "1".to_string(),
            content: "Hello world".to_string(),
            embedding: vec![1.0, 0.0, 0.0],
            metadata: ChunkMetadata {
                session_id: "s1".to_string(),
                project_id: "p1".to_string(),
                agent_id: "a1".to_string(),
                chunk_type: ChunkType::Conversation,
                timestamp: chrono::Utc::now().to_rfc3339(),
                token_count: 5,
            },
            score: None,
        };

        memory.store(chunk);

        // Search with similar embedding
        let results = memory.search(&[1.0, 0.0, 0.0], 5);
        assert_eq!(results.len(), 1);
        assert!(results[0].score > 0.9);
    }

    #[test]
    fn test_search_text() {
        let mut memory = EpisodicMemory::new(10);

        memory.store(MemoryChunk {
            id: "1".to_string(),
            content: "Rust is a systems programming language".to_string(),
            embedding: vec![],
            metadata: ChunkMetadata {
                session_id: "s1".to_string(),
                project_id: "p1".to_string(),
                agent_id: "a1".to_string(),
                chunk_type: ChunkType::Research,
                timestamp: chrono::Utc::now().to_rfc3339(),
                token_count: 10,
            },
            score: None,
        });

        memory.store(MemoryChunk {
            id: "2".to_string(),
            content: "Python is great for data science".to_string(),
            embedding: vec![],
            metadata: ChunkMetadata {
                session_id: "s1".to_string(),
                project_id: "p1".to_string(),
                agent_id: "a1".to_string(),
                chunk_type: ChunkType::Research,
                timestamp: chrono::Utc::now().to_rfc3339(),
                token_count: 8,
            },
            score: None,
        });

        let results = memory.search_text("Rust programming", 5);
        assert_eq!(results.len(), 1);
        assert!(results[0].score > 0.5);
        assert!(results[0].chunk.id == "1");
    }

    #[test]
    fn test_eviction() {
        let mut memory = EpisodicMemory::new(3);

        for i in 0..5 {
            memory.store(MemoryChunk {
                id: i.to_string(),
                content: format!("Chunk {}", i),
                embedding: vec![i as f32],
                metadata: ChunkMetadata {
                    session_id: "s1".to_string(),
                    project_id: "p1".to_string(),
                    agent_id: "a1".to_string(),
                    chunk_type: ChunkType::Conversation,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    token_count: 5,
                },
                score: None,
            });
        }

        assert_eq!(memory.count(), 3);
        // Oldest chunks should be evicted
        let ids: Vec<&str> = memory.chunks.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"4"));
        assert!(ids.contains(&"3"));
        assert!(ids.contains(&"2"));
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_by_session() {
        let mut memory = EpisodicMemory::new(10);

        for i in 0..5 {
            memory.store(MemoryChunk {
                id: i.to_string(),
                content: format!("Chunk {}", i),
                embedding: vec![],
                metadata: ChunkMetadata {
                    session_id: if i < 3 { "s1".to_string() } else { "s2".to_string() },
                    project_id: "p1".to_string(),
                    agent_id: "a1".to_string(),
                    chunk_type: ChunkType::Conversation,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    token_count: 5,
                },
                score: None,
            });
        }

        let s1_chunks = memory.by_session("s1");
        assert_eq!(s1_chunks.len(), 3);
    }
}