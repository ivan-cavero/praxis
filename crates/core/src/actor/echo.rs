//! EchoAgent — A test actor that echoes messages back.
//!
//! Validates the actor model, message passing, and event bus integration.

use async_trait::async_trait;
use ractor::{Actor, ActorRef, ActorProcessingErr, RpcReplyPort};

// ─── Messages ─────────────────────────────────────────────────

/// Messages the EchoAgent understands.
pub enum EchoMessage {
    /// Echo the content back via the reply port.
    Echo { content: String, reply: RpcReplyPort<String> },
    /// Simple health check — returns "pong".
    Ping(RpcReplyPort<String>),
    /// Returns current agent statistics.
    GetStats(RpcReplyPort<EchoStats>),
    /// Gracefully shut down.
    Shutdown,
}

// State must be Debug for debugging
impl std::fmt::Debug for EchoMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EchoMessage::Echo { content, .. } => write!(f, "Echo({})", content),
            EchoMessage::Ping(_) => write!(f, "Ping"),
            EchoMessage::GetStats(_) => write!(f, "GetStats"),
            EchoMessage::Shutdown => write!(f, "Shutdown"),
        }
    }
}

/// Statistics collected by the EchoAgent.
#[derive(Debug, Clone)]
pub struct EchoStats {
    pub messages_processed: u64,
    pub uptime_seconds: u64,
    pub agent_id: String,
}

// ─── Actor State ──────────────────────────────────────────────

pub struct EchoState {
    pub id: String,
    pub message_count: u64,
    pub started_at: std::time::Instant,
}

// ─── Actor Implementation ─────────────────────────────────────

pub struct EchoAgent;

#[async_trait]
impl Actor for EchoAgent {
    type Msg = EchoMessage;
    type State = EchoState;
    type Arguments = String; // agent_id

    async fn pre_start(
        &self,
        _myself: ActorRef<Self>,
        agent_id: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(EchoState {
            id: agent_id,
            message_count: 0,
            started_at: std::time::Instant::now(),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        state.message_count += 1;

        match message {
            EchoMessage::Echo { content, reply } => {
                let response = format!("[{}] echo: {}", state.id, content);
                let _ = reply.send(response);
            }

            EchoMessage::Ping(reply) => {
                let _ = reply.send("pong".to_string());
            }

            EchoMessage::GetStats(reply) => {
                let _ = reply.send(EchoStats {
                    messages_processed: state.message_count,
                    uptime_seconds: state.started_at.elapsed().as_secs(),
                    agent_id: state.id.clone(),
                });
            }

            EchoMessage::Shutdown => {
                tracing::info!("EchoAgent '{}' shutting down", state.id);
            }
        }

        Ok(())
    }
}

// ─── Helper Functions ─────────────────────────────────────────

/// Send an echo message to an agent and wait for the response.
pub async fn echo(agent: &ActorRef<EchoAgent>, content: &str) -> Result<String, crate::CoreError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    agent
        .cast(EchoMessage::Echo {
            content: content.to_string(),
            reply: RpcReplyPort::from(tx),
        })
        .map_err(|e| crate::CoreError::Actor(format!("Failed to send echo: {}", e)))?;
    rx.await.map_err(|e| crate::CoreError::Actor(format!("Echo response error: {}", e)))
}

/// Ping an agent to check if it's alive.
pub async fn ping(agent: &ActorRef<EchoAgent>) -> Result<String, crate::CoreError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    agent
        .cast(EchoMessage::Ping(RpcReplyPort::from(tx)))
        .map_err(|e| crate::CoreError::Actor(format!("Failed to ping: {}", e)))?;
    rx.await.map_err(|e| crate::CoreError::Actor(format!("Ping response error: {}", e)))
}

/// Get statistics from an agent.
pub async fn get_stats(agent: &ActorRef<EchoAgent>) -> Result<EchoStats, crate::CoreError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    agent
        .cast(EchoMessage::GetStats(RpcReplyPort::from(tx)))
        .map_err(|e| crate::CoreError::Actor(format!("Failed to get stats: {}", e)))?;
    rx.await.map_err(|e| crate::CoreError::Actor(format!("Stats response error: {}", e)))
}