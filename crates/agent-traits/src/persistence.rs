//! EventStore trait — event sourcing persistence.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A stored event in the append-only event store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub id: Uuid,
    pub aggregate_id: Uuid,
    pub aggregate_type: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
    pub version: i64,
    pub created_at: String,
}

/// A snapshot of an aggregate's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSnapshot {
    pub aggregate_id: Uuid,
    pub aggregate_type: String,
    pub state: serde_json::Value,
    pub version: i64,
    pub updated_at: String,
}

/// The event store trait.
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append a new event to the store.
    async fn append(&self, event: StoredEvent) -> crate::Result<()>;

    /// Read all events for an aggregate, optionally starting from a version.
    async fn read_events(
        &self,
        aggregate_id: Uuid,
        after_version: Option<i64>,
    ) -> crate::Result<Vec<StoredEvent>>;

    /// Get the latest snapshot for an aggregate.
    async fn get_snapshot(&self, aggregate_id: Uuid) -> crate::Result<Option<StoredSnapshot>>;

    /// Save a snapshot for an aggregate.
    async fn save_snapshot(&self, snapshot: StoredSnapshot) -> crate::Result<()>;

    /// List all aggregate IDs of a given type.
    async fn list_aggregates(&self, aggregate_type: &str) -> crate::Result<Vec<Uuid>>;
}

/// Persistence mode selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistenceMode {
    Embedded, // SQLite (default)
    Remote,   // PostgreSQL (VPS)
}