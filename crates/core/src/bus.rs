//! EventBus — Publish/subscribe event system using tokio::sync::broadcast.
//!
//! The central nervous system of praxis. Every subsystem publishes
//! events here; subscribers (logs, metrics, dashboard, CLI) receive them.
//!
//! # Design
//! - Channel capacity: 1024 events (configurable)
//! - Slow subscribers: dropped (broadcast channel behavior)
//! - Events are serializable (sent to WebSocket clients)

use praxis_shared::protocol::SystemEvent;

use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Default capacity of the internal broadcast channel.
const DEFAULT_CHANNEL_CAPACITY: usize = 1024;

/// Shared, cloneable handle to the event bus.
#[derive(Clone)]
pub struct EventBus {
    tx: Arc<broadcast::Sender<SystemEvent>>,
    capacity: usize,
}

impl EventBus {
    /// Create a new event bus with the default capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CHANNEL_CAPACITY)
    }

    /// Create a new event bus with a specific channel capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx: Arc::new(tx),
            capacity,
        }
    }

    /// Publish an event to all subscribers.
    ///
    /// Returns the number of subscribers that received the event.
    /// If all receivers have been dropped, returns `Ok(0)`.
    /// If all channels are at capacity, the oldest unread message is dropped
    /// and `Err` is returned with the number of active receivers.
    pub fn publish(&self, kind: praxis_shared::protocol::MessageKind, source: &str) -> usize {
        let event = SystemEvent {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            kind,
            source: source.to_string(),
            metadata: serde_json::Value::Object(Default::default()),
        };

        match self.tx.send(event) {
            Ok(receiver_count) => receiver_count,
            Err(broadcast::error::SendError(_event)) => {
                // No active receivers — this is normal during shutdown
                0
            }
        }
    }

    /// Subscribe to all events.
    ///
    /// Returns a `Receiver` that can be awaited in an async task.
    /// If the receiver lags behind, the oldest events will be dropped.
    pub fn subscribe(&self) -> broadcast::Receiver<SystemEvent> {
        self.tx.subscribe()
    }

    /// Return the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }

    /// Return the channel capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience subscriber that logs all events to tracing.
pub struct LogSubscriber {
    rx: broadcast::Receiver<SystemEvent>,
}

impl LogSubscriber {
    /// Create a new log subscriber. Spawn this in a background task.
    pub fn new(bus: &EventBus) -> Self {
        Self {
            rx: bus.subscribe(),
        }
    }

    /// Run the subscriber loop. Consumes `self` — spawn this in a tokio task.
    pub async fn run(mut self) {
        use tracing::info;
        while let Ok(event) = self.rx.recv().await {
            info!(
                event_id = %event.id,
                source = %event.source,
                timestamp = %event.timestamp,
                "system event"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use praxis_shared::protocol::MessageKind;


    #[tokio::test]
    async fn test_publish_subscribe() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.publish(
            MessageKind::TokenUsed {
                provider: "openai".into(),
                model: "gpt-5".into(),
                input: 100,
                output: 50,
            },
            "test",
        );

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for event")
            .expect("recv error");

        assert_eq!(event.source, "test");
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(
            MessageKind::SessionHeartbeat,
            "test",
        );

        // Both subscribers receive the event
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), rx1.recv())
            .await
            .expect("timeout")
            .expect("recv error");

        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), rx2.recv())
            .await
            .expect("timeout")
            .expect("recv error");
    }

    #[tokio::test]
    async fn test_subscriber_count() {
        let bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0);

        let _rx = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);

        let _rx2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
    }
}