//! Supervisor actor — manages child agents with hierarchical supervision.

use crate::actor::echo::{EchoAgent, EchoStats, echo, ping, get_stats};
use async_trait::async_trait;
use ractor::{Actor, ActorRef, ActorProcessingErr, RpcReplyPort};
use std::collections::HashMap;

// ─── Messages ─────────────────────────────────────────────────

pub enum SupervisorMessage {
    SpawnEcho {
        name: String,
        reply: RpcReplyPort<Result<AgentHandle, String>>,
    },
    PingChild {
        name: String,
        reply: RpcReplyPort<Result<String, String>>,
    },
    EchoTo {
        name: String,
        content: String,
        reply: RpcReplyPort<Result<String, String>>,
    },
    GetChildStats {
        name: String,
        reply: RpcReplyPort<Result<EchoStats, String>>,
    },
    KillChild {
        name: String,
        reply: RpcReplyPort<Result<(), String>>,
    },
    ListChildren {
        reply: RpcReplyPort<Vec<AgentHandle>>,
    },
    ShutdownAll,
}

impl std::fmt::Debug for SupervisorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SupervisorMessage::SpawnEcho { name, .. } => write!(f, "SpawnEcho({})", name),
            SupervisorMessage::PingChild { name, .. } => write!(f, "PingChild({})", name),
            SupervisorMessage::EchoTo { name, content, .. } => write!(f, "EchoTo({}, {})", name, content),
            SupervisorMessage::GetChildStats { name, .. } => write!(f, "GetChildStats({})", name),
            SupervisorMessage::KillChild { name, .. } => write!(f, "KillChild({})", name),
            SupervisorMessage::ListChildren { .. } => write!(f, "ListChildren"),
            SupervisorMessage::ShutdownAll => write!(f, "ShutdownAll"),
        }
    }
}

// ─── Handle returned to callers ───────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentHandle {
    pub name: String,
    pub role: String,
    pub spawned_at: String,
}

// ─── State ────────────────────────────────────────────────────

pub struct SupervisorState {
    pub children: HashMap<String, ActorRef<EchoAgent>>,
    pub total_spawned: u64,
}

// ─── Actor Implementation ─────────────────────────────────────

pub struct Supervisor;

#[async_trait]
impl Actor for Supervisor {
    type Msg = SupervisorMessage;
    type State = SupervisorState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self>,
        _args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(SupervisorState {
            children: HashMap::new(),
            total_spawned: 0,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            SupervisorMessage::SpawnEcho { name, reply } => {
                let result = self.spawn_child(&name, state).await;
                let _ = reply.send(result);
            }

            SupervisorMessage::PingChild { name, reply } => {
                let result = match state.children.get(&name) {
                    Some(child) => ping(child).await.map_err(|e| e.to_string()),
                    None => Err(format!("Child '{}' not found", name)),
                };
                let _ = reply.send(result);
            }

            SupervisorMessage::EchoTo { name, content, reply } => {
                let result = match state.children.get(&name) {
                    Some(child) => echo(child, &content).await.map_err(|e| e.to_string()),
                    None => Err(format!("Child '{}' not found", name)),
                };
                let _ = reply.send(result);
            }

            SupervisorMessage::GetChildStats { name, reply } => {
                let result = match state.children.get(&name) {
                    Some(child) => get_stats(child).await.map_err(|e| e.to_string()),
                    None => Err(format!("Child '{}' not found", name)),
                };
                let _ = reply.send(result);
            }

            SupervisorMessage::KillChild { name, reply } => {
                if let Some(child) = state.children.remove(&name) {
                    child.get_cell().stop(None);
                    let _ = reply.send(Ok(()));
                } else {
                    let _ = reply.send(Err(format!("Child '{}' not found", name)));
                }
            }

            SupervisorMessage::ListChildren { reply } => {
                let handles: Vec<AgentHandle> = state
                    .children
                    .keys()
                    .map(|name| AgentHandle {
                        name: name.clone(),
                        role: "echo".to_string(),
                        spawned_at: chrono::Utc::now().to_rfc3339(),
                    })
                    .collect();
                let _ = reply.send(handles);
            }

            SupervisorMessage::ShutdownAll => {
                for (name, child) in state.children.drain() {
                    child.get_cell().stop(None);
                    tracing::info!("Supervisor killed child: {}", name);
                }
                myself.get_cell().stop(None);
            }
        }

        Ok(())
    }
}

impl Supervisor {
    async fn spawn_child(
        &self,
        name: &str,
        state: &mut SupervisorState,
    ) -> Result<AgentHandle, String> {
        if state.children.contains_key(name) {
            return Err(format!("Child '{}' already exists", name));
        }

        let (actor_ref, _handle) = Actor::spawn(
            Some(name.to_string()),
            EchoAgent,
            name.to_string(), // Arguments: agent_id
        )
        .await
        .map_err(|e| format!("Failed to spawn '{}': {}", name, e))?;

        let handle = AgentHandle {
            name: name.to_string(),
            role: "echo".to_string(),
            spawned_at: chrono::Utc::now().to_rfc3339(),
        };

        state.children.insert(name.to_string(), actor_ref);
        state.total_spawned += 1;

        tracing::info!("Supervisor spawned child: {}", name);
        Ok(handle)
    }

    pub async fn spawn() -> Result<ActorRef<Supervisor>, crate::CoreError> {
        let (cell, _handle) = Actor::spawn(None, Supervisor, ())
            .await
            .map_err(|e| crate::CoreError::Actor(format!("Failed to spawn Supervisor: {}", e)))?;
        Ok(cell)
    }
}

// ─── Supervisor Helper Functions ──────────────────────────────

pub async fn spawn_echo(
    supervisor: &ActorRef<Supervisor>,
    name: &str,
) -> Result<AgentHandle, crate::CoreError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    supervisor
        .cast(SupervisorMessage::SpawnEcho {
            name: name.to_string(),
            reply: RpcReplyPort::from(tx),
        })
        .map_err(|e| crate::CoreError::Actor(format!("Failed to send SpawnEcho: {}", e)))?;
    rx.await
        .map_err(|e| crate::CoreError::Actor(format!("SpawnEcho reply error: {}", e)))?
        .map_err(crate::CoreError::Actor)
}

pub async fn supervisor_echo_to(
    supervisor: &ActorRef<Supervisor>,
    child_name: &str,
    content: &str,
) -> Result<String, crate::CoreError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    supervisor
        .cast(SupervisorMessage::EchoTo {
            name: child_name.to_string(),
            content: content.to_string(),
            reply: RpcReplyPort::from(tx),
        })
        .map_err(|e| crate::CoreError::Actor(format!("Failed to send EchoTo: {}", e)))?;
    rx.await
        .map_err(|e| crate::CoreError::Actor(format!("EchoTo reply error: {}", e)))?
        .map_err(crate::CoreError::Actor)
}

pub async fn list_children(
    supervisor: &ActorRef<Supervisor>,
) -> Result<Vec<AgentHandle>, crate::CoreError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    supervisor
        .cast(SupervisorMessage::ListChildren {
            reply: RpcReplyPort::from(tx),
        })
        .map_err(|e| crate::CoreError::Actor(format!("Failed to send ListChildren: {}", e)))?;
    rx.await.map_err(|e| crate::CoreError::Actor(format!("ListChildren reply error: {}", e)))
}

pub async fn shutdown_all(
    supervisor: &ActorRef<Supervisor>,
) -> Result<(), crate::CoreError> {
    let _ = supervisor.cast(SupervisorMessage::ShutdownAll);
    Ok(())
}