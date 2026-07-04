//! praxis Desktop — Tauri v2 binary.
//!
//! Embeds the core runtime + API server and serves the dashboard via WebView.
//! The Vue dashboard is served from `../dashboard/dist` (production)
//! or `http://localhost:3000` (development via `bun run dev`).
//!
//! Features: system tray (show/hide, new session, settings, quit),
//! auto-updater from GitHub releases, window close-to-hide.

mod commands;
mod events;

use commands::AppState;
use praxis_core::CoreRuntime;
use praxis_persistence::SqliteEventStore;
use std::path::PathBuf;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

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

            // ── 1. Auto-updater plugin ──────────────────────────
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;

            // ── 2. System tray ──────────────────────────────────
            build_tray(app)?;

            // ── 3. Window close → hide instead of quit ─────────
            // (handled in on_window_event below)

            // ── 4. Background backend initialization ───────────
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                init_backend(&handle).await;
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide window on close instead of quitting.
            // User must use tray → Quit to fully exit.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().ok();
                api.prevent_close();
            }
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

/// Build the system tray icon and menu.
fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Menu items
    let show_hide = MenuItem::with_id(app, "show_hide", "Show/Hide", true, None::<&str>)?;
    let new_session = MenuItem::with_id(app, "new_session", "New Session", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_hide, &new_session, &settings, &separator, &quit])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("praxis — Autonomous Multi-Agent System")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app_handle, event| {
            let window = app_handle.get_webview_window("main");
            match event.id().as_ref() {
                "show_hide" => {
                    if let Some(w) = window {
                        if w.is_visible().ok().unwrap_or(false) {
                            w.hide().ok();
                        } else {
                            w.show().ok();
                            w.set_focus().ok();
                        }
                    }
                }
                "new_session" => {
                    // Emit event to frontend — the dashboard will navigate to run a new goal
                    let _ = app_handle.emit("tray:new_session", ());
                }
                "settings" => {
                    // Emit event to frontend — the dashboard will navigate to settings
                    let _ = app_handle.emit("tray:settings", ());
                }
                "quit" => {
                    app_handle.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            // Left click → toggle window visibility
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app_handle = tray.app_handle();
                if let Some(window) = app_handle.get_webview_window("main") {
                    if window.is_visible().ok().unwrap_or(false) {
                        window.hide().ok();
                    } else {
                        window.show().ok();
                        window.set_focus().ok();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
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
            runtime = runtime
                .with_default_memory()
                .with_data_dir(data_dir.clone());
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
            port: api_port,
            hostname: gethostname::gethostname().to_string_lossy().to_string(),
            started_at: chrono::Utc::now(),
            bus: praxis_core::EventBus::new(),
            auth: std::sync::Arc::new(
                praxis_core::api::auth::AuthState::from_file_or_create(
                    &data_dir.join("jwt.secret"),
                ),
            ),
            vault,
            data_dir: data_dir.clone(),
            token_counters: std::sync::Arc::new(
                std::sync::RwLock::new(
                    praxis_core::api::routes::TokenCounters::default(),
                ),
            ),
            session_registry: std::sync::Arc::new(
                std::sync::RwLock::new(Vec::new()),
            ),
            event_store: {
                let db_path = data_dir.join("state.db");
                praxis_persistence::SqliteEventStore::new(&db_path)
                    .ok()
                    .map(std::sync::Arc::new)
            },
            pairing: None,
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
