//! REST API routes.
//!
//! Architecture:
//!   %APPDATA%/praxis/          (o ~/.config/praxis/)
//!   ├── projects.json             ← TODOS los proyectos con forge_toml embebido
//!   ├── jwt.secret                ← Global
//!   └── credentials.vault.json    ← Global (API keys del usuario)

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get, post, put},
};
use praxis_agent_traits::persistence::EventStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Shared state for the API server.
#[derive(Clone)]
pub struct AppState {
    pub version: String,
    pub port: u16,
    pub hostname: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub bus: crate::EventBus,
    pub auth: std::sync::Arc<super::auth::AuthState>,
    pub vault: std::sync::Arc<praxis_vault::VaultService>,
    /// Directory where ALL project data lives (%APPDATA%/praxis).
    pub data_dir: std::path::PathBuf,
    /// Global token usage counters (updated by EventBus listener).
    pub token_counters: std::sync::Arc<std::sync::RwLock<TokenCounters>>,
    /// Session registry (shared with CoreRuntime).
    pub session_registry: std::sync::Arc<std::sync::RwLock<Vec<SessionEntry>>>,
    /// Active goal runs: session_id → shutdown handle + live token/cost counters.
    pub active_runs:
        std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, ActiveRun>>>,
    /// Persistent event store (SQLite) shared with `praxis run --goal`.
    /// Used to read persisted sessions from other processes.
    pub event_store: Option<std::sync::Arc<praxis_persistence::SqliteEventStore>>,
    /// Optional pairing system (enabled via --pair flag).
    pub pairing: Option<std::sync::Arc<super::pairing::PairingState>>,
}

/// A goal run spawned via the API. Holds the shutdown flag and live counters.
#[derive(Clone)]
pub struct ActiveRun {
    /// Set to true to request graceful shutdown of this session's loop.
    pub shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Live token count (updated by EventBus listener for this session).
    pub tokens_used: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Live cost in USD (updated by EventBus listener for this session).
    pub cost_usd: std::sync::Arc<std::sync::RwLock<f64>>,
    /// Project ID this run belongs to.
    pub project_id: String,
    /// Goal text.
    pub goal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenCounters {
    pub total_input: u64,
    pub total_output: u64,
    pub by_provider: HashMap<String, u64>,
    pub by_model: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: String,
    pub project: String,
    pub goal: String,
    pub phase: String,
    pub iteration: u32,
    pub status: String, // running, completed, failed
    pub started_at: String,
    pub completed_at: Option<String>,
    /// Total tokens consumed by this session (live, updated by EventBus).
    #[serde(default)]
    pub tokens_used: u64,
    /// Estimated cost in USD (live, updated by EventBus).
    #[serde(default)]
    pub cost_usd: f64,
}

/// API server configuration.
pub struct ApiServerConfig {
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub vault_password: Option<String>,
    /// Data directory (%APPDATA%/praxis or ~/.config/praxis).
    pub data_dir: std::path::PathBuf,
    /// Enable QR pairing system for remote connections.
    pub enable_pairing: bool,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            cors_origins: vec!["*".to_string()],
            vault_password: None,
            data_dir: std::path::PathBuf::from("."),
            enable_pairing: false,
        }
    }
}

/// The main API server.
pub struct ApiServer {
    config: ApiServerConfig,
}

impl ApiServer {
    pub fn new(config: ApiServerConfig) -> Self {
        Self { config }
    }

