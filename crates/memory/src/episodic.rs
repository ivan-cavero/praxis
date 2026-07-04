//! Episodic memory: in-memory vector store with optional Qdrant sync.
//!
//! Primary: in-memory `Vec<MemoryChunk>` with cosine similarity search.
//! Optional: sync to remote Qdrant server via `QdrantBackend`.
//!
//! When an `EmbeddingService` is configured, `store()` auto-generates
//! embeddings for chunks that don't have one.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::EmbeddingService;

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

// ─── Optional Qdrant Backend ──────────────────────────────────

/// Optional remote Qdrant server backend for persistent storage.
///
/// Only available when a Qdrant server URL is configured.
/// Uses the `qdrant-client` crate (gRPC).
#[derive(Debug, Clone)]
pub struct QdrantBackend {
    /// Qdrant server URL (e.g., "http://localhost:6334").
    pub url: String,
    /// Collection name for episodic chunks.
    pub collection: String,
    /// API key (if required).
    pub api_key: Option<String>,
}

impl QdrantBackend {
    /// Create a new Qdrant backend config.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            collection: "praxis_episodic".to_string(),
            api_key: None,
        }
    }

    /// Set the collection name.
    pub fn with_collection(mut self, name: impl Into<String>) -> Self {
        self.collection = name.into();
        self
    }

    /// Set an optional API key.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Build a Qdrant client from this config.
    pub async fn build_client(&self) -> Result<qdrant_client::Qdrant, String> {
        let mut builder = qdrant_client::Qdrant::from_url(&self.url);
        if let Some(key) = &self.api_key {
            builder = builder.api_key(key.clone());
        }
        builder.build().map_err(|e| format!("Qdrant connection failed: {}", e))
    }

    /// Ensure the collection exists with the right vector size and distance.
    pub async fn ensure_collection(
        &self,
        client: &qdrant_client::Qdrant,
        vector_size: u64,
    ) -> Result<(), String> {
        use qdrant_client::qdrant::{CreateCollectionBuilder, Distance, VectorParamsBuilder};

        let collections = client
            .list_collections()
            .await
            .map_err(|e| format!("Failed to list Qdrant collections: {}", e))?;

        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == self.collection);

        if !exists {
            client
                .create_collection(
                    CreateCollectionBuilder::new(self.collection.clone())
                        .vectors_config(VectorParamsBuilder::new(vector_size, Distance::Cosine)),
                )
                .await
                .map_err(|e| format!("Failed to create Qdrant collection: {}", e))?;

            tracing::info!(
                "Created Qdrant collection '{}' (size={}, distance=Cosine)",
                self.collection,
                vector_size
            );
        }

        Ok(())
    }
}

// ─── Episodic Memory ──────────────────────────────────────────

/// Episodic memory with in-memory vector store and optional Qdrant sync.
pub struct EpisodicMemory {
    /// In-memory vector storage.
    chunks: Vec<MemoryChunk>,
    /// Maximum chunks to keep in memory.
    max_chunks: usize,
    /// Chunks ordered by recency.
    recency: VecDeque<String>,
    /// Optional embedding service — when set, auto-embeds chunks on store().
    embedding_service: Option<Arc<EmbeddingService>>,
    /// Optional Qdrant backend for persistent sync.
    qdrant: Option<QdrantBackend>,
}

impl EpisodicMemory {
    /// Create a new episodic memory store.
    pub fn new(max_chunks: usize) -> Self {
        Self {
            chunks: Vec::with_capacity(max_chunks.min(100)),
            max_chunks,
            recency: VecDeque::with_capacity(max_chunks),
            embedding_service: None,
            qdrant: None,
        }
    }

    /// Create with default capacity (1000 chunks).
    pub fn default_store() -> Self {
        Self::new(1000)
    }

    /// Attach an embedding service for auto-embedding on store().
    pub fn with_embedding_service(mut self, service: Arc<EmbeddingService>) -> Self {
        self.embedding_service = Some(service);
        self
    }

    /// Attach an optional Qdrant backend for persistent sync.
    pub fn with_qdrant(mut self, backend: QdrantBackend) -> Self {
        self.qdrant = Some(backend);
        self
    }

