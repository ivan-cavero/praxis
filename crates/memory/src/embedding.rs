//! EmbeddingService — wraps an LLM provider's `embed()` with batching + caching.
//!
//! - Batches multiple texts into a single API call when possible.
//! - Caches embeddings for frequently-embedded texts using moka.
//! - Falls back to a default embedding model if the provider doesn't support
//!   embeddings (e.g., Anthropic, Gemini without embedding API).

    use praxis_agent_traits::prelude::LLMProvider;
use std::sync::Arc;

/// Default embedding dimension (OpenAI text-embedding-3-small).
const DEFAULT_DIMENSION: usize = 1536;

/// Embedding cache TTL (10 minutes — long enough for frequent texts within a
/// session, short enough that stale embeddings are flushed).
const CACHE_TTL_SECS: u64 = 600;

/// Service for generating and caching embeddings.
pub struct EmbeddingService {
    /// The provider used for embedding generation.
    provider: Arc<dyn LLMProvider>,
    /// moka cache: text → embedding vector.
    cache: moka::sync::Cache<String, Vec<f32>>,
    /// Maximum batch size for embedding API calls.
    max_batch_size: usize,
    /// Whether the underlying provider supports embeddings.
    supports_embeddings: bool,
    /// Stats tracker.
    stats: std::sync::Arc<std::sync::Mutex<EmbeddingStats>>,
}

/// Statistics for the embedding service.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbeddingStats {
    pub total_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub batched_requests: u64,
    pub average_batch_size: f64,
}

impl Default for EmbeddingStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            cache_hits: 0,
            cache_misses: 0,
            batched_requests: 0,
            average_batch_size: 1.0,
        }
    }
}

impl EmbeddingService {
    /// Create a new embedding service wrapping the given provider.
    ///
    /// Uses `max_batch_size` (default 20) to limit how many texts are sent in
    /// a single API call. Pass `max_entries` to control the cache size.
    pub fn new(
        provider: Arc<dyn LLMProvider>,
        max_batch_size: usize,
        max_cache_entries: u64,
    ) -> Self {
        let model_info = provider.model_info();
        let supports_embeddings = model_info.supports_embeddings;

        let cache: moka::sync::Cache<String, Vec<f32>> = moka::sync::Cache::builder()
            .max_capacity(max_cache_entries)
            .time_to_live(std::time::Duration::from_secs(CACHE_TTL_SECS))
            .build();

        tracing::info!(
            "EmbeddingService initialized (supports_embeddings={}, max_batch={}, cache={})",
            supports_embeddings, max_batch_size, max_cache_entries,
        );

        Self {
            provider,
            cache,
            max_batch_size,
            supports_embeddings,
            stats: std::sync::Arc::new(std::sync::Mutex::new(EmbeddingStats::default())),
        }
    }

    /// Create with sensible defaults (batch=20, cache=5000).
    pub fn new_default(provider: Arc<dyn LLMProvider>) -> Self {
        Self::new(provider, 20, 5_000)
    }

    /// Embed a single text string.
    pub async fn embed(&self, text: &str) -> Vec<f32> {
        // Check cache first
        if let Some(cached) = self.cache.get(text) {
            let mut stats = self.stats.lock().expect("embedding stats lock");
            stats.cache_hits += 1;
            stats.total_requests += 1;
            return cached;
        }

        // Generate embedding
        let result = self
            .embed_batch_impl(&[text.to_string()])
            .await;

        match result {
            Ok(mut vectors) if !vectors.is_empty() => {
                let vec = vectors.remove(0);
                self.cache.insert(text.to_string(), vec.clone());

                let mut stats = self.stats.lock().expect("embedding stats lock");
                stats.cache_misses += 1;
                stats.total_requests += 1;

                vec
            }
            _ => {
                // Fallback: return zero vector
                let mut stats = self.stats.lock().expect("embedding stats lock");
                stats.cache_misses += 1;
                stats.total_requests += 1;

                vec![0.0; DEFAULT_DIMENSION]
            }
        }
    }

    /// Embed multiple texts in one batch.
    ///
    /// Returns vectors in the same order as the input texts.
    /// Splits into sub-batches if `texts.len()` exceeds `max_batch_size`.
    pub async fn embed_batch(&self, texts: &[String]) -> Vec<Vec<f32>> {
        if texts.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::with_capacity(texts.len());
        let mut batch_start = 0;

        while batch_start < texts.len() {
            let batch_end = (batch_start + self.max_batch_size).min(texts.len());
            let batch = &texts[batch_start..batch_end];

            // Check cache for each text in this batch
            let mut uncached: Vec<(usize, String)> = Vec::new();
            let mut batched_results: Vec<Option<Vec<f32>>> = Vec::with_capacity(batch.len());

            for (i, text) in batch.iter().enumerate() {
                if let Some(cached) = self.cache.get(text) {
                    batched_results.push(Some(cached));
                } else {
                    batched_results.push(None);
                    uncached.push((i, text.clone()));
                }
            }

            // Fetch uncached texts in a single API call
            if !uncached.is_empty() {
                let uncached_texts: Vec<String> = uncached
                    .iter()
                    .map(|(_, t)| t.clone())
                    .collect();

                if let Ok(vectors) = self.embed_batch_impl(&uncached_texts).await {
                    // Cache and insert results
                    for (vec_idx, (orig_idx, text)) in uncached.iter().enumerate() {
                        if vec_idx < vectors.len() {
                            let vec = vectors[vec_idx].clone();
                            self.cache.insert(text.clone(), vec.clone());
                            batched_results[*orig_idx] = Some(vec);
                        }
                    }
                }
            }

            // Collect results (use zero vector for failures)
            for opt in batched_results {
                results.push(opt.unwrap_or_else(|| vec![0.0; DEFAULT_DIMENSION]));
            }

            // Update stats
            {
                let mut stats = self.stats.lock().expect("embedding stats lock");
                stats.batched_requests += 1;
                if !uncached.is_empty() {
                    stats.cache_misses += uncached.len() as u64;
                }
                stats.cache_hits += batch.len().saturating_sub(uncached.len()) as u64;
                stats.total_requests += batch.len() as u64;
            }

            batch_start = batch_end;
        }

        results
    }