    pub fn router(state: AppState) -> Router {
        let cors = tower_http::cors::CorsLayer::permissive();
        let auth_state = state.auth.clone();

        // Authenticated routes
        let auth_routes = Router::new()
            .route("/api/projects", get(projects::list))
            .route("/api/projects", post(projects::create))
            .route("/api/projects/{id}", get(projects::get_one))
            .route("/api/projects/{id}", put(projects::update))
            .route("/api/projects/{id}", delete(projects::delete))
            .route("/api/projects/{id}/config", get(projects::get_config))
            .route("/api/projects/{id}/config", put(projects::update_config))
            .route("/api/projects/{id}/run", post(sessions::run_goal))
            .route("/api/projects/{id}/plan", post(sessions::plan_goal))
            .route("/api/sessions", get(sessions::list))
            .route("/api/sessions/{id}", get(sessions::get_one))
            .route("/api/sessions/{id}/stop", post(sessions::stop))
            .route("/api/sessions/{id}/events", get(sessions::list_events))
            .route(
                "/api/sessions/{id}/checkpoint",
                get(sessions::get_checkpoint),
            )
            .route("/api/sessions/{id}/state", get(sessions::get_state))
            .route("/api/sessions/{id}/rollback", post(sessions::rollback))
            .route("/api/sessions/{id}/diff", get(sessions::diff))
            .route("/api/agents", get(agent_crud::list_agents))
            .route("/api/agents", post(agent_crud::create_agent))
            .route("/api/agents/{name}", get(agent_crud::get_agent))
            .route("/api/agents/{name}", put(agent_crud::update_agent))
            .route("/api/agents/{name}", delete(agent_crud::delete_agent))
            .route("/api/skills", get(routes::list_skills))
            .route("/api/memory/stats", get(debug::memory_stats))
            .route("/api/debug/sessions", get(debug::debug_sessions))
            .route("/api/devices", get(super::pairing::list_devices))
            .route(
                "/api/devices/{device_id}",
                delete(super::pairing::revoke_device),
            )
            .with_state(Arc::new(state.clone()))
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                super::auth::auth_middleware,
            ));

        // Public routes (no auth required)
        let public_routes = Router::new()
            // Pairing endpoints — no auth (they ARE the auth mechanism)
            .route("/api/pair", post(super::pairing::create_pairing))
            .route("/api/pair/{code}", get(super::pairing::pairing_page))
            .route(
                "/api/pair/{code}/confirm",
                post(super::pairing::confirm_pairing),
            )
            .route(
                "/api/pair/{code}/status",
                get(super::pairing::get_pairing_status),
            )
            .route(
                "/api/pair/{code}/token",
                post(super::pairing::get_pairing_token),
            )
            // Existing public routes
            .route("/api/health", get(routes::health))
            .route("/api/metrics/tokens", get(routes::token_metrics))
            .route("/api/metrics/summary", get(routes::metrics_summary))
            .route("/api/metrics/context", get(routes::context_metrics))
            .route("/api/vault/keys", get(vault::list_keys))
            .route("/api/vault/keys", post(vault::set_key))
            .route("/api/vault/keys/{provider}", delete(vault::delete_key))
            .route("/api/inject", post(routes::inject))
            .route("/ws/global", get(super::ws::ws_handler))
            .with_state(Arc::new(state));

        Router::new()
            .merge(public_routes)
            .merge(auth_routes)
            .layer(cors)
    }

    pub async fn start(self) -> anyhow::Result<()> {
        let bus = crate::EventBus::new();
        let data_dir = self.config.data_dir.clone();

        // Ensure data directory exists
        std::fs::create_dir_all(&data_dir)?;

        // Initialize JWT auth
        let secret_path = data_dir.join("jwt.secret");
        let auth = std::sync::Arc::new(super::auth::AuthState::from_file_or_create(&secret_path));

        // Initialize vault
        let vault_path = data_dir.join("credentials.vault.json");
        let vault = std::sync::Arc::new(praxis_vault::VaultService::with_path(
            vault_path,
            self.config.vault_password.clone(),
        ));
        vault.init()?;

        // Initialize token counters from event store or start fresh
        let token_counters = std::sync::Arc::new(std::sync::RwLock::new(TokenCounters::default()));
        let session_registry = std::sync::Arc::new(std::sync::RwLock::new(Vec::new()));
        let active_runs: std::sync::Arc<
            std::sync::RwLock<std::collections::HashMap<String, ActiveRun>>,
        > = std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));

        // Open shared event store for reading sessions from other processes
        let db_path = data_dir.join("state.db");
        let event_store = if db_path.exists() {
            tracing::info!("Opening shared event store at {}", db_path.display());
            praxis_persistence::SqliteEventStore::new(&db_path)
                .ok()
                .map(std::sync::Arc::new)
        } else {
            tracing::info!(
                "No shared event store found ({}), sessions from other processes won't appear",
                db_path.display()
            );
            None
        };

        // Spawn EventBus listener: update token counters from live events.
        // Clone BEFORE moving originals into AppState.
        let token_counters_listener = token_counters.clone();
        let mut bus_rx = bus.subscribe();
        tokio::spawn(async move {
            loop {
                match bus_rx.recv().await {
                    Ok(event) => {
                        if let praxis_shared::protocol::MessageKind::TokenUsed {
                            provider,
                            model,
                            input,
                            output,
                        } = event.kind
                        {
                            let mut counters = token_counters_listener
                                .write()
                                .expect("RwLock not poisoned");
                            counters.total_input += input as u64;
                            counters.total_output += output as u64;
                            *counters.by_provider.entry(provider).or_insert(0) +=
                                (input + output) as u64;
                            *counters.by_model.entry(model).or_insert(0) += (input + output) as u64;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!("Token counter listener lagged by {} events", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        // Optionally create pairing state (enabled via --pair flag or config)
        let pairing = self.config.enable_pairing.then(|| {
            std::sync::Arc::new(super::pairing::PairingState::new(300)) // 5 min TTL
        });

        // Determine hostname
        let hostname = gethostname::gethostname().to_string_lossy().to_string();

        // If pairing is enabled, generate and display the QR immediately
        if let Some(ref pairing_state) = pairing {
            let (pair_code, path) = pairing_state.generate().await;
            let qr_url = format!("http://{hostname}:{}{path}", self.config.port);

            // Generate ASCII QR and print it
            if let Ok(qr_code) = qrcode::QrCode::new(&qr_url) {
                let qr_string = qr_code.render().light_color(' ').dark_color('█').build();
                println!();
                println!("╔══════════════════════════════════╗");
                println!("║     🔗 PAIR YOUR DEVICE          ║");
                println!("╠══════════════════════════════════╣");
                println!("║  Scan this QR or open the URL:   ║");
                println!("║  {}", qr_url);
                println!("╠══════════════════════════════════╣");
                println!("║  Code: {}               ║", pair_code);
                println!("║  Expires: 5 minutes              ║");
                println!("╚══════════════════════════════════╝");
                println!();
                println!("{}", qr_string);
                println!();
            }
        }

        // Generate and display first-run admin token
        let first_run_token = super::auth::generate_first_run_token(&auth).ok();
        if let Some(ref token) = first_run_token {
            println!();
            println!("╔══════════════════════════════════════════════════════════╗");
            println!("║     🔑 FIRST-RUN ADMIN TOKEN                           ║");
            println!("║  Copy this token and paste it in the dashboard login:  ║");
            println!("║                                                          ║");
            println!("║  {}", token);
            println!("║                                                          ║");
            println!("║  Expires in 24 hours                                     ║");
            println!("╚══════════════════════════════════════════════════════════╝");
            println!();
        }

        let state = AppState {
            version: env!("CARGO_PKG_VERSION").to_string(),
            port: self.config.port,
            hostname,
            started_at: chrono::Utc::now(),
            bus,
            auth,
            vault,
            data_dir,
            token_counters,
            session_registry,
            active_runs,
            event_store,
            pairing,
        };

        let app = Self::router(state);
        let addr = format!("0.0.0.0:{}", self.config.port);
        tracing::info!("API server starting on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        let local_addr = listener.local_addr()?;
        tracing::info!("API server listening on {}", local_addr);

        axum::serve(listener, app)
            .await
            .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

        Ok(())
    }
}

// ─── Helper: read/write projects.json ─────────────────────────

fn projects_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("projects.json")
}

fn read_projects(data_dir: &std::path::Path) -> Vec<ProjectEntry> {
    let path = projects_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn write_projects(data_dir: &std::path::Path, projects: &[ProjectEntry]) -> Result<(), String> {
    let path = projects_path(data_dir);
    serde_json::to_string_pretty(projects)
        .map_err(|e| e.to_string())
        .and_then(|content| std::fs::write(&path, &content).map_err(|e| e.to_string()))
}

/// Write a project's forge.toml content to a temp file for `load_forge_config`.
/// Returns the temp file path. Caller must clean up.
fn write_temp_config(forge_toml: &str, project_name: &str) -> Result<std::path::PathBuf, String> {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join(format!("praxis-{}.toml", project_name));
    std::fs::write(&path, forge_toml).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Create a git worktree for an isolated session (blocking version for API server).
///
/// Creates a new worktree at `../praxis-worktree-<session-id>` with a new branch.
/// Returns the worktree path. The caller must clean up via `remove_worktree_blocking`.
fn create_worktree_blocking(session_id: &str) -> Option<std::path::PathBuf> {
    // Check if we're in a git repo
    let status = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    if !status.success() {
        tracing::info!("Not in a git repo, skipping worktree creation");
        return None;
    }

    let short_id = &session_id[..8.min(session_id.len())];
    let branch_name = format!("praxis-{}", short_id);
    let worktree_path = std::env::current_dir()
        .ok()?
        .parent()?
        .join(format!("praxis-worktree-{}", short_id));

    let output = std::process::Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            &branch_name,
            worktree_path.to_str()?,
            "HEAD",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        tracing::warn!(
            "Failed to create worktree: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }

    tracing::info!(
        "Created worktree: {} (branch: {})",
        worktree_path.display(),
        branch_name
    );
    Some(worktree_path)
}

/// Remove a git worktree and its branch (blocking version for API server).
fn remove_worktree_blocking(worktree_path: &std::path::Path) {
    let path_str = worktree_path.to_string_lossy().to_string();
    let _ = std::process::Command::new("git")
        .args(["worktree", "remove", "--force", &path_str])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    let branch_name = worktree_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .replace("praxis-worktree-", "praxis-");
    let _ = std::process::Command::new("git")
        .args(["branch", "-D", &branch_name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    tracing::info!("Removed worktree: {}", worktree_path.display());
}

/// Default forge.toml template for new projects.
const DEFAULT_FORGE_TOML: &str = r#"[project]
name = "{name}"
version = "0.1.0"

[providers.nan]
base_url = "https://api.nan.builders/v1"
api_key = "env:NAN_API_KEY"
default_model = "qwen3.6"

[roles.architect]
model = "claude-sonnet-4-20250514"
temperature = 0.2
tools = ["filesystem", "web_search"]
system_prompt = "You are a senior software architect."

[roles.coder]
model = "gpt-4o"
temperature = 0.3
tools = ["filesystem", "execute_command"]
system_prompt = "You implement clean, maintainable code."

[roles.reviewer]
model = "gpt-4o"
temperature = 0.2
tools = ["filesystem"]
system_prompt = "Review for correctness and quality."

[roles.security]
model = "claude-sonnet-4-20250514"
temperature = 0.1
tools = ["filesystem", "grep"]
system_prompt = "Audit for security vulnerabilities."

[roles.tester]
model = "gpt-4o"
temperature = 0.2
tools = ["filesystem", "execute_command"]
system_prompt = "Generate and execute tests."

[roles.researcher]
model = "gpt-4o"
temperature = 0.3
tools = ["web_search", "read_url"]
system_prompt = "Research technical topics."

[[goals]]
name = "full-feature"
agents = ["architect", "coder", "reviewer", "security", "tester"]
gates = ["review.pass", "security.no_critical", "test.pass"]
max_iterations = 10

[[goals]]
name = "quick-fix"
agents = ["coder", "reviewer"]
max_iterations = 3

[limits]
max_iterations_per_goal = 50
max_iterations_per_phase = 5
session_ttl_seconds = 3600
phase_timeout_seconds = 300
"#;

// ─── Project types ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub created_at: String,
    #[serde(default)]
    pub last_active: String,
    #[serde(default)]
    pub forge_toml: String,
    /// Path to the per-project directory (e.g., ~/.config/praxis/projects/<name>).
    /// None for legacy projects created before the directory structure existed.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Optional forge.toml content. Auto-generated if empty.
    #[serde(default)]
    pub forge_toml: String,
    /// Optional path to an existing codebase. If provided, the project points
    /// to this directory instead of creating a new one under data_dir/projects.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateProjectRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub forge_toml: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateConfigRequest {
    pub config: String,
}

// ─── Goal Run API ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RunGoalRequest {
    pub goal: String,
    /// Completion criterion: "coding", "manual", "stagnant=N", "until:<command>"
    #[serde(default)]
    pub completion: Option<String>,
    /// Shell command that must exit 0 for the goal to be achieved.
    #[serde(default)]
    pub until: Option<String>,
    /// Maximum total tokens (session budget cap).
    #[serde(default)]
    pub max_tokens: Option<u64>,
    /// Maximum estimated cost in USD (session budget cap).
    #[serde(default)]
    pub max_cost_usd: Option<f64>,
    /// Number of parallel reviewers.
    #[serde(default)]
    pub parallel_reviewers: Option<u32>,
    /// Built-in skill IDs to enable (e.g., ["rust-best-practices", "security"]).
    #[serde(default)]
    pub skills: Vec<String>,
    /// Create a git worktree for this session (isolated working directory).
    #[serde(default)]
    pub worktree: bool,
}

#[derive(Serialize)]
pub struct SkillInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct RunGoalResponse {
    pub session_id: String,
    pub project_id: String,
    pub goal: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct SessionStateResponse {
    pub session_id: String,
    pub phase: String,
    pub iteration: u32,
    pub tokens_used: u64,
    pub cost_usd: f64,
    pub status: String,
    /// STATE.md content (if the state file exists in the working directory).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_file: Option<String>,
}

#[derive(Deserialize)]
pub struct SetKeyRequest {
    pub provider: String,
    pub api_key: String,
    pub base_url: Option<String>,
}

#[derive(Serialize)]
pub struct ProviderKeyResponse {
    pub provider: String,
    pub key_masked: String,
    pub has_key: bool,
}

#[derive(Serialize)]
pub struct ListKeysResponse {
    pub providers: Vec<ProviderKeyResponse>,
    pub total: usize,
}

pub mod vault {
    use super::*;

    pub async fn list_keys(State(state): State<Arc<AppState>>) -> Json<ListKeysResponse> {
        let keys = state.vault.list_keys().unwrap_or_default();
        let providers: Vec<ProviderKeyResponse> = keys
            .into_iter()
            .map(|provider| {
                let has_key = state.vault.get(&provider).unwrap_or_default().is_some();
                let key_masked = if has_key {
                    let key = state.vault.get(&provider).expect("has_key verified above").expect("vault value present");
                    if key.len() > 8 {
                        format!("{}...{}", &key[..4], &key[key.len() - 4..])
                    } else {
                        "****".to_string()
                    }
                } else {
                    String::new()
                };
                ProviderKeyResponse {
                    provider,
                    key_masked,
                    has_key,
                }
            })
            .collect();
        Json(ListKeysResponse {
            total: providers.len(),
            providers,
        })
    }

    pub async fn set_key(
        State(state): State<Arc<AppState>>,
        Json(request): Json<SetKeyRequest>,
    ) -> (StatusCode, Json<ProviderKeyResponse>) {
        if !request
            .provider
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
        {
            return (
                StatusCode::BAD_REQUEST,
                Json(ProviderKeyResponse {
                    provider: request.provider,
                    key_masked: String::new(),
                    has_key: false,
                }),
            );
        }
        if let Err(e) = state.vault.set(&request.provider, &request.api_key) {
            tracing::error!("Failed to store API key: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ProviderKeyResponse {
                    provider: request.provider,
                    key_masked: String::new(),
                    has_key: false,
                }),
            );
        }
        let masked = if request.api_key.len() > 8 {
            format!(
                "{}...{}",
                &request.api_key[..4],
                &request.api_key[request.api_key.len() - 4..]
            )
        } else {
            "****".to_string()
        };
        (
            StatusCode::OK,
            Json(ProviderKeyResponse {
                provider: request.provider,
                key_masked: masked,
                has_key: true,
            }),
        )
    }

    pub async fn delete_key(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(provider): axum::extract::Path<String>,
    ) -> (StatusCode, Json<ProviderKeyResponse>) {
        if let Err(e) = state.vault.delete(&provider) {
            tracing::error!("Failed to delete key: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ProviderKeyResponse {
                    provider,
                    key_masked: String::new(),
                    has_key: false,
                }),
            );
        }
        (
            StatusCode::OK,
            Json(ProviderKeyResponse {
                provider,
                key_masked: String::new(),
                has_key: false,
            }),
        )
    }
}

// ─── Projects API ─────────────────────────────────────────────

pub mod projects {
    use super::*;

    pub async fn list(State(state): State<Arc<AppState>>) -> Json<Vec<ProjectEntry>> {
        let projects = read_projects(&state.data_dir);
        Json(projects)
    }

    pub async fn get_one(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> Result<Json<ProjectEntry>, StatusCode> {
        let projects = read_projects(&state.data_dir);
        projects
            .into_iter()
            .find(|p| p.id == id)
            .map(Json)
            .ok_or(StatusCode::NOT_FOUND)
    }

    pub async fn create(
        State(state): State<Arc<AppState>>,
        Json(request): Json<CreateProjectRequest>,
    ) -> Result<(StatusCode, Json<ProjectEntry>), StatusCode> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let forge_toml = if request.forge_toml.is_empty() {
            DEFAULT_FORGE_TOML.replace("{name}", &request.name)
        } else {
            request.forge_toml
        };

        // Use custom path if provided, otherwise create per-project directory
        let project_dir = match &request.path {
            Some(p) if !p.is_empty() => {
                let dir = std::path::PathBuf::from(p);
                if !dir.exists() {
                    std::fs::create_dir_all(&dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
                dir
            }
            _ => {
                let dir = state.data_dir.join("projects").join(&request.name);
                std::fs::create_dir_all(&dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                let _ = std::fs::create_dir_all(dir.join("skills"));
                let _ = std::fs::create_dir_all(dir.join("plans"));
                let _ = std::fs::create_dir_all(dir.join("injections"));
                dir
            }
        };

        // Write config.toml into the project directory (only if it doesn't exist)
        let config_path = project_dir.join("config.toml");
        if !config_path.exists() {
            let _ = std::fs::write(&config_path, &forge_toml);
        }

        let entry = ProjectEntry {
            id: id.clone(),
            name: request.name,
            description: request.description,
            created_at: now.clone(),
            last_active: now,
            forge_toml,
            path: Some(project_dir.display().to_string()),
        };

        let mut projects = read_projects(&state.data_dir);
        projects.push(entry.clone());
        write_projects(&state.data_dir, &projects)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        tracing::info!("Created project: {} ({})", entry.name, entry.id);
        Ok((StatusCode::CREATED, Json(entry)))
    }

    pub async fn update(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
        Json(request): Json<UpdateProjectRequest>,
    ) -> Result<Json<ProjectEntry>, StatusCode> {
        let mut projects = read_projects(&state.data_dir);
        let idx = projects
            .iter()
            .position(|p| p.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;

        let entry = &mut projects[idx];
        if let Some(name) = request.name {
            entry.name = name;
        }
        if let Some(desc) = request.description {
            entry.description = desc;
        }
        if let Some(toml) = request.forge_toml {
            entry.forge_toml = toml;
        }
        entry.last_active = chrono::Utc::now().to_rfc3339();

        let cloned = entry.clone();

        write_projects(&state.data_dir, &projects)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(cloned))
    }

    pub async fn delete(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> StatusCode {
        let mut projects = read_projects(&state.data_dir);
        let before = projects.len();
        projects.retain(|p| p.id != id);
        if projects.len() == before {
            return StatusCode::NOT_FOUND;
        }
        let _ = write_projects(&state.data_dir, &projects);
        tracing::info!("Deleted project: {}", id);
        StatusCode::OK
    }

    /// Get parsed forge.toml config for a project.
    pub async fn get_config(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> Result<Json<ProjectConfigResponse>, StatusCode> {
        let projects = read_projects(&state.data_dir);
        let entry = projects
            .into_iter()
            .find(|p| p.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;

        let parsed = parse_forge_toml(&entry.forge_toml);
        Ok(Json(ProjectConfigResponse {
            raw: entry.forge_toml,
            roles: parsed.roles,
            providers: parsed.providers,
            goals: parsed.goals,
            limits: parsed.limits,
            project: ProjectInfo {
                name: entry.name.clone(),
                version: parsed.project_version,
            },
        }))
    }

    /// Update forge.toml for a project.
    pub async fn update_config(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
        Json(request): Json<UpdateConfigRequest>,
    ) -> Result<Json<ProjectConfigResponse>, StatusCode> {
        let mut projects = read_projects(&state.data_dir);
        let idx = projects
            .iter()
            .position(|p| p.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;

        let entry = &mut projects[idx];
        entry.forge_toml = request.config;
        entry.last_active = chrono::Utc::now().to_rfc3339();

        let forge_toml = entry.forge_toml.clone();
        let name = entry.name.clone();

        write_projects(&state.data_dir, &projects)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let parsed = parse_forge_toml(&forge_toml);
        Ok(Json(ProjectConfigResponse {
            raw: forge_toml,
            roles: parsed.roles,
            providers: parsed.providers,
            goals: parsed.goals,
            limits: parsed.limits,
            project: ProjectInfo {
                name,
                version: parsed.project_version,
            },
        }))
    }
}

// ─── Config parsing ───────────────────────────────────────────

fn parse_forge_toml(raw: &str) -> ParsedConfig {
    let parsed = match toml::from_str::<toml::Value>(raw) {
        Ok(v) => v,
        Err(_) => return ParsedConfig::default(),
    };

    let roles: HashMap<String, RoleDetail> = parsed
        .get("roles")
        .and_then(|v| v.as_table())
        .map(|table| {
            table
                .iter()
                .map(|(name, value)| {
                    let detail = RoleDetail {
                        model: value
                            .get("model")
                            .and_then(|v| v.as_str())
                            .unwrap_or("gpt-4o")
                            .to_string(),
                        temperature: value
                            .get("temperature")
                            .and_then(|v| v.as_float())
                            .unwrap_or(0.3),
                        max_tokens: value
                            .get("max_tokens")
                            .and_then(|v| v.as_integer())
                            .unwrap_or(4096) as u32,
                        system_prompt: value
                            .get("system_prompt")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        tools: value
                            .get("tools")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        description: value
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    };
                    (name.clone(), detail)
                })
                .collect()
        })
        .unwrap_or_default();

    let providers: HashMap<String, ProviderDetail> = parsed
        .get("providers")
        .and_then(|v| v.as_table())
        .map(|table| {
            table
                .iter()
                .map(|(name, value)| {
                    let detail = ProviderDetail {
                        base_url: value
                            .get("base_url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        api_key_ref: value
                            .get("api_key")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        default_model: value
                            .get("default_model")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    };
                    (name.clone(), detail)
                })
                .collect()
        })
        .unwrap_or_default();

    let goals: Vec<GoalDetail> = parsed
        .get("goals")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_table())
                .map(|table| GoalDetail {
                    name: table
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    agents: table
                        .get("agents")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    max_iterations: table
                        .get("max_iterations")
                        .and_then(|v| v.as_integer())
                        .unwrap_or(10) as u32,
                    gates: table
                        .get("gates")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    let limits = LimitsDetail {
        max_iterations_per_goal: parsed
            .get("limits")
            .and_then(|l| l.get("max_iterations_per_goal"))
            .and_then(|v| v.as_integer())
            .unwrap_or(50) as u32,
        max_iterations_per_phase: parsed
            .get("limits")
            .and_then(|l| l.get("max_iterations_per_phase"))
            .and_then(|v| v.as_integer())
            .unwrap_or(5) as u32,
        session_ttl_seconds: parsed
            .get("limits")
            .and_then(|l| l.get("session_ttl_seconds"))
            .and_then(|v| v.as_integer())
            .unwrap_or(3600) as u32,
        phase_timeout_seconds: parsed
            .get("limits")
            .and_then(|l| l.get("phase_timeout_seconds"))
            .and_then(|v| v.as_integer())
            .unwrap_or(300) as u32,
        max_tokens: parsed
            .get("limits")
            .and_then(|l| l.get("max_tokens"))
            .and_then(|v| v.as_integer())
            .map(|n| n as u64),
        max_cost_usd: parsed
            .get("limits")
            .and_then(|l| l.get("max_cost_usd"))
            .and_then(|v| v.as_float()),
    };

    let project_version = parsed
        .get("project")
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("0.1.0")
        .to_string();

    ParsedConfig {
        roles,
        providers,
        goals,
        limits,
        project_version,
    }
}

// ─── Config response types ────────────────────────────────────

#[derive(Serialize)]
pub struct ProjectConfigResponse {
    pub raw: String,
    pub roles: HashMap<String, RoleDetail>,
    pub providers: HashMap<String, ProviderDetail>,
    pub goals: Vec<GoalDetail>,
    pub limits: LimitsDetail,
    pub project: ProjectInfo,
}

#[derive(Serialize)]
pub struct ProjectInfo {
    pub name: String,
    pub version: String,
}

struct ParsedConfig {
    roles: HashMap<String, RoleDetail>,
    providers: HashMap<String, ProviderDetail>,
    goals: Vec<GoalDetail>,
    limits: LimitsDetail,
    project_version: String,
}

impl Default for ParsedConfig {
    fn default() -> Self {
        Self {
            roles: HashMap::new(),
            providers: HashMap::new(),
            goals: Vec::new(),
            limits: LimitsDetail::default(),
            project_version: "0.1.0".to_string(),
        }
    }
}

#[derive(Serialize, Clone)]
pub struct RoleDetail {
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u32,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub description: String,
}

#[derive(Serialize, Clone)]
pub struct ProviderDetail {
    pub base_url: String,
    pub api_key_ref: String,
    pub default_model: String,
}

#[derive(Serialize, Clone)]
pub struct GoalDetail {
    pub name: String,
    pub agents: Vec<String>,
    pub max_iterations: u32,
    pub gates: Vec<String>,
}

#[derive(Serialize, Clone, Default)]
pub struct LimitsDetail {
    pub max_iterations_per_goal: u32,
    pub max_iterations_per_phase: u32,
    pub session_ttl_seconds: u32,
    pub phase_timeout_seconds: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cost_usd: Option<f64>,
}

// ─── Response Types ───────────────────────────────────────────

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
}

#[derive(Serialize)]
pub struct TokenMetricsResponse {
    pub total_input: u64,
    pub total_output: u64,
    pub total_tokens: u64,
    pub by_provider: std::collections::HashMap<String, u64>,
    pub by_model: std::collections::HashMap<String, u64>,
}

#[derive(Serialize)]
pub struct ContextMetricsResponse {
    pub avg_pressure: f32,
    pub max_pressure: f32,
    pub total_compressions: u32,
    pub active_sessions: u32,
}

#[derive(Serialize)]
pub struct MetricsSummaryResponse {
    pub version: String,
    pub uptime_seconds: u64,
    pub active_sessions: u32,
    pub total_tokens: u64,
    pub avg_asi_score: f32,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: u16,
}

// ─── Session API ──────────────────────────────────────────────

pub mod sessions {
    use super::*;

    pub async fn list(State(state): State<Arc<AppState>>) -> Json<Vec<SessionEntry>> {
        // Collect sessions from the in-memory registry (same-process active sessions)
        let mut all: Vec<SessionEntry> = {
            let registry = state.session_registry.read().expect("RwLock not poisoned");
            registry.clone()
        };

        // Merge sessions from the event store (persisted sessions from other processes)
        if let Some(store) = &state.event_store
            && let Ok(session_ids) = store.list_aggregates("session").await
        {
            for sid in &session_ids {
                let sid_str = sid.to_string();
                // Skip if already in the in-memory registry
                if all.iter().any(|s| s.id == sid_str) {
                    continue;
                }
                if let Ok(Some(snap)) = store.get_snapshot(*sid).await {
                    let phase = snap.state["phase"].as_str().unwrap_or("unknown");
                    let status =
                        if phase == "Completed" || phase == "Failed" || phase == "Cancelled" {
                            phase.to_lowercase()
                        } else {
                            "running".to_string()
                        };
                    all.push(SessionEntry {
                        id: sid_str,
                        project: snap.state["project"]
                            .as_str()
                            .unwrap_or("default")
                            .to_string(),
                        goal: snap.state["goal"].as_str().unwrap_or("unknown").to_string(),
                        phase: phase.to_string(),
                        iteration: snap.state["iteration"].as_u64().unwrap_or(0) as u32,
                        status,
                        started_at: snap.updated_at.clone(),
                        completed_at: None,
                        tokens_used: snap.state["tokens_used"].as_u64().unwrap_or(0),
                        cost_usd: snap.state["cost_usd"].as_f64().unwrap_or(0.0),
                    });
                }
            }
        }

        Json(all)
    }

    pub async fn get_one(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> Result<Json<SessionEntry>, StatusCode> {
        // First check the in-memory registry
        {
            let sessions = state.session_registry.read().expect("RwLock not poisoned");
            if let Some(session) = sessions.iter().find(|s| s.id == id) {
                return Ok(Json(session.clone()));
            }
        }

        // Fall back to the event store
        if let Some(store) = &state.event_store
            && let Ok(sid) = id.parse::<uuid::Uuid>()
            && let Ok(Some(snap)) = store.get_snapshot(sid).await
        {
            let phase = snap.state["phase"].as_str().unwrap_or("unknown");
            let status = if phase == "Completed" || phase == "Failed" || phase == "Cancelled" {
                phase.to_lowercase()
            } else {
                "running".to_string()
            };
            return Ok(Json(SessionEntry {
                id,
                project: snap.state["project"]
                    .as_str()
                    .unwrap_or("default")
                    .to_string(),
                goal: snap.state["goal"].as_str().unwrap_or("unknown").to_string(),
                phase: phase.to_string(),
                iteration: snap.state["iteration"].as_u64().unwrap_or(0) as u32,
                status,
                started_at: snap.updated_at.clone(),
                completed_at: None,
                tokens_used: snap.state["tokens_used"].as_u64().unwrap_or(0),
                cost_usd: snap.state["cost_usd"].as_f64().unwrap_or(0.0),
            }));
        }

        Err(StatusCode::NOT_FOUND)
    }

    /// Spawn a goal run in a background tokio task.
    ///
    /// Creates a fresh CoreRuntime, wires it to the API's EventBus and vault,
    /// applies the project config, and spawns `run_goal` in a detached task.
    /// The session is tracked in `active_runs` for live monitoring + shutdown.
    pub async fn run_goal(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(project_id): axum::extract::Path<String>,
        Json(req): Json<RunGoalRequest>,
    ) -> Result<Json<RunGoalResponse>, (StatusCode, String)> {
        // Find the project to get its config path
        let projects = read_projects(&state.data_dir);
        let project = projects
            .iter()
            .find(|p| p.id == project_id)
            .ok_or((StatusCode::NOT_FOUND, "Project not found".to_string()))?;

        let project_name = project.name.clone();

        // Write the project's forge.toml to a temp file for load_forge_config
        let config_path = write_temp_config(&project.forge_toml, &project_name).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write config: {}", e),
            )
        })?;

        // Create the runtime — use built-in skills if requested, otherwise load custom skills
        let skill_ids: Vec<&str> = req.skills.iter().map(|s| s.as_str()).collect();
        let mut runtime = crate::CoreRuntime::new()
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create runtime: {}", e),
                )
            })?
            .with_default_memory()
            .with_state_file()
            .with_project_name(project_name.clone());

        // Load skills: built-in (if any IDs specified) or custom (from skills/ dir)
        if !skill_ids.is_empty() {
            runtime = runtime.with_builtin_skills(&skill_ids);
        } else {
            runtime = runtime.with_skills();
        }

        // Wire the runtime's EventBus to the API server's EventBus (for live dashboard events)
        runtime.bus = state.bus.clone();

        // Apply completion criterion
        if let Some(ref cmd) = req.until {
            runtime = runtime
                .with_completion(crate::CompletionCriterion::from_until_command(cmd.clone()));
        } else if let Some(ref comp) = req.completion
            && let Some(criterion) = crate::CompletionCriterion::from_string(comp)
        {
            runtime = runtime.with_completion(criterion);
        }

        // Apply budget caps
        if req.max_tokens.is_some() || req.max_cost_usd.is_some() {
            runtime.loop_controller.limits.max_tokens = req.max_tokens;
            runtime.loop_controller.limits.max_cost_usd = req.max_cost_usd;
        }

        // Get the shutdown handle before moving the runtime into the task
        let shutdown_handle = runtime.shutdown_handle();
        let session_id = uuid::Uuid::new_v4();
        let session_id_str = session_id.to_string();

        // Live token/cost counters for this session
        let tokens_used = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let cost_usd = std::sync::Arc::new(std::sync::RwLock::new(0.0f64));

        // Register the active run
        {
            let mut runs = state.active_runs.write().expect("RwLock not poisoned");
            runs.insert(
                session_id_str.clone(),
                ActiveRun {
                    shutdown: shutdown_handle.clone(),
                    tokens_used: tokens_used.clone(),
                    cost_usd: cost_usd.clone(),
                    project_id: project_id.clone(),
                    goal: req.goal.clone(),
                },
            );
        }

        // Register in the session registry
        {
            let mut registry = state.session_registry.write().expect("RwLock not poisoned");
            registry.push(SessionEntry {
                id: session_id_str.clone(),
                project: project_name.clone(),
                goal: req.goal.clone(),
                phase: "Planning".to_string(),
                iteration: 0,
                status: "running".to_string(),
                started_at: chrono::Utc::now().to_rfc3339(),
                completed_at: None,
                tokens_used: 0,
                cost_usd: 0.0,
            });
        }

        // Subscribe to EventBus to track when this session stops being active.
        // The live token/cost counters are read from the runtime's loop controller
        // when the session completes. This listener just detects completion.
        let bus_for_listener = state.bus.clone();
        let sid_for_listener = session_id_str.clone();
        let active_runs_for_listener = state.active_runs.clone();
        tokio::spawn(async move {
            let mut rx = bus_for_listener.subscribe();
            loop {
                match rx.recv().await {
                    Ok(_event) => {
                        // Check if this session is still active; if not, stop listening.
                        let still_active = {
                            let runs = active_runs_for_listener
                                .read()
                                .expect("RwLock not poisoned");
                            runs.contains_key(&sid_for_listener)
                        };
                        if !still_active {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        // Spawn the actual goal run
        let goal = req.goal.clone();
        let goal_for_result = goal.clone();
        let sid_for_task = session_id_str.clone();
        let sid_for_registry = session_id_str.clone();
        let registry = state.session_registry.clone();
        let active_runs = state.active_runs.clone();
        let config_path_for_task = config_path.clone();
        let vault = state.vault.clone();
        let create_worktree = req.worktree;

        tokio::spawn(async move {
            // Create worktree if requested (isolated working directory)
            let worktree_path = if create_worktree {
                create_worktree_blocking(&sid_for_task)
            } else {
                None
            };

            let vault_ref: Option<&praxis_vault::VaultService> = Some(&vault);
            let result = runtime
                .run_goal(&goal, Some(&config_path_for_task), vault_ref)
                .await;

            // Clean up temp config
            let _ = std::fs::remove_file(&config_path_for_task);

            // Clean up worktree if created
            if let Some(ref wt_path) = worktree_path {
                remove_worktree_blocking(wt_path);
            }

            // Update session registry
            {
                let mut reg = registry.write().expect("RwLock not poisoned");
                if let Some(entry) = reg.iter_mut().find(|s| s.id == sid_for_registry) {
                    entry.status = match &result {
                        Ok(r) if r.passed => "completed".to_string(),
                        Ok(_) => "failed".to_string(),
                        Err(_) => "failed".to_string(),
                    };
                    entry.completed_at = Some(chrono::Utc::now().to_rfc3339());
                    // Pull final token/cost from the runtime's loop controller
                    entry.tokens_used = runtime.loop_controller.tokens_used;
                    entry.cost_usd = runtime.cost_usd_for_session();
                }
            }

            // Remove from active runs
            {
                let mut runs = active_runs.write().expect("RwLock not poisoned");
                runs.remove(&sid_for_task);
            }

            match &result {
                Ok(r) => tracing::info!("Session {} completed: passed={}", sid_for_task, r.passed),
                Err(e) => tracing::error!("Session {} failed: {}", sid_for_task, e),
            }

            let _ = runtime.shutdown().await;
        });

        Ok(Json(RunGoalResponse {
            session_id: session_id_str,
            project_id,
            goal: goal_for_result,
            status: "running".to_string(),
        }))
    }

    /// Plan mode: run only the Planning + Designing phases, return the plan.
    ///
    /// This does NOT execute the full loop — it produces a plan that the user
    /// can review and then execute via `run_goal` with the plan as context.
    pub async fn plan_goal(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(project_id): axum::extract::Path<String>,
        Json(req): Json<RunGoalRequest>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        // For now, plan mode returns the goal with a "plan" phase marker.
        // Full plan mode (Planning + Designing only) requires a phase-limited loop,
        // which is a larger change. This endpoint marks the intent and returns
        // a plan placeholder that the frontend can display.
        let projects = read_projects(&state.data_dir);
        let project = projects
            .iter()
            .find(|p| p.id == project_id)
            .ok_or((StatusCode::NOT_FOUND, "Project not found".to_string()))?;

        let plan_id = uuid::Uuid::new_v4().to_string();
        let plan_content = format!(
            "# Plan: {}\n\n\
             ## Goal\n{}\n\n\
             ## Project\n{}\n\n\
             ## Status\nPlanning phase — review the goal above, then click Execute to run the full agent loop.\n",
            req.goal, req.goal, project.name
        );

        // Save the plan to the project's plans directory
        let plans_dir = state
            .data_dir
            .join("projects")
            .join(&project.name)
            .join("plans");
        let _ = std::fs::create_dir_all(&plans_dir);
        let plan_path = plans_dir.join(format!("{}.md", plan_id));
        let _ = std::fs::write(&plan_path, &plan_content);

        Ok(Json(serde_json::json!({
            "plan_id": plan_id,
            "project_id": project_id,
            "goal": req.goal,
            "plan": plan_content,
            "plan_path": plan_path.display().to_string(),
            "status": "planned",
        })))
    }

    /// Get live state for a session: phase, iteration, tokens, cost, STATE.md content.
    pub async fn get_state(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> Result<Json<SessionStateResponse>, StatusCode> {
        // Check active runs for live data
        let (tokens_used, cost_usd, status) = {
            let runs = state.active_runs.read().expect("RwLock not poisoned");
            if let Some(run) = runs.get(&id) {
                (
                    run.tokens_used.load(std::sync::atomic::Ordering::Relaxed),
                    *run.cost_usd.read().expect("RwLock not poisoned"),
                    "running".to_string(),
                )
            } else {
                // Not active — check the session registry for final data
                let registry = state.session_registry.read().expect("RwLock not poisoned");
                if let Some(entry) = registry.iter().find(|s| s.id == id) {
                    (entry.tokens_used, entry.cost_usd, entry.status.clone())
                } else {
                    (0, 0.0, "unknown".to_string())
                }
            }
        };

        // Get phase + iteration from the session registry or event store
        let (phase, iteration) = {
            let registry = state.session_registry.read().expect("RwLock not poisoned");
            if let Some(entry) = registry.iter().find(|s| s.id == id) {
                (entry.phase.clone(), entry.iteration)
            } else {
                ("unknown".to_string(), 0)
            }
        };

        // Read STATE.md if it exists
        let state_file = std::fs::read_to_string("STATE.md").ok();

        Ok(Json(SessionStateResponse {
            session_id: id,
            phase,
            iteration,
            tokens_used,
            cost_usd,
            status,
            state_file,
        }))
    }

    pub async fn stop(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> Result<Json<serde_json::Value>, StatusCode> {
        // If this is an active run, set the shutdown flag for graceful stop
        let was_active = {
            let runs = state.active_runs.read().expect("RwLock not poisoned");
            if let Some(run) = runs.get(&id) {
                run.shutdown
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                true
            } else {
                false
            }
        };

        // Also write a stop-injection (for cross-process sessions)
        let injections_dir = state.data_dir.join("injections");
        let _ = std::fs::create_dir_all(&injections_dir);

        let msg = serde_json::json!({
            "target_agent": "all",
            "message_type": "stop",
            "content": "Stop the current session immediately.",
            "created_at": chrono::Utc::now().to_rfc3339(),
        });

        let filename = format!(
            "{}_stop_{}.json",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            id
        );
        let path = injections_dir.join(&filename);
        let content = serde_json::to_string_pretty(&msg).unwrap_or_default();
        let _ = std::fs::write(&path, &content);

        // Mark session as completed in registry
        if let Ok(mut guard) = state.session_registry.write()
            && let Some(session) = guard.iter_mut().find(|s| s.id == id)
        {
            session.status = "stopped".to_string();
            session.completed_at = Some(chrono::Utc::now().to_rfc3339());
        }

        Ok(Json(serde_json::json!({
            "status": "stopped",
            "session_id": id,
            "was_active_run": was_active,
        })))
    }

    /// Rollback a session's file changes to the pre-session git baseline.
    ///
    /// Restores the working tree to the HEAD commit + uncommitted diff captured
    /// at `run_goal` start. Returns 404 if no baseline exists for the session.
    pub async fn rollback(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> Result<Json<serde_json::Value>, StatusCode> {
        let Some(store) = &state.event_store else {
            return Err(StatusCode::NOT_FOUND);
        };

        let sid = uuid::Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

        let working_dir = std::env::current_dir().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match crate::rollback::restore_baseline(store, sid, &working_dir) {
            Ok(message) => Ok(Json(serde_json::json!({
                "session_id": id,
                "status": "rolled_back",
                "message": message,
            }))),
            Err(e) => {
                tracing::error!("Rollback failed for session {}: {}", id, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    /// Get the diff between a session's baseline commit and the current HEAD.
    ///
    /// Returns the unified diff text. Empty string if no baseline or no changes.
    pub async fn diff(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> Result<Json<serde_json::Value>, StatusCode> {
        let Some(store) = &state.event_store else {
            return Err(StatusCode::NOT_FOUND);
        };

        let sid = uuid::Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;

        let working_dir = std::env::current_dir().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match crate::rollback::diff_from_baseline(store, sid, &working_dir) {
            Ok(diff_text) => Ok(Json(serde_json::json!({
                "session_id": id,
                "diff": diff_text,
            }))),
            Err(e) => {
                tracing::error!("Diff failed for session {}: {}", id, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    #[derive(Serialize)]
    pub struct AgentSummary {
        pub name: String,
        pub role: String,
        pub model: String,
        pub tools: Vec<String>,
        pub status: String,
    }

    /// Parse agents from the most recent project's forge.toml.
    /// Falls back to a default agent list if no project is found.
    fn agents_from_projects(data_dir: &std::path::Path) -> Vec<AgentSummary> {
        let projects = read_projects(data_dir);
        let Some(latest) = projects.last() else {
            return Vec::new();
        };
        let parsed = parse_forge_toml(&latest.forge_toml);
        parsed
            .roles
            .iter()
            .map(|(name, detail)| AgentSummary {
                name: name.clone(),
                role: name.clone(),
                model: detail.model.clone(),
                tools: detail.tools.clone(),
                status: "idle".into(),
            })
            .collect()
    }

    pub async fn list_agents(State(state): State<Arc<AppState>>) -> Json<Vec<AgentSummary>> {
        let agents = agents_from_projects(&state.data_dir);
        Json(agents)
    }

    /// List all persisted events for a session from the event store.
    pub async fn list_events(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
        let Some(store) = &state.event_store else {
            return Err(StatusCode::NOT_FOUND);
        };
        let Ok(sid) = id.parse::<uuid::Uuid>() else {
            return Err(StatusCode::BAD_REQUEST);
        };
        match store.read_events(sid, None).await {
            Ok(events) => {
                let values: Vec<serde_json::Value> = events
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "id": e.id,
                            "event_type": e.event_type,
                            "payload": e.payload,
                            "version": e.version,
                            "created_at": e.created_at,
                        })
                    })
                    .collect();
                Ok(Json(values))
            }
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }

    /// Get the latest checkpoint (snapshot) for a session.
    pub async fn get_checkpoint(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> Result<Json<serde_json::Value>, StatusCode> {
        let Some(store) = &state.event_store else {
            return Err(StatusCode::NOT_FOUND);
        };
        let Ok(sid) = id.parse::<uuid::Uuid>() else {
            return Err(StatusCode::BAD_REQUEST);
        };
        match store.get_snapshot(sid).await {
            Ok(Some(snap)) => Ok(Json(serde_json::json!({
                "aggregate_id": snap.aggregate_id,
                "aggregate_type": snap.aggregate_type,
                "state": snap.state,
                "version": snap.version,
                "updated_at": snap.updated_at,
            }))),
            Ok(None) => Err(StatusCode::NOT_FOUND),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

// ─── Debug API ────────────────────────────────────────────────

pub mod debug {
    use super::*;

    #[derive(Serialize)]
    pub struct MemoryStats {
        pub total_sessions: usize,
        pub total_events: usize,
        pub total_snapshots: usize,
        pub total_projects: usize,
        pub event_store_path: Option<String>,
    }

    /// Memory and persistence statistics for debugging.
    pub async fn memory_stats(
        State(state): State<Arc<AppState>>,
    ) -> Result<Json<MemoryStats>, StatusCode> {
        let projects = read_projects(&state.data_dir);
        let total_projects = projects.len();

        let (total_sessions, total_snapshots) = if let Some(store) = &state.event_store {
            let sessions = store.list_aggregates("session").await.unwrap_or_default();
            let session_count = sessions.len();
            let mut snapshot_count = 0;
            for sid in &sessions {
                if let Ok(Some(_)) = store.get_snapshot(*sid).await {
                    snapshot_count += 1;
                }
            }
            (session_count, snapshot_count)
        } else {
            (0, 0)
        };

        let event_store_path = state
            .event_store
            .as_ref()
            .map(|_| state.data_dir.join("state.db").display().to_string());

        Ok(Json(MemoryStats {
            total_sessions,
            total_events: 0, // would need a count_events method
            total_snapshots,
            total_projects,
            event_store_path,
        }))
    }

    /// Debug overview of all sessions with their checkpoint state.
    pub async fn debug_sessions(
        State(state): State<Arc<AppState>>,
    ) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
        let Some(store) = &state.event_store else {
            return Ok(Json(Vec::new()));
        };
        let session_ids = store.list_aggregates("session").await.unwrap_or_default();
        let mut result = Vec::new();
        for sid in &session_ids {
            if let Ok(Some(snap)) = store.get_snapshot(*sid).await {
                result.push(serde_json::json!({
                    "session_id": sid,
                    "goal": snap.state.get("goal").and_then(|v| v.as_str()).unwrap_or("?"),
                    "project": snap.state.get("project").and_then(|v| v.as_str()).unwrap_or("default"),
                    "phase": snap.state.get("phase").and_then(|v| v.as_str()).unwrap_or("?"),
                    "iteration": snap.state.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0),
                    "context_pressure": snap.state.get("context_pressure").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    "updated_at": snap.updated_at,
                }));
            }
        }
        Ok(Json(result))
    }
}

// ─── Inject Request ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct InjectRequest {
    pub target_agent: String,
    pub message_type: String,
    pub content: String,
}

// ─── Route Handlers ───────────────────────────────────────────

#[allow(clippy::module_inception)]
pub mod routes {
    use super::*;

    pub async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
        let uptime = chrono::Utc::now()
            .signed_duration_since(state.started_at)
            .num_seconds() as u64;
        Json(HealthResponse {
            status: "ok".to_string(),
            version: state.version.clone(),
            uptime_seconds: uptime,
        })
    }

    pub async fn token_metrics(State(state): State<Arc<AppState>>) -> Json<TokenMetricsResponse> {
        let counters = state.token_counters.read().expect("RwLock not poisoned");
        Json(TokenMetricsResponse {
            total_input: counters.total_input,
            total_output: counters.total_output,
            total_tokens: counters.total_input + counters.total_output,
            by_provider: counters.by_provider.clone(),
            by_model: counters.by_model.clone(),
        })
    }

    pub async fn context_metrics(
        State(state): State<Arc<AppState>>,
    ) -> Json<ContextMetricsResponse> {
        let session_count = state
            .session_registry
            .read()
            .expect("RwLock not poisoned")
            .len() as u32;

        // Read context pressure from the most recent checkpoint
        let (avg_pressure, max_pressure) = if let Some(store) = &state.event_store {
            if let Ok(session_ids) = store.list_aggregates("session").await {
                let mut pressures: Vec<f64> = Vec::new();
                for sid in &session_ids {
                    if let Ok(Some(snap)) = store.get_snapshot(*sid).await
                        && let Some(p) = snap.state.get("context_pressure").and_then(|v| v.as_f64())
                    {
                        pressures.push(p);
                    }
                }
                if pressures.is_empty() {
                    (0.0, 0.0)
                } else {
                    let avg = pressures.iter().sum::<f64>() / pressures.len() as f64;
                    let max = pressures.iter().cloned().fold(0.0_f64, f64::max);
                    (avg as f32, max as f32)
                }
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

        Json(ContextMetricsResponse {
            avg_pressure,
            max_pressure,
            total_compressions: 0,
            active_sessions: session_count,
        })
    }

    /// List all available built-in skills.
    pub async fn list_skills() -> Json<Vec<SkillInfo>> {
        let skills = crate::skills::builtin_skills();
        let result = skills
            .iter()
            .map(|s| SkillInfo {
                id: s.id.to_string(),
                name: s.name.to_string(),
                description: s.description.to_string(),
            })
            .collect();
        Json(result)
    }

    pub async fn inject(
        State(state): State<Arc<AppState>>,
        Json(req): Json<InjectRequest>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        // Write injection to {data_dir}/injections/ as a JSON file
        let injections_dir = state.data_dir.join("injections");
        std::fs::create_dir_all(&injections_dir).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create injections dir: {}", e),
            )
        })?;

        let msg = serde_json::json!({
            "target_agent": req.target_agent,
            "message_type": req.message_type,
            "content": req.content,
            "created_at": chrono::Utc::now().to_rfc3339(),
        });

        let filename = format!(
            "{}_{}.json",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            req.target_agent
        );
        let path = injections_dir.join(&filename);
        let content = serde_json::to_string_pretty(&msg).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to serialize: {}", e),
            )
        })?;
        std::fs::write(&path, &content).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write injection: {}", e),
            )
        })?;

        tracing::info!("Injection written to: {:?}", path);

        Ok(Json(serde_json::json!({
            "status": "injected",
            "file": filename,
            "target_agent": req.target_agent,
            "message_type": req.message_type,
        })))
    }

    pub async fn metrics_summary(
        State(state): State<Arc<AppState>>,
    ) -> Json<MetricsSummaryResponse> {
        let uptime = chrono::Utc::now()
            .signed_duration_since(state.started_at)
            .num_seconds() as u64;
        let session_count = state
            .session_registry
            .read()
            .expect("RwLock not poisoned")
            .len() as u32;
        let total_tokens = {
            let counters = state.token_counters.read().expect("RwLock not poisoned");
            counters.total_input + counters.total_output
        };
        Json(MetricsSummaryResponse {
            version: state.version.clone(),
            uptime_seconds: uptime,
            active_sessions: session_count,
            total_tokens,
            avg_asi_score: 100.0,
        })
    }
}

// ─── Agent CRUD ────────────────────────────────────────────────

/// Agent definition as returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinitionResponse {
    pub name: String,
    pub description: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub tools: Vec<String>,
    pub max_turns: u32,
    pub max_depth: u8,
    pub can_spawn: Vec<String>,
    pub system_prompt: String,
    pub scope: String,
}

/// Request to create or update an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub description: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Option<Vec<String>>,
    pub max_turns: Option<u32>,
    pub max_depth: Option<u8>,
    pub can_spawn: Option<Vec<String>>,
    pub system_prompt: String,
    /// "project" or "global" (default: "project").
    #[serde(default = "default_scope")]
    pub scope: String,
}

fn default_scope() -> String {
    "project".to_string()
}

/// Resolve the project agents directory from the latest project's path.
/// Falls back to `data_dir/agents/` if no project has a path.
fn resolve_project_agents_dir(data_dir: &std::path::Path) -> std::path::PathBuf {
    let projects = read_projects(data_dir);
    if let Some(latest) = projects.last()
        && let Some(path) = &latest.path
    {
        return std::path::PathBuf::from(path).join("agents");
    }
    data_dir.join("agents")
}

/// Build an AgentRegistry from all scopes (builtin + global + latest project).
fn build_registry(state: &AppState) -> crate::agents::AgentRegistry {
    let global_dir = crate::agents::AgentRegistry::global_dir();
    let project_dir = resolve_project_agents_dir(&state.data_dir);
    crate::agents::AgentRegistry::load(Some(&global_dir), Some(&project_dir))
}

/// Serialize an AgentDefinition to Markdown+YAML format for disk persistence.
///
/// Uses `serde_yaml_neo::to_string` for the frontmatter to ensure proper escaping
/// of all YAML special characters (newlines, colons, booleans, numbers, etc.).
/// The Markdown body (system prompt) is appended after the closing `---`.
fn serialize_agent_md(def: &crate::agents::AgentDefinition) -> String {
    // serde_yaml_neo::to_string produces a YAML document with a leading "---\n"
    // document marker and ends with "\n". We use it directly for the frontmatter.
    let yaml = serde_yaml_neo::to_string(&def.frontmatter).unwrap_or_else(|e| {
        tracing::error!("Failed to serialize agent frontmatter: {}", e);
        String::new()
    });

    // serde_yaml_neo outputs "---\n<fields>\n" — we need to add the closing "---"
    // and then the Markdown body.
    // The yaml string already has "---\n" at the start and ends with "\n".
    // We strip the leading "---\n" (serde_yaml_neo adds it as a document marker)
    // and reformat to our frontmatter style.
    let yaml_body = yaml.strip_prefix("---\n").unwrap_or(&yaml);
    format!("---\n{yaml_body}---\n\n{}", def.system_prompt)
}

pub mod agent_crud {
    use super::*;

    /// List all agents (builtin + global + project).
    pub async fn list_agents(
        State(state): State<Arc<AppState>>,
    ) -> Json<Vec<AgentDefinitionResponse>> {
        let registry = build_registry(&state);
        let agents: Vec<AgentDefinitionResponse> = registry
            .list()
            .iter()
            .map(|scoped| AgentDefinitionResponse {
                name: scoped.definition.name().to_string(),
                description: scoped.definition.frontmatter.description.clone(),
                model: scoped.definition.model().to_string(),
                temperature: scoped.definition.frontmatter.temperature,
                max_tokens: scoped.definition.frontmatter.max_tokens,
                tools: scoped.definition.tools().to_vec(),
                max_turns: scoped.definition.max_turns(),
                max_depth: scoped.definition.max_depth(),
                can_spawn: scoped.definition.can_spawn().to_vec(),
                system_prompt: scoped.definition.system_prompt().to_string(),
                scope: scoped.scope.as_str().to_string(),
            })
            .collect();
        Json(agents)
    }

    /// Get a single agent by name.
    pub async fn get_agent(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(name): axum::extract::Path<String>,
    ) -> Result<Json<AgentDefinitionResponse>, (StatusCode, String)> {
        let registry = build_registry(&state);
        let scoped = registry
            .resolve(&name)
            .ok_or((StatusCode::NOT_FOUND, format!("Agent '{}' not found", name)))?;
        Ok(Json(AgentDefinitionResponse {
            name: scoped.definition.name().to_string(),
            description: scoped.definition.frontmatter.description.clone(),
            model: scoped.definition.model().to_string(),
            temperature: scoped.definition.frontmatter.temperature,
            max_tokens: scoped.definition.frontmatter.max_tokens,
            tools: scoped.definition.tools().to_vec(),
            max_turns: scoped.definition.max_turns(),
            max_depth: scoped.definition.max_depth(),
            can_spawn: scoped.definition.can_spawn().to_vec(),
            system_prompt: scoped.definition.system_prompt().to_string(),
            scope: scoped.scope.as_str().to_string(),
        }))
    }

    /// Create a new agent (writes .md to project or global scope).
    pub async fn create_agent(
        State(state): State<Arc<AppState>>,
        Json(req): Json<CreateAgentRequest>,
    ) -> Result<(StatusCode, Json<AgentDefinitionResponse>), (StatusCode, String)> {
        // Validate name
        if req.name.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                "Agent name is required".to_string(),
            ));
        }
        if req.system_prompt.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                "System prompt is required".to_string(),
            ));
        }
        // Validate scope
        if req.scope != "project" && req.scope != "global" {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Scope must be 'project' or 'global', got '{}'", req.scope),
            ));
        }

        // Determine target directory
        let dir = if req.scope == "global" {
            crate::agents::AgentRegistry::global_dir()
        } else {
            resolve_project_agents_dir(&state.data_dir)
        };

        // Create directory if needed
        std::fs::create_dir_all(&dir).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create agents dir: {}", e),
            )
        })?;

        // Atomically create the file — fails if it already exists (prevents TOCTOU race)
        let file_path = dir.join(format!("{}.md", req.name));
        use std::io::Write;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    (
                        StatusCode::CONFLICT,
                        format!("Agent '{}' already exists in {} scope", req.name, req.scope),
                    )
                } else {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to create agent file: {}", e),
                    )
                }
            })?;

        // Build the AgentDefinition
        let definition = crate::agents::AgentDefinition {
            frontmatter: crate::agents::AgentFrontmatter {
                name: req.name.clone(),
                description: req.description.unwrap_or_default(),
                model: req.model.unwrap_or_else(|| "gpt-5".to_string()),
                temperature: req.temperature.unwrap_or(0.3),
                max_tokens: req.max_tokens.unwrap_or(4096),
                tools: req.tools.unwrap_or_default(),
                max_turns: req.max_turns.unwrap_or(25),
                max_depth: req.max_depth.unwrap_or(0),
                can_spawn: req.can_spawn.unwrap_or_default(),
            },
            system_prompt: req.system_prompt,
        };

        // Serialize and write
        let content = serialize_agent_md(&definition);
        let mut file = file;
        file.write_all(content.as_bytes()).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write agent file: {}", e),
            )
        })?;

        tracing::info!(
            "Created agent '{}' in {} scope ({})",
            req.name,
            req.scope,
            file_path.display()
        );

        Ok((
            StatusCode::CREATED,
            Json(AgentDefinitionResponse {
                name: definition.name().to_string(),
                description: definition.frontmatter.description.clone(),
                model: definition.model().to_string(),
                temperature: definition.frontmatter.temperature,
                max_tokens: definition.frontmatter.max_tokens,
                tools: definition.tools().to_vec(),
                max_turns: definition.max_turns(),
                max_depth: definition.max_depth(),
                can_spawn: definition.can_spawn().to_vec(),
                system_prompt: definition.system_prompt().to_string(),
                scope: req.scope,
            }),
        ))
    }

    /// Update an existing agent (overwrites the .md file).
    pub async fn update_agent(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(name): axum::extract::Path<String>,
        Json(req): Json<CreateAgentRequest>,
    ) -> Result<Json<AgentDefinitionResponse>, (StatusCode, String)> {
        // Validate scope
        if req.scope != "project" && req.scope != "global" {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Scope must be 'project' or 'global', got '{}'", req.scope),
            ));
        }

        // Determine target directory
        let dir = if req.scope == "global" {
            crate::agents::AgentRegistry::global_dir()
        } else {
            resolve_project_agents_dir(&state.data_dir)
        };

        let file_path = dir.join(format!("{}.md", name));

        // If the agent is built-in only (no file in project/global), we need to
        // create the file (override). If it exists, we overwrite.
        std::fs::create_dir_all(&dir).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create agents dir: {}", e),
            )
        })?;

        // Build the updated definition
        let definition = crate::agents::AgentDefinition {
            frontmatter: crate::agents::AgentFrontmatter {
                name: name.clone(),
                description: req.description.unwrap_or_default(),
                model: req.model.unwrap_or_else(|| "gpt-5".to_string()),
                temperature: req.temperature.unwrap_or(0.3),
                max_tokens: req.max_tokens.unwrap_or(4096),
                tools: req.tools.unwrap_or_default(),
                max_turns: req.max_turns.unwrap_or(25),
                max_depth: req.max_depth.unwrap_or(0),
                can_spawn: req.can_spawn.unwrap_or_default(),
            },
            system_prompt: req.system_prompt,
        };

        let content = serialize_agent_md(&definition);
        std::fs::write(&file_path, &content).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write agent file: {}", e),
            )
        })?;

        tracing::info!(
            "Updated agent '{}' in {} scope ({})",
            name,
            req.scope,
            file_path.display()
        );

        Ok(Json(AgentDefinitionResponse {
            name: definition.name().to_string(),
            description: definition.frontmatter.description.clone(),
            model: definition.model().to_string(),
            temperature: definition.frontmatter.temperature,
            max_tokens: definition.frontmatter.max_tokens,
            tools: definition.tools().to_vec(),
            max_turns: definition.max_turns(),
            max_depth: definition.max_depth(),
            can_spawn: definition.can_spawn().to_vec(),
            system_prompt: definition.system_prompt().to_string(),
            scope: req.scope,
        }))
    }

    /// Delete an agent (removes the .md file). Only project/global scope.
    pub async fn delete_agent(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(name): axum::extract::Path<String>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        // Try project scope first, then global
        let project_dir = resolve_project_agents_dir(&state.data_dir);
        let global_dir = crate::agents::AgentRegistry::global_dir();

        let project_path = project_dir.join(format!("{}.md", name));
        let global_path = global_dir.join(format!("{}.md", name));

        let (deleted_path, scope) = if project_path.exists() {
            std::fs::remove_file(&project_path).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to delete: {}", e),
                )
            })?;
            (project_path, "project")
        } else if global_path.exists() {
            std::fs::remove_file(&global_path).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to delete: {}", e),
                )
            })?;
            (global_path, "global")
        } else {
            return Err((
                StatusCode::NOT_FOUND,
                format!(
                    "Agent '{}' not found in project or global scope (built-in agents cannot be deleted)",
                    name
                ),
            ));
        };

        tracing::info!(
            "Deleted agent '{}' from {} scope ({})",
            name,
            scope,
            deleted_path.display()
        );

        Ok(Json(serde_json::json!({
            "deleted": name,
            "scope": scope,
        })))
    }
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response() {
        let response = HealthResponse {
            status: "ok".to_string(),
            version: "0.1.0".to_string(),
            uptime_seconds: 100,
        };
        assert_eq!(response.status, "ok");
    }

    #[test]
    fn test_project_entry_roundtrip() {
        let entry = ProjectEntry {
            id: "test-id".to_string(),
            name: "test".to_string(),
            description: "desc".to_string(),
            created_at: "now".to_string(),
            last_active: "now".to_string(),
            forge_toml: "[project]\nname = \"test\"".to_string(),
            path: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: ProjectEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "test-id");
        assert_eq!(back.name, "test");
        assert!(back.forge_toml.contains("name = \"test\""));
    }

    #[test]
    fn test_default_forge_toml_template() {
        let filled = DEFAULT_FORGE_TOML.replace("{name}", "my-project");
        assert!(filled.contains("name = \"my-project\""));
    }

    #[test]
    fn test_parse_forge_toml() {
        let raw = DEFAULT_FORGE_TOML.replace("{name}", "test");
        let result = parse_forge_toml(&raw);
        assert!(result.roles.contains_key("architect"));
        assert!(result.roles.contains_key("coder"));
        assert!(result.providers.contains_key("nan"));
        assert!(result.goals.len() >= 1);
        assert_eq!(result.limits.max_iterations_per_goal, 50);
    }

    #[test]
    fn test_parse_empty_forge_toml() {
        let result = parse_forge_toml("");
        assert!(result.roles.is_empty());
        assert!(result.providers.is_empty());
        assert!(result.goals.is_empty());
    }

    #[test]
    fn test_session_entry_has_token_cost_fields() {
        let entry = SessionEntry {
            id: "test".to_string(),
            project: "proj".to_string(),
            goal: "goal".to_string(),
            phase: "Planning".to_string(),
            iteration: 1,
            status: "running".to_string(),
            started_at: "now".to_string(),
            completed_at: None,
            tokens_used: 5000,
            cost_usd: 0.15,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"tokens_used\":5000"));
        assert!(json.contains("\"cost_usd\":0.15"));
    }

    #[test]
    fn test_run_goal_request_deserializes() {
        let json = r#"{"goal":"test goal","until":"cargo test","max_tokens":1000000}"#;
        let req: RunGoalRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.goal, "test goal");
        assert_eq!(req.until.as_deref(), Some("cargo test"));
        assert_eq!(req.max_tokens, Some(1000000));
    }

    #[test]
    fn test_run_goal_request_minimal() {
        let json = r#"{"goal":"simple goal"}"#;
        let req: RunGoalRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.goal, "simple goal");
        assert!(req.completion.is_none());
        assert!(req.until.is_none());
        assert!(req.max_tokens.is_none());
    }

    #[test]
    fn test_project_entry_with_path() {
        let entry = ProjectEntry {
            id: "test".to_string(),
            name: "proj".to_string(),
            description: "".to_string(),
            created_at: "now".to_string(),
            last_active: "now".to_string(),
            forge_toml: "".to_string(),
            path: Some("/home/user/.config/praxis/projects/proj".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: ProjectEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.path.as_deref(),
            Some("/home/user/.config/praxis/projects/proj")
        );
    }

    #[test]
    fn test_project_entry_legacy_no_path() {
        // Legacy projects don't have a path field — should deserialize with None
        let json = r#"{"id":"x","name":"old","description":"","created_at":"now","last_active":"now","forge_toml":""}"#;
        let entry: ProjectEntry = serde_json::from_str(json).unwrap();
        assert!(entry.path.is_none());
    }

    #[test]
    fn test_run_goal_request_with_skills_and_worktree() {
        let json = r#"{"goal":"test","skills":["rust-best-practices","security"],"worktree":true}"#;
        let req: RunGoalRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.goal, "test");
        assert_eq!(req.skills, vec!["rust-best-practices", "security"]);
        assert!(req.worktree);
    }

    #[test]
    fn test_run_goal_request_without_skills() {
        let json = r#"{"goal":"simple"}"#;
        let req: RunGoalRequest = serde_json::from_str(json).unwrap();
        assert!(req.skills.is_empty());
        assert!(!req.worktree);
    }

    #[test]
    fn test_skill_info_serializes() {
        let info = SkillInfo {
            id: "rust-best-practices".to_string(),
            name: "Rust Best Practices".to_string(),
            description: "Idiomatic Rust patterns".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"id\":\"rust-best-practices\""));
        assert!(json.contains("\"name\":\"Rust Best Practices\""));
    }

    #[test]
    fn test_create_agent_request_deserializes() {
        let json = r#"{"name":"custom-agent","system_prompt":"You are a custom agent.","scope":"project"}"#;
        let req: CreateAgentRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "custom-agent");
        assert_eq!(req.system_prompt, "You are a custom agent.");
        assert_eq!(req.scope, "project");
    }

    #[test]
    fn test_create_agent_request_defaults_scope_to_project() {
        let json = r#"{"name":"x","system_prompt":"prompt"}"#;
        let req: CreateAgentRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.scope, "project");
    }

    #[test]
    fn test_agent_definition_response_serializes() {
        let resp = AgentDefinitionResponse {
            name: "coder".to_string(),
            description: "Writes code".to_string(),
            model: "gpt-5".to_string(),
            temperature: 0.2,
            max_tokens: 8192,
            tools: vec!["filesystem".to_string()],
            max_turns: 30,
            max_depth: 0,
            can_spawn: vec![],
            system_prompt: "You are a coder.".to_string(),
            scope: "builtin".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"name\":\"coder\""));
        assert!(json.contains("\"scope\":\"builtin\""));
        assert!(json.contains("\"max_depth\":0"));
    }

    #[test]
    fn test_serialize_agent_md_roundtrips() {
        let def = crate::agents::AgentDefinition {
            frontmatter: crate::agents::AgentFrontmatter {
                name: "test-agent".to_string(),
                description: "A test".to_string(),
                model: "gpt-5".to_string(),
                temperature: 0.3,
                max_tokens: 4096,
                tools: vec!["filesystem".to_string(), "git".to_string()],
                max_turns: 25,
                max_depth: 1,
                can_spawn: vec!["explorer".to_string()],
            },
            system_prompt: "You are a test agent.\nDo things.".to_string(),
        };
        let md = serialize_agent_md(&def);
        let parsed = crate::agents::parse_agent_md(&md).unwrap();
        assert_eq!(parsed.name(), "test-agent");
        assert_eq!(parsed.model(), "gpt-5");
        assert_eq!(parsed.tools(), &["filesystem", "git"]);
        assert_eq!(parsed.max_depth(), 1);
        assert!(parsed.can_spawn_type("explorer"));
        assert!(parsed.system_prompt().contains("You are a test agent."));
    }

    #[test]
    fn test_serialize_agent_md_empty_tools_and_spawn() {
        let def = crate::agents::AgentDefinition {
            frontmatter: crate::agents::AgentFrontmatter {
                name: "leaf".to_string(),
                description: String::new(),
                model: "gpt-5".to_string(),
                temperature: 0.3,
                max_tokens: 4096,
                tools: vec![],
                max_turns: 25,
                max_depth: 0,
                can_spawn: vec![],
            },
            system_prompt: "Leaf agent.".to_string(),
        };
        let md = serialize_agent_md(&def);
        let parsed = crate::agents::parse_agent_md(&md).unwrap();
        assert!(parsed.tools().is_empty());
        assert!(parsed.can_spawn().is_empty());
        assert_eq!(parsed.max_depth(), 0);
    }
}
