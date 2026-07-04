//! SummarizerAgent — generates consolidated summaries from episodic memory.
//!
//! Converts agent interactions (stored as chunks in EpisodicMemory) into
//! structured `MemorySummary` objects via the `Summarizer`, then stores
//! them in `ConsolidatedMemory` for cross-session knowledge retention.
//!
//! # Lifecycle
//! 1. Created via `SummarizerAgent::new(episodic, consolidated)`.
//! 2. `summarize_session()` is called after each phase completes in `run_goal()`.
//! 3. No background task — synchronous (async) calls from the main loop.

use std::sync::Arc;

use tokio::sync::RwLock;

use praxis_memory::consolidated::{
    ConsolidatedMemory, InteractionKind, InteractionSummary, MemorySummary, Summarizer,
};
use praxis_memory::episodic::EpisodicMemory;

/// Generates consolidated summaries from episodic memory.
pub struct SummarizerAgent {
    /// Source: episodic chunks indexed by the MemoryKeeper.
    episodic_memory: Arc<RwLock<EpisodicMemory>>,
    /// Target: structured summaries for long-term retention.
    consolidated_memory: Arc<RwLock<ConsolidatedMemory>>,
    /// The stateless summarizer engine.
    summarizer: Summarizer,
}

impl SummarizerAgent {
    /// Create a new SummarizerAgent.
    pub fn new(
        episodic_memory: Arc<RwLock<EpisodicMemory>>,
        consolidated_memory: Arc<RwLock<ConsolidatedMemory>>,
    ) -> Self {
        Self {
            episodic_memory,
            consolidated_memory,
            summarizer: Summarizer::new(),
        }
    }

    /// Summarize all chunks for a session and store the result.
    ///
    /// Reads recent chunks from episodic memory, converts them to
    /// `InteractionSummary` objects, runs the summarizer, and stores
    /// the resulting `MemorySummary` in consolidated memory.
    ///
    /// Returns the generated summary.
    pub async fn summarize_session(
        &self,
        session_id: &str,
        project_id: &str,
    ) -> MemorySummary {
        // 1. Read chunks from episodic memory
        let chunks = {
            let memory = self.episodic_memory.read().await;
            memory
                .by_session(session_id)
                .into_iter()
                .map(|c| c.clone())
                .collect::<Vec<_>>()
        };

        if chunks.is_empty() {
            tracing::debug!("SummarizerAgent: no chunks to summarize for session {}", session_id);
            let now = chrono::Utc::now().to_rfc3339();
            let summary = MemorySummary {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session_id.to_string(),
                project_id: project_id.to_string(),
                summary: "No interactions recorded for this session.".to_string(),
                key_decisions: vec![],
                errors_learned: vec![],
                current_state: String::new(),
                action_items: vec![],
                interaction_count: 0,
                period_start: now.clone(),
                period_end: now,
                compressed_at: chrono::Utc::now().to_rfc3339(),
            };
            let mut consolidated = self.consolidated_memory.write().await;
            consolidated.store(summary.clone()).await;
            return summary;
        }

        // 2. Convert chunks into InteractionSummary objects
        let interactions: Vec<InteractionSummary> = chunks
            .iter()
            .map(|chunk| {
                let kind = detect_interaction_kind(&chunk.content);
                InteractionSummary {
                    content: chunk.content.clone(),
                    context: String::new(),
                    kind,
                    agent_id: chunk.metadata.agent_id.clone(),
                    timestamp: chunk.metadata.timestamp.clone(),
                }
            })
            .collect();

        // 3. Run the summarizer
        let summary = self
            .summarizer
            .summarize(session_id, project_id, &interactions);

        // 4. Store in consolidated memory
        let mut consolidated = self.consolidated_memory.write().await;
        consolidated.store(summary.clone()).await;

        tracing::info!(
            "SummarizerAgent: stored summary for session {} ({} interactions, {} decisions, {} errors)",
            session_id,
            summary.interaction_count,
            summary.key_decisions.len(),
            summary.errors_learned.len(),
        );

        summary
    }

    /// Access the consolidated memory for querying.
    pub fn consolidated_memory(&self) -> &Arc<RwLock<ConsolidatedMemory>> {
        &self.consolidated_memory
    }
}