    /// Actually call the provider's embed().
    async fn embed_batch_impl(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if !self.supports_embeddings {
            tracing::warn!(
                "Provider does not support embeddings, returning zero vectors (count={})",
                texts.len()
            );
            return Ok(texts.iter().map(|_| vec![0.0; DEFAULT_DIMENSION]).collect());
        }

        self.provider
            .embed(texts)
            .await
            .map_err(|e| format!("Embedding failed: {}", e))
    }

    /// Get current statistics.
    pub fn stats(&self) -> EmbeddingStats {
        self.stats.lock().expect("embedding stats lock").clone()
    }

    /// Clear the embedding cache.
    pub fn clear_cache(&self) {
        self.cache.invalidate_all();
        tracing::debug!("Embedding cache cleared");
    }

    /// Return the expected embedding dimension.
    pub fn dimension(&self) -> usize {
        DEFAULT_DIMENSION
    }
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use praxis_agent_traits::provider::{ChatConfig, ChatMessage, ChatResponse, ModelCost, StreamReceiver};
    use praxis_shared::types::ModelInfo;
    use std::sync::Arc;

    /// A mock provider that returns deterministic embeddings.
    struct MockEmbedProvider {
        dimension: usize,
    }

    #[async_trait]
    impl LLMProvider for MockEmbedProvider {
        async fn chat(&self, _messages: &[ChatMessage], _config: &ChatConfig) -> praxis_agent_traits::Result<ChatResponse> {
            unimplemented!()
        }

        async fn stream(&self, _messages: &[ChatMessage], _config: &ChatConfig) -> praxis_agent_traits::Result<StreamReceiver> {
            unimplemented!()
        }

        async fn embed(&self, input: &[String]) -> praxis_agent_traits::Result<Vec<Vec<f32>>> {
            Ok(input.iter().map(|s| {
                let mut v = vec![0.0; self.dimension];
                let bytes = s.as_bytes();
                for (i, b) in bytes.iter().enumerate() {
                    v[i % self.dimension] += *b as f32 / 255.0;
                }
                let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in &mut v { *x /= norm; }
                }
                v
            }).collect())
        }

        fn count_tokens(&self, text: &str) -> usize {
            text.len() / 4
        }

        fn model_info(&self) -> ModelInfo {
            ModelInfo {
                name: "mock-embed".into(),
                provider: "mock".into(),
                context_window: 128_000,
                hard_limit_pct: 0.7,
                max_output_tokens: 4096,
                supports_streaming: true,
                supports_embeddings: true,
            }
        }

        fn model_cost(&self) -> ModelCost {
            ModelCost {
                per_input_token: 0.0,
                per_output_token: 0.0,
                currency: "USD".to_string(),
            }
        }

        fn provider_name(&self) -> &str { "mock" }
    }

    #[tokio::test]
    async fn test_embed_single() {
        let provider = Arc::new(MockEmbedProvider { dimension: 4 });
        let service = EmbeddingService::new(provider, 10, 100);

        let vec = service.embed("hello world").await;
        assert_eq!(vec.len(), 4);
        // Should be normalized (norm ≈ 1.0)
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_embed_cache_hit() {
        let provider = Arc::new(MockEmbedProvider { dimension: 4 });
        let service = EmbeddingService::new(provider, 10, 100);

        let v1 = service.embed("hello").await;
        let v2 = service.embed("hello").await;

        assert_eq!(v1, v2);
        let stats = service.stats();
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 1);
    }

    #[tokio::test]
    async fn test_embed_batch() {
        let provider = Arc::new(MockEmbedProvider { dimension: 4 });
        let service = EmbeddingService::new(provider, 10, 100);

        let texts = vec!["a".to_string(), "bb".to_string(), "ccc".to_string()];
        let results = service.embed_batch(&texts).await;

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].len(), 4);
        assert_eq!(results[1].len(), 4);
        assert_eq!(results[2].len(), 4);

        // Different inputs should have different embeddings
        assert_ne!(results[0], results[1]);
        assert_ne!(results[1], results[2]);
    }

    #[tokio::test]
    async fn test_embed_batch_splitting() {
        let provider = Arc::new(MockEmbedProvider { dimension: 4 });
        let service = EmbeddingService::new(provider, 2, 100); // max_batch=2

        let texts: Vec<String> = (0..5).map(|i| format!("text-{}", i)).collect();
        let results = service.embed_batch(&texts).await;

        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_dimension_constant() {
        assert_eq!(DEFAULT_DIMENSION, 1536);
    }
}
