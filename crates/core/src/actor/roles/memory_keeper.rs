//! MemoryKeeper — background task that indexes agent outputs into episodic memory.
//!
//! Listens to the EventBus for `AgentOutput` (streaming deltas), `AgentCompleted`
//! (full text), and `ToolCalled` events. Buffers streaming deltas per-agent,
//! then chunks + embeds (batched) + stores via `EpisodicMemory` for future RAG retrieval.
//!
//! # Lifecycle
//! 1. Created via `MemoryKeeper::new(bus, memory)` with optional embedding service.
//! 2. `start()` spawns a background tokio task.
//! 3. `set_session()` / `set_embedding_service()` called when a new goal starts.
//! 4. Shuts down cleanly via `Notify` when the event bus closes or shutdown is requested.

use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{Notify, RwLock, broadcast};

use praxis_memory::chunk_text_auto;
use praxis_memory::embedding::EmbeddingService;
use praxis_memory::episodic::{
    ChunkMetadata, ChunkType, EpisodicMemory, MemoryChunk, SearchResult,
};
use praxis_shared::protocol::{MessageKind, SystemEvent};

use crate::bus::EventBus;

/// Background memory keeper that indexes agent outputs into episodic memory.
pub struct MemoryKeeper {
    /// Event bus subscription for listening to agent events.
    bus: EventBus,
    /// Shared episodic memory store.
    episodic_memory: Arc<RwLock<EpisodicMemory>>,
    /// Optional embedding service for semantic vector generation.
    embedding_service: Arc<RwLock<Option<Arc<EmbeddingService>>>>,
    /// Per-agent buffers for streaming delta accumulation: agent_name → accumulated text.
    delta_buffer: Arc<DashMap<String, String>>,
    /// Current session ID for metadata tagging (set when a goal starts).
    session_id: Arc<RwLock<Option<String>>>,
    /// Project identifier for metadata tagging.
    project_id: String,
    /// Shutdown notifier — set() triggers clean exit from the event loop.
    shutdown: Arc<Notify>,
}