    /// Store a memory chunk, auto-embedding if missing and possible.
    pub async fn store(&mut self, mut chunk: MemoryChunk) {
        // Auto-embed if embedding is empty and we have an embedding service
        if chunk.embedding.is_empty() {
            if let Some(service) = &self.embedding_service {
                let vec = service.embed(&chunk.content).await;
                chunk.embedding = vec;
            }
        }

        let chunk_id = chunk.id.clone();

        // Store in-memory
        self.chunks.push(chunk);
        self.recency.push_back(chunk_id.clone());

        // Evict oldest if over capacity
        while self.chunks.len() > self.max_chunks {
            if let Some(oldest_id) = self.recency.pop_front() {
                self.chunks.retain(|c| c.id != oldest_id);
            }
        }

        // Sync to Qdrant if configured (fire-and-forget)
        if let Some(qdrant) = &self.qdrant {
            if let Some(last) = self.chunks.last() {
                let qdrant = qdrant.clone();
                let chunk = last.clone();
                tokio::spawn(async move {
                    if let Err(e) = Self::sync_to_qdrant(&qdrant, &chunk).await {
                        tracing::warn!("Qdrant sync failed: {}", e);
                    }
                });
            }
        }
    }

    /// Sync a single chunk to Qdrant.
    async fn sync_to_qdrant(qdrant: &QdrantBackend, chunk: &MemoryChunk) -> Result<(), String> {
        use qdrant_client::qdrant::{PointStruct, UpsertPointsBuilder};

        let client = qdrant.build_client().await?;

        let payload: std::collections::HashMap<String, serde_json::Value> = [
            ("session_id".to_string(), serde_json::Value::String(chunk.metadata.session_id.clone())),
            ("project_id".to_string(), serde_json::Value::String(chunk.metadata.project_id.clone())),
            ("agent_id".to_string(), serde_json::Value::String(chunk.metadata.agent_id.clone())),
            ("chunk_type".to_string(), serde_json::Value::String(format!("{:?}", chunk.metadata.chunk_type))),
            ("timestamp".to_string(), serde_json::Value::String(chunk.metadata.timestamp.clone())),
            ("token_count".to_string(), serde_json::json!(chunk.metadata.token_count)),
            ("content".to_string(), serde_json::Value::String(chunk.content.clone())),
        ]
        .into_iter()
        .collect();

        let point = PointStruct::new(chunk.id.clone(), chunk.embedding.clone(), payload);

        client
            .upsert_points(UpsertPointsBuilder::new(qdrant.collection.clone(), vec![point]))
            .await
            .map_err(|e| format!("Qdrant upsert failed: {}", e))?;

        Ok(())
    }

    /// Search in-memory for chunks similar to a query embedding.
    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Vec<SearchResult> {
        if self.chunks.is_empty() || query_embedding.is_empty() {
            return Vec::new();
        }

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

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }

    /// Search Qdrant for chunks (requires configured backend).
    pub async fn search_qdrant(
        &self,
        query_embedding: &[f32],
        limit: usize,
        session_filter: Option<&str>,
        project_filter: Option<&str>,
    ) -> Result<Vec<SearchResult>, String> {
        let qdrant = self.qdrant.as_ref().ok_or_else(|| "No Qdrant backend configured".to_string())?;
        let client = qdrant.build_client().await?;

        use qdrant_client::qdrant::{Condition, Filter, QueryPointsBuilder};

        let mut builder = QueryPointsBuilder::new(qdrant.collection.clone())
            .query(query_embedding.to_vec())
            .limit(limit as u64)
            .with_payload(true);

        // Build optional filter
        let mut conditions = Vec::new();
        if let Some(session) = session_filter {
            conditions.push(Condition::matches("session_id", session.to_string()));
        }
        if let Some(project) = project_filter {
            conditions.push(Condition::matches("project_id", project.to_string()));
        }
        if !conditions.is_empty() {
            builder = builder.filter(Filter::must(conditions));
        }

        let response = client
            .query(builder)
            .await
            .map_err(|e| format!("Qdrant search failed: {}", e))?;

        let results = response
            .result
            .into_iter()
            .filter_map(|scored| {
                // Serialize payload to JSON for robust field access
                let payload_json: serde_json::Value = serde_json::to_value(&scored.payload).ok()?;

                // Extract score
                let score = scored.score;

                // Extract vector data (not always serializable, default to empty)
                let embedding: Vec<f32> = Vec::new();

                let extract_str = |key: &str| -> String {
                    payload_json
                        .get(key)
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default()
                };

                let chunk = MemoryChunk {
                    id: scored.id.map(|id| format!("{:?}", id)).unwrap_or_default(),
                    content: extract_str("content"),
                    embedding,
                    metadata: ChunkMetadata {
                        session_id: extract_str("session_id"),
                        project_id: extract_str("project_id"),
                        agent_id: extract_str("agent_id"),
                        chunk_type: ChunkType::Conversation,
                        timestamp: extract_str("timestamp"),
                        token_count: payload_json
                            .get("token_count")
                            .and_then(|v| v.as_u64().map(|n| n as u32))
                            .unwrap_or(0),
                    },
                    score: Some(score),
                };
                Some(SearchResult { chunk, score })
            })
            .collect();

        Ok(results)
    }

