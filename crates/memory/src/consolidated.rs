//! Consolidated memory: compressed summaries and cross-session knowledge.
//!
//! When context grows too large, interactions are compressed into structured
//! summaries that retain key decisions, errors, and learnings.

use std::collections::VecDeque;

/// A consolidated summary of past interactions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemorySummary {
    pub id: String,
    pub session_id: String,
    pub project_id: String,
    pub summary: String,
    pub key_decisions: Vec<KeyDecision>,
    pub errors_learned: Vec<ErrorLearned>,
    pub current_state: String,
    pub action_items: Vec<String>,
    pub interaction_count: u32,
    pub period_start: String,
    pub period_end: String,
    pub compressed_at: String,
}

/// A key decision made during the session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeyDecision {
    pub decision: String,
    pub rationale: String,
    pub agent: String,
}

/// An error that was learned from.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorLearned {
    pub error: String,
    pub fix: String,
    pub agent: String,
}

/// Long-term memory store for consolidated summaries.
///
/// Primary: in-memory `VecDeque<MemorySummary>` for fast access.
/// Optional: Qdrant sync for persistence across restarts.
pub struct ConsolidatedMemory {
    summaries: VecDeque<MemorySummary>,
    max_summaries: usize,
    /// Optional Qdrant backend for persistent backup.
    qdrant: Option<crate::episodic::QdrantBackend>,
}

impl ConsolidatedMemory {
    /// Create a new consolidated memory store.
    pub fn new(max_summaries: usize) -> Self {
        Self {
            summaries: VecDeque::with_capacity(max_summaries),
            max_summaries,
            qdrant: None,
        }
    }

    /// Create with default capacity (100 summaries).
    pub fn default_store() -> Self {
        Self::new(100)
    }

    /// Attach a Qdrant backend for persistent storage.
    ///
    /// Summaries are synced to Qdrant on every `store()` call.
    pub fn with_qdrant(mut self, qdrant: crate::episodic::QdrantBackend) -> Self {
        self.qdrant = Some(qdrant);
        self
    }

    /// Store a summary synchronously (in-memory only).
    ///
    /// Use this for unit tests and non-async contexts.
    /// For production, prefer `store()` which also syncs to Qdrant.
    pub fn store_sync(&mut self, summary: MemorySummary) {
        self.summaries.push_back(summary);
        while self.summaries.len() > self.max_summaries {
            self.summaries.pop_front();
        }
    }