/// Heuristic: detect what kind of interaction a chunk represents.
fn detect_interaction_kind(content: &str) -> InteractionKind {
    let lower = content.to_lowercase();
    if lower.starts_with("tool call:")
        || lower.contains("error:")
        || lower.contains("failed:")
        || lower.contains("panic")
        || lower.contains("exception")
    {
        InteractionKind::Error
    } else if lower.contains("decided")
        || lower.contains("decision")
        || lower.contains("chose")
        || lower.contains("selected")
        || lower.contains("approach")
        || lower.contains("architecture")
    {
        InteractionKind::Decision
    } else if lower.contains("fn ")
        || lower.contains("impl ")
        || lower.contains("struct ")
        || lower.contains("```")
        || lower.contains("function")
        || lower.contains("code")
    {
        InteractionKind::Code
    } else if lower.contains("research")
        || lower.contains("search")
        || lower.contains("lookup")
        || lower.contains("investigate")
        || lower.contains("documentation")
    {
        InteractionKind::Research
    } else {
        InteractionKind::Task
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use praxis_memory::episodic::{ChunkMetadata, ChunkType, MemoryChunk};

    fn make_chunk(content: &str, agent: &str, session: &str) -> MemoryChunk {
        MemoryChunk {
            id: uuid::Uuid::new_v4().to_string(),
            content: content.to_string(),
            embedding: vec![],
            metadata: ChunkMetadata {
                session_id: session.to_string(),
                project_id: "test".to_string(),
                agent_id: agent.to_string(),
                chunk_type: ChunkType::Conversation,
                timestamp: chrono::Utc::now().to_rfc3339(),
                token_count: 0,
            },
            score: None,
        }
    }

    #[tokio::test]
    async fn test_summarize_empty_session() {
        let episodic = Arc::new(RwLock::new(EpisodicMemory::new(100)));
        let consolidated = Arc::new(RwLock::new(ConsolidatedMemory::new(10)));
        let agent = SummarizerAgent::new(episodic, consolidated.clone());

        let summary = agent.summarize_session("empty-session", "p1").await;
        assert_eq!(summary.interaction_count, 0);
        assert!(summary.summary.contains("No interactions"));
    }

    #[tokio::test]
    async fn test_summarize_with_chunks() {
        let episodic = Arc::new(RwLock::new(EpisodicMemory::new(100)));
        let consolidated = Arc::new(RwLock::new(ConsolidatedMemory::new(10)));
        let agent = SummarizerAgent::new(episodic.clone(), consolidated.clone());

        // Store some chunks
        {
            let mut memory = episodic.write().await;
            memory
                .store(make_chunk(
                    "Decided to use PostgreSQL for the database",
                    "architect",
                    "s1",
                ))
                .await;
            memory
                .store(make_chunk(
                    "Implemented the user login endpoint",
                    "coder",
                    "s1",
                ))
                .await;
            memory
                .store(make_chunk(
                    "Error: connection pool exhaustion fixed by increasing max connections",
                    "coder",
                    "s1",
                ))
                .await;
        }

        let summary = agent.summarize_session("s1", "p1").await;
        assert_eq!(summary.interaction_count, 3);
        assert_eq!(summary.key_decisions.len(), 1);
        assert_eq!(summary.errors_learned.len(), 1);
        assert_eq!(summary.session_id, "s1");
        assert_eq!(summary.project_id, "p1");
    }

    #[tokio::test]
    async fn test_summarize_multiple_sessions() {
        let episodic = Arc::new(RwLock::new(EpisodicMemory::new(100)));
        let consolidated = Arc::new(RwLock::new(ConsolidatedMemory::new(10)));
        let agent = SummarizerAgent::new(episodic.clone(), consolidated.clone());

        // Store chunks for two sessions
        {
            let mut memory = episodic.write().await;
            memory
                .store(make_chunk("Session 1 work", "coder", "s1"))
                .await;
            memory
                .store(make_chunk("Session 2 work", "coder", "s2"))
                .await;
        }

        let s1 = agent.summarize_session("s1", "p1").await;
        let s2 = agent.summarize_session("s2", "p1").await;

        assert_eq!(s1.interaction_count, 1);
        assert_eq!(s2.interaction_count, 1);

        // Verify both stored in consolidated
        let cons = consolidated.read().await;
        assert_eq!(cons.count(), 2);
        assert_eq!(cons.by_session("s1").len(), 1);
        assert_eq!(cons.by_session("s2").len(), 1);
    }

    #[test]
    fn test_detect_interaction_kind() {
        assert_eq!(detect_interaction_kind("Tool call: filesystem/read"), InteractionKind::Error);
        assert_eq!(detect_interaction_kind("Decided to use Rust"), InteractionKind::Decision);
        assert_eq!(detect_interaction_kind("fn calculate_tax()"), InteractionKind::Code);
        assert_eq!(detect_interaction_kind("Researching API design patterns"), InteractionKind::Research);
        assert_eq!(detect_interaction_kind("Regular task work"), InteractionKind::Task);
    }
}
