//! praxis Desktop — Tauri v2 binary.
//!
//! Embeds the core runtime + API server and serves the dashboard via WebView.
//! The Vue dashboard is served from `../dashboard/dist` (production)
//! or `http://localhost:3000` (development via `bun run dev`).

mod commands;
mod events;

use commands::AppState;
use praxis_core::CoreRuntime;
use praxis_persistence::SqliteEventStore;
use std::path::PathBuf;
use tauri::{Emitter, Manager};

fn main() {
    run();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // ─── Initialize state ─────────────────────────────────────
    // CoreRuntime + API server are started in setup() to have access to AppHandle.
    let initial_state = AppState::new();

    tauri::Builder::default()
        .manage(initial_state)
        .setup(|app| {
            tracing::info!("praxis Desktop starting");
            let handle = app.handle().clone();

            // Start background initialization
            tauri::async_runtime::spawn(async move {
                init_backend(&handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_version,
            commands::get_status,
            commands::get_sessions,
            commands::run_goal,
            commands::stop_session,
            commands::get_metrics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Initialize CoreRuntime and API server in the background.
async fn init_backend(handle: &tauri::AppHandle) {
    let data_dir = dirs::data_dir()
        .map(|d| d.join("praxis"))
        .unwrap_or_else(|| PathBuf::from(".praxis"));
    std::fs::create_dir_all(&data_dir).ok();
    let db_path = data_dir.join("praxis.db");

    // 1. Initialize CoreRuntime
    let state = handle.state::<AppState>();
    match CoreRuntime::new().await {
        Ok(mut runtime) => {
            match SqliteEventStore::new(&db_path) {
                Ok(store) => {
                    runtime = runtime.with_event_store(store);
                    tracing::info!("Event store attached at: {:?}", db_path);
                }
                Err(e) => {
                    tracing::warn!("Failed to create event store: {}", e);
                }
            }

            let mut path_guard = state.db_path.write().await;
            *path_guard = Some(db_path.clone());
            drop(path_guard);

            let mut guard = state.runtime.write().await;
            *guard = Some(runtime);
        }
        Err(e) => {
            tracing::error!("Failed to create CoreRuntime: {}", e);
        }
    }

    // 2. Start API server on a random port
    // Bind to get the actual port
    let addr = format!("127.0.0.1:0");
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind API server: {}", e);
            return;
        }
    };
    let local_addr = match listener.local_addr() {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("Failed to get API server address: {}", e);
            return;
        }
    };
    let api_port = local_addr.port();

    tracing::info!(
        "API server starting on http://{}:{}",
        local_addr.ip(),
        api_port
    );

    let vault = std::sync::Arc::new(
        praxis_vault::VaultService::with_path(
            data_dir.join("credentials.vault.json"),
            None,
        )
    );
    let _ = vault.init();

    let app = praxis_core::api::ApiServer::router(
        praxis_core::api::AppState {
            version: env!("CARGO_PKG_VERSION").to_string(),
            started_at: chrono::Utc::now(),
            bus: praxis_core::EventBus::new(),
            auth: std::sync::Arc::new(
                praxis_core::api::auth::AuthState::from_file_or_create(
                    &data_dir.join("jwt.secret"),
                ),
            ),
            vault,
            data_dir,
        },
    );

    // Spawn API server in background
    tauri::async_runtime::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("API server error: {}", e);
        }
    });

    // 3. Tell the frontend the API port
    tracing::info!("API server listening on port {}", api_port);
    let _ = handle.emit("api:ready", api_port);

    // Emit core ready
    let _ = handle.emit("core:ready", ());
}

// Re-exports for Tauri's state management
pub use commands::{AppState as DesktopAppState, MetricsInfo, RunGoalRequest, SessionInfo, StatusInfo, VersionInfo};