    /// Store a new summary (in-memory + optional Qdrant sync).
    ///
    /// Qdrant sync is fire-and-forget on a spawned task — the in-memory
    /// write is synchronous, so the caller can immediately read back the data.
    pub async fn store(&mut self, summary: MemorySummary) {
        let summary_id = summary.id.clone();
        self.store_sync(summary);

        // Fire-and-forget Qdrant sync
        if let Some(qdrant) = self.qdrant.clone() {
            let last = self.summaries.back().cloned();
            if let Some(last) = last {
                tokio::spawn(async move {
                    let payload: std::collections::HashMap<String, serde_json::Value> = [
                        ("data".to_string(), serde_json::to_value(&last).unwrap_or(serde_json::Value::Null)),
                    ].into_iter().collect();

                    let client = match qdrant.build_client().await {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::warn!("Qdrant sync failed (connection): {}", e);
                            return;
                        }
                    };

                    let _ = qdrant.ensure_collection(&client, 1).await;

                    use qdrant_client::qdrant::{PointStruct, UpsertPointsBuilder};
                    let point = PointStruct::new(summary_id, vec![0.0f32; 1], payload);

                    if let Err(e) = client
                        .upsert_points(UpsertPointsBuilder::new(qdrant.collection.clone(), vec![point]))
                        .await
                    {
                        tracing::warn!("Qdrant sync failed (upsert): {}", e);
                    }
                });
            }
        }
    }

    /// Search summaries by keyword.
    pub fn search(&self, query: &str) -> Vec<&MemorySummary> {
        let query_lower = query.to_lowercase();
        self.summaries
            .iter()
            .filter(|s| {
                s.summary.to_lowercase().contains(&query_lower)
                    || s.key_decisions.iter().any(|d| d.decision.to_lowercase().contains(&query_lower))
                    || s.errors_learned.iter().any(|e| e.error.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// Get summaries for a specific session.
    pub fn by_session(&self, session_id: &str) -> Vec<&MemorySummary> {
        self.summaries
            .iter()
            .filter(|s| s.session_id == session_id)
            .collect()
    }

    /// Get summaries for a specific project.
    pub fn by_project(&self, project_id: &str) -> Vec<&MemorySummary> {
        self.summaries
            .iter()
            .filter(|s| s.project_id == project_id)
            .collect()
    }

    /// Get the most recent summary.
    pub fn latest(&self) -> Option<&MemorySummary> {
        self.summaries.back()
    }

    /// Total summaries stored.
    pub fn count(&self) -> usize {
        self.summaries.len()
    }

    /// Clear all summaries.
    pub fn clear(&mut self) {
        self.summaries.clear();
    }

    /// Get all key decisions across summaries.
    pub fn all_decisions(&self) -> Vec<&KeyDecision> {
        self.summaries
            .iter()
            .flat_map(|s| s.key_decisions.iter())
            .collect()
    }

    /// Get all errors learned across summaries.
    pub fn all_errors(&self) -> Vec<&ErrorLearned> {
        self.summaries
            .iter()
            .flat_map(|s| s.errors_learned.iter())
            .collect()
    }

    /// Get memory statistics.
    pub fn stats(&self) -> ConsolidatedStats {
        let total_decisions: usize = self.summaries.iter().map(|s| s.key_decisions.len()).sum();
        let total_errors: usize = self.summaries.iter().map(|s| s.errors_learned.len()).sum();
        let total_interactions: u32 = self.summaries.iter().map(|s| s.interaction_count).sum();

        ConsolidatedStats {
            total_summaries: self.summaries.len(),
            max_summaries: self.max_summaries,
            total_decisions,
            total_errors,
            total_interactions,
        }
    }
}

/// Statistics for consolidated memory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConsolidatedStats {
    pub total_summaries: usize,
    pub max_summaries: usize,
    pub total_decisions: usize,
    pub total_errors: usize,
    pub total_interactions: u32,
}

// ─── Summarizer Agent ─────────────────────────────────────────

/// Generates structured summaries from interaction history.
pub struct Summarizer;

impl Summarizer {
    /// Create a new summarizer.
    pub fn new() -> Self {
        Self
    }

    /// Generate a summary from a list of interactions.
    pub fn summarize(
        &self,
        session_id: &str,
        project_id: &str,
        interactions: &[InteractionSummary],
    ) -> MemorySummary {
        let key_decisions: Vec<KeyDecision> = interactions
            .iter()
            .filter(|i| i.kind == InteractionKind::Decision)
            .map(|i| KeyDecision {
                decision: i.content.clone(),
                rationale: i.context.clone(),
                agent: i.agent_id.clone(),
            })
            .collect();

        let errors_learned: Vec<ErrorLearned> = interactions
            .iter()
            .filter(|i| i.kind == InteractionKind::Error)
            .map(|i| ErrorLearned {
                error: i.content.clone(),
                fix: i.context.clone(),
                agent: i.agent_id.clone(),
            })
            .collect();

        let summary_text = self.generate_summary_text(interactions);

        let current_state = interactions
            .last()
            .map(|i| i.content.clone())
            .unwrap_or_default();

        let action_items = interactions
            .iter()
            .filter(|i| i.kind == InteractionKind::Task)
            .map(|i| format!("[{}] {}", i.agent_id, i.content))
            .collect();

        let count = interactions.len() as u32;
        let period_start = interactions.first()
            .map(|i| i.timestamp.clone())
            .unwrap_or_default();
        let period_end = interactions.last()
            .map(|i| i.timestamp.clone())
            .unwrap_or_default();

        MemorySummary {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            project_id: project_id.to_string(),
            summary: summary_text,
            key_decisions,
            errors_learned,
            current_state,
            action_items,
            interaction_count: count,
            period_start,
            period_end,
            compressed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Generate a text summary from interactions.
    fn generate_summary_text(&self, interactions: &[InteractionSummary]) -> String {
        let total = interactions.len();
        let decisions = interactions.iter().filter(|i| i.kind == InteractionKind::Decision).count();
        let errors = interactions.iter().filter(|i| i.kind == InteractionKind::Error).count();
        let tasks = interactions.iter().filter(|i| i.kind == InteractionKind::Task).count();

        format!(
            "Session summary: {} interactions total ({} decisions, {} errors, {} tasks). Key activities: {}",
            total, decisions, errors, tasks,
            interactions.iter().take(3).map(|i| i.content.as_str()).collect::<Vec<_>>().join("; ")
        )
    }
}

/// Summary of an interaction for the summarizer.
#[derive(Debug, Clone)]
pub struct InteractionSummary {
    pub content: String,
    pub context: String,
    pub kind: InteractionKind,
    pub agent_id: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionKind {
    Task,
    Decision,
    Error,
    Research,
    Code,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_search() {
        let mut memory = ConsolidatedMemory::new(10);

        memory.store_sync(MemorySummary {
            id: "1".to_string(),
            session_id: "s1".to_string(),
            project_id: "p1".to_string(),
            summary: "Used Rust for backend API".to_string(),
            key_decisions: vec![KeyDecision {
                decision: "Use Rust".to_string(),
                rationale: "Performance".to_string(),
                agent: "architect".to_string(),
            }],
            errors_learned: vec![],
            current_state: "API complete".to_string(),
            action_items: vec![],
            interaction_count: 50,
            period_start: "2024-01-01".to_string(),
            period_end: "2024-01-02".to_string(),
            compressed_at: chrono::Utc::now().to_rfc3339(),
        });

        let results = memory.search("Rust");
        assert_eq!(results.len(), 1);
        assert!(results[0].summary.contains("Rust"));
    }

    #[test]
    fn test_by_session() {
        let mut memory = ConsolidatedMemory::new(10);

        for i in 0..5 {
            memory.store_sync(MemorySummary {
                id: i.to_string(),
                session_id: if i < 3 { "s1".to_string() } else { "s2".to_string() },
                project_id: "p1".to_string(),
                summary: format!("Summary {}", i),
                key_decisions: vec![],
                errors_learned: vec![],
                current_state: String::new(),
                action_items: vec![],
                interaction_count: 10,
                period_start: String::new(),
                period_end: String::new(),
                compressed_at: chrono::Utc::now().to_rfc3339(),
            });
        }

        let s1 = memory.by_session("s1");
        assert_eq!(s1.len(), 3);
    }

    #[test]
    fn test_summarizer() {
        let summarizer = Summarizer::new();
        let interactions = vec![
            InteractionSummary {
                content: "Decided to use PostgreSQL".to_string(),
                context: "For authentication backend".to_string(),
                kind: InteractionKind::Decision,
                agent_id: "architect".to_string(),
                timestamp: "2024-01-01".to_string(),
            },
            InteractionSummary {
                content: "Found SQL injection vulnerability".to_string(),
                context: "Fixed by using parameterized queries".to_string(),
                kind: InteractionKind::Error,
                agent_id: "security".to_string(),
                timestamp: "2024-01-01".to_string(),
            },
            InteractionSummary {
                content: "Implemented login endpoint".to_string(),
                context: "POST /auth/login".to_string(),
                kind: InteractionKind::Task,
                agent_id: "coder".to_string(),
                timestamp: "2024-01-01".to_string(),
            },
        ];

        let summary = summarizer.summarize("s1", "p1", &interactions);
        assert_eq!(summary.key_decisions.len(), 1);
        assert_eq!(summary.errors_learned.len(), 1);
        assert!(summary.summary.contains("3 interactions"));
    }

    #[test]
    fn test_eviction() {
        let mut memory = ConsolidatedMemory::new(3);

        for i in 0..5 {
            memory.store_sync(MemorySummary {
                id: i.to_string(),
                session_id: "s1".to_string(),
                project_id: "p1".to_string(),
                summary: format!("Summary {}", i),
                key_decisions: vec![],
                errors_learned: vec![],
                current_state: String::new(),
                action_items: vec![],
                interaction_count: 10,
                period_start: String::new(),
                period_end: String::new(),
                compressed_at: chrono::Utc::now().to_rfc3339(),
            });
        }

        assert_eq!(memory.count(), 3);
    }

    #[test]
    fn test_all_decisions() {
        let mut memory = ConsolidatedMemory::new(10);

        memory.store_sync(MemorySummary {
            id: "1".to_string(),
            session_id: "s1".to_string(),
            project_id: "p1".to_string(),
            summary: String::new(),
            key_decisions: vec![
                KeyDecision { decision: "Use Rust".to_string(), rationale: "Fast".to_string(), agent: "a".to_string() },
                KeyDecision { decision: "Use Postgres".to_string(), rationale: "Reliable".to_string(), agent: "b".to_string() },
            ],
            errors_learned: vec![],
            current_state: String::new(),
            action_items: vec![],
            interaction_count: 10,
            period_start: String::new(),
            period_end: String::new(),
            compressed_at: chrono::Utc::now().to_rfc3339(),
        });

        let decisions = memory.all_decisions();
        assert_eq!(decisions.len(), 2);
    }
}