impl MemoryKeeper {
    /// Create a new MemoryKeeper.
    pub fn new(bus: EventBus, episodic_memory: Arc<RwLock<EpisodicMemory>>) -> Self {
        Self {
            bus,
            episodic_memory,
            embedding_service: Arc::new(RwLock::new(None)),
            delta_buffer: Arc::new(DashMap::new()),
            session_id: Arc::new(RwLock::new(None)),
            project_id: String::from("default"),
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Attach an embedding service for auto-embedding on store.
    pub fn with_embedding_service(mut self, service: Arc<EmbeddingService>) -> Self {
        self.embedding_service = Arc::new(RwLock::new(Some(service)));
        self
    }

    /// Late-binding: set the embedding service after creation (called from CoreRuntime
    /// once the LLM provider is initialized in `run_goal()`).
    pub async fn set_embedding_service(&self, service: Arc<EmbeddingService>) {
        *self.embedding_service.write().await = Some(service);
    }

    /// Set the project identifier for metadata tagging.
    pub fn with_project(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = project_id.into();
        self
    }

    /// Update the current session ID (called when a new goal starts).
    pub async fn set_session(&self, session_id: Option<String>) {
        *self.session_id.write().await = session_id;
    }

    /// Get a reference to the episodic memory for RAG queries.
    pub fn episodic_memory(&self) -> &Arc<RwLock<EpisodicMemory>> {
        &self.episodic_memory
    }

    /// Search episodic memory for chunks relevant to `query`.
    ///
    /// Uses embedding-based `search_with_filter()` when an embedding service is configured,
    /// falls back to keyword `search_text_with_filter()` otherwise.
    ///
    /// When `project_id` is `Some`, only chunks from that project are returned.
    /// When `None`, all chunks are searched (cross-project).
    pub async fn search_rag(
        &self,
        query: &str,
        limit: usize,
        project_id: Option<&str>,
    ) -> Vec<SearchResult> {
        let memory = self.episodic_memory.read().await;

        if let Some(ref es) = *self.embedding_service.read().await {
            let query_embedding = es.embed(query).await;
            if !query_embedding.is_empty() {
                return memory.search_with_filter(&query_embedding, limit, project_id);
            }
        }

        memory.search_text_with_filter(query, limit, project_id)
    }

    /// Get a handle to the shutdown notifier.
    pub fn shutdown_handle(&self) -> Arc<Notify> {
        self.shutdown.clone()
    }

    /// Start the background event listener.
    ///
    /// Spawns a tokio task that subscribes to the EventBus and processes
    /// events until the bus is closed or shutdown is requested.
    ///
    /// Takes `Arc<Self>` so the caller retains a handle for session updates.
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    /// Main event loop — uses Notify for clean shutdown, no busy-polling.
    async fn run(self: Arc<Self>) {
        let mut rx = self.bus.subscribe();

        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(event) => self.handle_event(event).await,
                        Err(broadcast::error::RecvError::Lagged(count)) => {
                            tracing::warn!("MemoryKeeper lagged behind by {} events", count);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            tracing::info!("MemoryKeeper: event bus closed, shutting down");
                            break;
                        }
                    }
                }
                () = self.shutdown.notified() => {
                    tracing::info!("MemoryKeeper: shutdown requested");
                    break;
                }
            }
        }
    }

    /// Handle a single system event.
    async fn handle_event(&self, event: SystemEvent) {
        match &event.kind {
            MessageKind::AgentOutput { agent, delta } => {
                // Buffer streaming deltas per agent
                self.delta_buffer
                    .entry(agent.clone())
                    .and_modify(|existing| existing.push_str(delta))
                    .or_insert_with(|| delta.clone());
            }
            MessageKind::AgentCompleted {
                agent,
                output_preview,
                ..
            } => {
                // Retrieve full buffered text, fall back to preview
                let full_text = self
                    .delta_buffer
                    .remove(agent)
                    .map(|(_, text)| text)
                    .unwrap_or_else(|| output_preview.clone());

                self.store_agent_output(agent, &full_text).await;
            }
            MessageKind::ToolCalled {
                agent,
                tool,
                duration_ms,
                success,
            } => {
                self.store_tool_call(agent, tool, *duration_ms, *success)
                    .await;
            }
            _ => { /* ignore other event types */ }
        }
    }

    /// Chunk (batch-embed) and store an agent's full output.
    async fn store_agent_output(&self, agent: &str, content: &str) {
        if content.trim().is_empty() {
            return;
        }

        let session = self.session_id.read().await.clone();
        let chunks = chunk_text_auto(content);

        if chunks.is_empty() {
            return;
        }

        // Batch-embed all chunks at once when embedding service is available
        let embeddings: Vec<Vec<f32>> = if let Some(ref es) = *self.embedding_service.read().await {
            let chunk_refs: Vec<String> = chunks.to_vec();
            if !chunk_refs.is_empty() {
                es.embed_batch(&chunk_refs).await
            } else {
                vec![Vec::new(); chunks.len()]
            }
        } else {
            vec![Vec::new(); chunks.len()]
        };

        let timestamp = chrono::Utc::now().to_rfc3339();

        for (i, chunk_text) in chunks.into_iter().enumerate() {
            let embedding = embeddings.get(i).cloned().unwrap_or_default();

            let chunk = MemoryChunk {
                id: uuid::Uuid::new_v4().to_string(),
                content: chunk_text,
                embedding,
                metadata: ChunkMetadata {
                    session_id: session.clone().unwrap_or_default(),
                    project_id: self.project_id.clone(),
                    agent_id: agent.to_string(),
                    chunk_type: ChunkType::Conversation,
                    timestamp: timestamp.clone(),
                    token_count: 0,
                },
                score: None,
            };

            // Lock and store — MemoryKeeper is the sole writer, contention is rare
            let mut memory = self.episodic_memory.write().await;
            memory.store(chunk).await;
        }
    }

    /// Store a tool call record as a memory chunk.
    async fn store_tool_call(&self, agent: &str, tool: &str, duration_ms: u64, success: bool) {
        let session = self.session_id.read().await.clone();
        let status = if success { "success" } else { "failed" };
        let content = format!(
            "Tool call: {} by agent {} ({}ms, {})",
            tool, agent, duration_ms, status
        );

        let chunk = MemoryChunk {
            id: uuid::Uuid::new_v4().to_string(),
            content,
            embedding: Vec::new(),
            metadata: ChunkMetadata {
                session_id: session.clone().unwrap_or_default(),
                project_id: self.project_id.clone(),
                agent_id: agent.to_string(),
                chunk_type: ChunkType::Conversation,
                timestamp: chrono::Utc::now().to_rfc3339(),
                token_count: 0,
            },
            score: None,
        };

        let mut memory = self.episodic_memory.write().await;
        memory.store(chunk).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::EventBus;

    #[tokio::test]
    async fn test_memory_keeper_creation() {
        let bus = EventBus::new();
        let memory = Arc::new(RwLock::new(EpisodicMemory::new(100)));
        let keeper = MemoryKeeper::new(bus, memory);

        assert!(keeper.delta_buffer.is_empty());
    }

    #[tokio::test]
    async fn test_buffer_output_deltas() {
        // Verify that publishing AgentOutput events doesn't crash
        let bus = EventBus::new();
        let memory = Arc::new(RwLock::new(EpisodicMemory::new(100)));
        let _keeper = MemoryKeeper::new(bus.clone(), memory);

        // Publishing should not panic
        bus.publish(
            MessageKind::AgentOutput {
                agent: "coder".to_string(),
                delta: "Hello ".to_string(),
            },
            "test",
        );
        bus.publish(
            MessageKind::AgentOutput {
                agent: "coder".to_string(),
                delta: "world!".to_string(),
            },
            "test",
        );
    }

    #[tokio::test]
    async fn test_store_on_completion() {
        let bus = EventBus::new();
        let memory = Arc::new(RwLock::new(EpisodicMemory::new(100)));
        let _keeper = MemoryKeeper::new(bus.clone(), memory);

        bus.publish(
            MessageKind::AgentOutput {
                agent: "coder".to_string(),
                delta: "Full agent output text".to_string(),
            },
            "test",
        );
        bus.publish(
            MessageKind::AgentCompleted {
                agent: "coder".to_string(),
                role: "coder".to_string(),
                status: "completed".to_string(),
                duration_ms: 1000,
                output_preview: "preview".to_string(),
            },
            "test",
        );
    }

    #[tokio::test]
    async fn test_set_session() {
        let bus = EventBus::new();
        let memory = Arc::new(RwLock::new(EpisodicMemory::new(100)));
        let keeper = MemoryKeeper::new(bus, memory);

        keeper.set_session(Some("session-123".to_string())).await;
        assert_eq!(
            *keeper.session_id.read().await,
            Some("session-123".to_string())
        );

        keeper.set_session(None).await;
        assert_eq!(*keeper.session_id.read().await, None);
    }

    #[tokio::test]
    async fn test_search_rag_fallback_without_embedding() {
        let bus = EventBus::new();
        let memory = Arc::new(RwLock::new(EpisodicMemory::new(100)));
        let keeper = MemoryKeeper::new(bus, memory);

        // Without embedding service, search_rag falls back to keyword search_text()
        let results = keeper.search_rag("test query", 5, None).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_set_embedding_service_late_binding() {
        let bus = EventBus::new();
        let memory = Arc::new(RwLock::new(EpisodicMemory::new(100)));
        let keeper = MemoryKeeper::new(bus, memory);

        // Initially no embedding service
        assert!(keeper.embedding_service.read().await.is_none());

        // Create a mock embedding service (zero vectors from empty provider)
        let provider = praxis_providers::MockProvider::simple("test");
        let es = Arc::new(EmbeddingService::new_default(
            Arc::new(provider) as Arc<dyn praxis_agent_traits::provider::LLMProvider>
        ));
        keeper.set_embedding_service(es).await;

        // Now it should be set
        assert!(keeper.embedding_service.read().await.is_some());
    }
}