    /// Search with text query (keyword matching on in-memory store).
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
            has_qdrant: self.qdrant.is_some(),
            has_embedding_service: self.embedding_service.is_some(),
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

    /// Remove chunks older than `max_age` based on their timestamp.
    ///
    /// Parses chunk timestamps as RFC 3339. Chunks with unparseable timestamps
    /// are kept (not removed). Returns the number of chunks removed.
    pub fn cleanup(&mut self, max_age: std::time::Duration) -> usize {
        let now = chrono::Utc::now();
        let max_secs = max_age.as_secs() as i64;
        let before_removal = self.chunks.len();

        self.chunks.retain(|chunk| {
            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&chunk.metadata.timestamp) {
                let age_secs = (now - ts.to_utc()).num_seconds();
                age_secs < max_secs
            } else {
                true
            }
        });

        let valid_ids: std::collections::HashSet<String> =
            self.chunks.iter().map(|c| c.id.clone()).collect();
        self.recency.retain(|id| valid_ids.contains(id));

        before_removal - self.chunks.len()
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
    pub has_qdrant: bool,
    pub has_embedding_service: bool,
    pub by_type: std::collections::HashMap<String, usize>,
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_search() {
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

        memory.store(chunk).await;

        // Search with similar embedding
        let results = memory.search(&[1.0, 0.0, 0.0], 5);
        assert_eq!(results.len(), 1);
        assert!(results[0].score > 0.9);
    }

