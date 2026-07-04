//! Tauri IPC commands — Phase 1 integration with CoreRuntime.
//!
//! These commands are exposed to the Vue frontend via Tauri's invoke().
//! AppState is initialized in main.rs and passed to commands via State<>.

use praxis_agent_traits::persistence::EventStore;
use praxis_core::{CoreRuntime, EventBus, GoalResult};
use praxis_persistence::SqliteEventStore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::RwLock;
use uuid::Uuid;

// ─── Response Types ───────────────────────────────────────────

#[derive(Serialize)]
pub struct VersionInfo {
    pub version: String,
    pub commit: String,
}

#[derive(Serialize)]
pub struct StatusInfo {
    pub running: bool,
    pub uptime_secs: u64,
    pub version: String,
    pub iteration: u32,
    pub phase: String,
}

#[derive(Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub goal: String,
    pub status: String,
    pub phase: String,
    pub iteration: u32,
    pub started_at: String,
}

#[derive(Serialize)]
pub struct MetricsInfo {
    pub total_tokens: u64,
    pub active_sessions: u32,
    pub iteration: u32,
}

#[derive(Deserialize)]
pub struct RunGoalRequest {
    pub goal: String,
}

// ─── Application State ───────────────────────────────────────

/// Shared state held by Tauri and accessible from all commands.
pub struct AppState {
    pub runtime: RwLock<Option<CoreRuntime>>,
    pub bus: EventBus,
    /// Path to the SQLite database (optional). Used to open a separate
    /// connection for session queries while CoreRuntime holds the primary one.
    pub db_path: RwLock<Option<PathBuf>>,
    /// API server port (set by init_backend after binding).
    pub api_port: RwLock<Option<u16>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            runtime: RwLock::new(None),
            bus: EventBus::new(),
            db_path: RwLock::new(None),
            api_port: RwLock::new(None),
        }
    }

    /// Initialize the CoreRuntime with an optional event store database path.
    pub async fn init_runtime(&self, db_path: Option<PathBuf>) -> Result<(), String> {
        let mut rt = CoreRuntime::new().await.map_err(|e| e.to_string())?
            .with_default_memory();

        if let Some(ref path) = db_path {
            let store = SqliteEventStore::new(path).map_err(|e| e.to_string())?;
            rt = rt.with_event_store(store);
        }

        let mut guard = self.runtime.write().await;
        *guard = Some(rt);

        // Store db_path for later session queries
        let mut path_guard = self.db_path.write().await;
        *path_guard = db_path;

        Ok(())
    }

    /// Open a separate event store connection for read-only queries.
    /// Returns None if no db_path was configured.
    async fn open_store(&self) -> Option<Arc<SqliteEventStore>> {
        let path_guard = self.db_path.read().await;
        let path = path_guard.as_ref()?.clone();
        drop(path_guard);

        match SqliteEventStore::new(&path) {
            Ok(store) => Some(Arc::new(store)),
            Err(e) => {
                tracing::warn!("Failed to open event store for queries: {}", e);
                None
            }
        }
    }
}

// ─── Helper ────────────────────────────────────────────────────

/// Build a SessionInfo from an aggregate ID by loading its snapshot.
async fn load_session(store: &Arc<SqliteEventStore>, id: &Uuid) -> Option<SessionInfo> {
    let snap = store.get_snapshot(*id).await.ok()??;
    Some(SessionInfo {
        id: snap.aggregate_id.to_string(),
        goal: snap.state["goal"].as_str().unwrap_or("unknown").to_string(),
        status: if snap.state["phase"].as_str() == Some("Completed") {
            "completed".to_string()
        } else {
            "running".to_string()
        },
        phase: snap.state["phase"].as_str().unwrap_or("unknown").to_string(),
        iteration: snap.state["iteration"].as_u64().unwrap_or(0) as u32,
        started_at: snap.updated_at.clone(),
    })
}

// ─── Commands ─────────────────────────────────────────────────

#[tauri::command]
pub fn get_version() -> VersionInfo {
    VersionInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: option_env!("GIT_COMMIT").unwrap_or("unknown").to_string(),
    }
}

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<StatusInfo, String> {
    let guard = state.runtime.read().await;
    match guard.as_ref() {
        Some(rt) => {
            let phase = format!("{:?}", rt.loop_controller.state_machine.current());
            Ok(StatusInfo {
                running: true,
                uptime_secs: rt.loop_controller.started_at.elapsed().as_secs(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                iteration: rt.loop_controller.iteration,
                phase,
            })
        }
        None => Ok(StatusInfo {
            running: false,
            uptime_secs: 0,
            version: env!("CARGO_PKG_VERSION").to_string(),
            iteration: 0,
            phase: "idle".to_string(),
        }),
    }
}

#[tauri::command]
pub async fn get_sessions(state: State<'_, AppState>) -> Result<Vec<SessionInfo>, String> {
    let store = state.open_store().await;
    let Some(store) = store else {
        return Ok(vec![]);
    };

    // List all session aggregates
    let aggregate_ids = store
        .list_aggregates("session")
        .await
        .map_err(|e| e.to_string())?;

    // Fetch each snapshot and convert to SessionInfo
    let mut sessions: Vec<SessionInfo> = Vec::new();
    for agg_id in &aggregate_ids {
        if let Some(session) = load_session(&store, agg_id).await {
            sessions.push(session);
        }
    }

    // Sort by started_at descending (most recent first)
    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));

    Ok(sessions)
}

#[tauri::command]
pub async fn run_goal(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    request: RunGoalRequest,
) -> Result<String, String> {
    tracing::info!("Desktop run_goal: {}", request.goal);

    let mut guard = state.runtime.write().await;
    let rt = guard.as_mut().ok_or("Runtime not initialized")?;

    // Subscribe to EventBus and forward events to frontend
    let mut rx = state.bus.subscribe();
    let handle = app_handle.clone();
    let forwarder = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let payload = serde_json::json!({
                        "id": event.id.to_string(),
                        "kind": format!("{:?}", event.kind),
                        "source": event.source,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });
                    let _ = handle.emit("system:event", payload);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Event forwarder lagged by {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Run the goal
    let result: GoalResult = rt
        .run_goal(&request.goal, None, None)
        .await
        .map_err(|e| e.to_string())?;

    forwarder.abort();

    Ok(serde_json::to_string(&result).unwrap_or_default())
}

#[tauri::command]
pub async fn stop_session(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.runtime.read().await;
    match guard.as_ref() {
        Some(rt) => {
            rt.shutdown_handle()
                .store(true, std::sync::atomic::Ordering::Relaxed);
            tracing::info!("Session stop requested via desktop");
            Ok(())
        }
        None => Err("No active session".to_string()),
    }
}

#[tauri::command]
pub async fn get_metrics(state: State<'_, AppState>) -> Result<MetricsInfo, String> {
    let guard = state.runtime.read().await;
    match guard.as_ref() {
        Some(rt) => Ok(MetricsInfo {
            total_tokens: 0,
            active_sessions: 1,
            iteration: rt.loop_controller.iteration,
        }),
        None => Ok(MetricsInfo {
            total_tokens: 0,
            active_sessions: 0,
            iteration: 0,
        }),
    }
}

#[tauri::command]
pub async fn get_api_port(state: State<'_, AppState>) -> Result<u16, String> {
    let guard = state.api_port.read().await;
    guard.ok_or_else(|| "API server not yet started".to_string())
}