    #[tokio::test]
    async fn test_search_text() {
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
        }).await;

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
        }).await;

        let results = memory.search_text("Rust programming", 5);
        assert_eq!(results.len(), 1);
        assert!(results[0].score > 0.5);
        assert!(results[0].chunk.id == "1");
    }

    #[tokio::test]
    async fn test_eviction() {
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
            }).await;
        }

        assert_eq!(memory.count(), 3);
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

    #[tokio::test]
    async fn test_by_session() {
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
            }).await;
        }

        let s1_chunks = memory.by_session("s1");
        assert_eq!(s1_chunks.len(), 3);
    }

    #[tokio::test]
    async fn test_auto_embed() {
        use crate::EmbeddingService;
        use praxis_agent_traits::prelude::LLMProvider;
        use praxis_agent_traits::provider::{ChatConfig, ChatMessage, ChatResponse, ModelCost, StreamReceiver};
        use praxis_shared::types::ModelInfo;
        use async_trait::async_trait;

        struct MockProvider;

        #[async_trait]
        impl LLMProvider for MockProvider {
            async fn chat(&self, _: &[ChatMessage], _: &ChatConfig) -> praxis_agent_traits::Result<ChatResponse> {
                unimplemented!()
            }
            async fn stream(&self, _: &[ChatMessage], _: &ChatConfig) -> praxis_agent_traits::Result<StreamReceiver> {
                unimplemented!()
            }
            async fn embed(&self, input: &[String]) -> praxis_agent_traits::Result<Vec<Vec<f32>>> {
                Ok(input.iter().map(|s| {
                    let mut v = vec![0.0; 4];
                    let bytes = s.as_bytes();
                    for (i, b) in bytes.iter().enumerate() {
                        v[i % 4] += *b as f32 / 255.0;
                    }
                    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                    if norm > 0.0 { for x in &mut v { *x /= norm; } }
                    v
                }).collect())
            }
            fn count_tokens(&self, text: &str) -> usize { text.len() / 4 }
            fn model_info(&self) -> ModelInfo {
                ModelInfo { name: "mock".into(), provider: "mock".into(), context_window: 128000, hard_limit_pct: 0.7, max_output_tokens: 4096, supports_streaming: true, supports_embeddings: true }
            }
            fn model_cost(&self) -> ModelCost {
                ModelCost { per_input_token: 0.0, per_output_token: 0.0, currency: "USD".into() }
            }
            fn provider_name(&self) -> &str { "mock" }
        }

        let embedding_service = Arc::new(EmbeddingService::new(Arc::new(MockProvider), 10, 100));
        let mut memory = EpisodicMemory::new(10)
            .with_embedding_service(embedding_service);

        let chunk = MemoryChunk {
            id: "auto-1".to_string(),
            content: "Auto-embedded chunk test".to_string(),
            embedding: vec![],  // Empty — should be auto-filled
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

        memory.store(chunk).await;
        let stored = &memory.chunks[0];
        assert!(!stored.embedding.is_empty(), "Embedding should be auto-generated");
        assert_eq!(stored.embedding.len(), 4);
    }

    #[test]
    fn test_qdrant_backend_config() {
        let backend = QdrantBackend::new("http://localhost:6334")
            .with_collection("my_episodic")
            .with_api_key("secret");
        assert_eq!(backend.url, "http://localhost:6334");
        assert_eq!(backend.collection, "my_episodic");
        assert_eq!(backend.api_key, Some("secret".to_string()));
    }

    #[test]
    fn test_stats() {
        let mut memory = EpisodicMemory::new(10);
        // No embedding service and no qdrant
        let stats = memory.stats();
        assert!(!stats.has_qdrant);
        assert!(!stats.has_embedding_service);
        assert_eq!(stats.total_chunks, 0);
    }

    #[test]
    fn test_cleanup_removes_old_chunks() {
        let mut memory = EpisodicMemory::new(100);

        // Chunk from 30 days ago (should be removed)
        let old_time = (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        let old_chunk = MemoryChunk {
            id: "old".to_string(),
            content: "Old content".to_string(),
            embedding: vec![],
            metadata: ChunkMetadata {
                session_id: "s1".to_string(),
                project_id: "p1".to_string(),
                agent_id: "a1".to_string(),
                chunk_type: ChunkType::Conversation,
                timestamp: old_time,
                token_count: 5,
            },
            score: None,
        };

        // Chunk from 1 hour ago (should be kept)
        let recent_time = chrono::Utc::now().to_rfc3339();
        let recent_chunk = MemoryChunk {
            id: "recent".to_string(),
            content: "Recent content".to_string(),
            embedding: vec![],
            metadata: ChunkMetadata {
                session_id: "s1".to_string(),
                project_id: "p1".to_string(),
                agent_id: "a1".to_string(),
                chunk_type: ChunkType::Conversation,
                timestamp: recent_time,
                token_count: 5,
            },
            score: None,
        };

        memory.chunks.push(old_chunk);
        memory.chunks.push(recent_chunk);
        memory.recency.push_back("old".to_string());
        memory.recency.push_back("recent".to_string());

        // Cleanup chunks older than 7 days
        let removed = memory.cleanup(std::time::Duration::from_secs(7 * 24 * 3600));
        assert_eq!(removed, 1, "Should remove 1 old chunk");
        assert_eq!(memory.chunks.len(), 1, "Should keep 1 recent chunk");
        assert_eq!(memory.chunks[0].id, "recent", "Recent chunk must survive");
        assert_eq!(memory.recency.len(), 1, "Recency list cleaned too");
        assert_eq!(memory.recency[0], "recent", "Recent chunk in recency");
    }
}
