//! REST API routes.
//!
//! Architecture:
//!   %APPDATA%/praxis/          (o ~/.config/praxis/)
//!   ├── projects.json             ← TODOS los proyectos con forge_toml embebido
//!   ├── jwt.secret                ← Global
//!   └── credentials.vault.json    ← Global (API keys del usuario)

use axum::{Router, routing::{get, post, delete, put}, Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use praxis_agent_traits::persistence::EventStore;

/// Shared state for the API server.
#[derive(Clone)]
pub struct AppState {
    pub version: String,
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
    /// Persistent event store (SQLite) shared with `praxis run --goal`.
    /// Used to read persisted sessions from other processes.
    pub event_store: Option<std::sync::Arc<praxis_persistence::SqliteEventStore>>,
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
}

/// API server configuration.
pub struct ApiServerConfig {
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub vault_password: Option<String>,
    /// Data directory (%APPDATA%/praxis or ~/.config/praxis).
    pub data_dir: std::path::PathBuf,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            cors_origins: vec!["*".to_string()],
            vault_password: None,
            data_dir: std::path::PathBuf::from("."),
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
            .route("/api/sessions", get(sessions::list))
            .route("/api/sessions/{id}", get(sessions::get_one))
            .route("/api/sessions/{id}/stop", post(sessions::stop))
            .route("/api/agents", get(sessions::list_agents))
            .with_state(Arc::new(state.clone()))
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                super::auth::auth_middleware,
            ));

        // Public routes
        let public_routes = Router::new()
            .route("/api/health", get(routes::health))
            .route("/api/metrics/tokens", get(routes::token_metrics))
            .route("/api/metrics/summary", get(routes::metrics_summary))
            .route("/api/metrics/context", get(routes::context_metrics))
            .route("/api/vault/keys", get(vault::list_keys))
            .route("/api/vault/keys", post(vault::set_key))
            .route("/api/vault/keys/{provider}", delete(vault::delete_key))
            .route("/api/inject", post(routes::inject))
            .route("/api/sessions", get(sessions::list))
            .route("/api/sessions/{id}", get(sessions::get_one))
            .route("/api/sessions/{id}/stop", post(sessions::stop))
            .route("/api/agents", get(sessions::list_agents))
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
        let auth = std::sync::Arc::new(
            super::auth::AuthState::from_file_or_create(&secret_path)
        );

        // Initialize vault
        let vault_path = data_dir.join("credentials.vault.json");
        let vault = std::sync::Arc::new(
            praxis_vault::VaultService::with_path(vault_path, self.config.vault_password.clone())
        );
        vault.init()?;

        // Initialize token counters from event store or start fresh
        let token_counters = std::sync::Arc::new(std::sync::RwLock::new(TokenCounters::default()));
        let session_registry = std::sync::Arc::new(std::sync::RwLock::new(Vec::new()));

        // Open shared event store for reading sessions from other processes
        let db_path = data_dir.join("state.db");
        let event_store = if db_path.exists() {
            tracing::info!("Opening shared event store at {}", db_path.display());
            praxis_persistence::SqliteEventStore::new(&db_path)
                .ok()
                .map(std::sync::Arc::new)
        } else {
            tracing::info!("No shared event store found ({}), sessions from other processes won't appear", db_path.display());
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
                        if let praxis_shared::protocol::MessageKind::TokenUsed { provider, model, input, output } = event.kind {
                            let mut counters = token_counters_listener.write().unwrap();
                            counters.total_input += input as u64;
                            counters.total_output += output as u64;
                            *counters.by_provider.entry(provider).or_insert(0) += (input + output) as u64;
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

        let state = AppState {
            version: env!("CARGO_PKG_VERSION").to_string(),
            started_at: chrono::Utc::now(),
            bus,
            auth,
            vault,
            data_dir,
            token_counters,
            session_registry,
            event_store,
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
        .and_then(|content| {
            std::fs::write(&path, &content).map_err(|e| e.to_string())
        })
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
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Optional forge.toml content. Auto-generated if empty.
    #[serde(default)]
    pub forge_toml: String,
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

// ─── Vault API ────────────────────────────────────────────────

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
        let providers: Vec<ProviderKeyResponse> = keys.into_iter().map(|provider| {
            let has_key = state.vault.get(&provider).unwrap_or_default().is_some();
            let key_masked = if has_key {
                let key = state.vault.get(&provider).unwrap().unwrap();
                if key.len() > 8 {
                    format!("{}...{}", &key[..4], &key[key.len()-4..])
                } else { "****".to_string() }
            } else { String::new() };
            ProviderKeyResponse { provider, key_masked, has_key }
        }).collect();
        Json(ListKeysResponse { total: providers.len(), providers })
    }

    pub async fn set_key(
        State(state): State<Arc<AppState>>,
        Json(request): Json<SetKeyRequest>,
    ) -> (StatusCode, Json<ProviderKeyResponse>) {
        if !request.provider.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return (StatusCode::BAD_REQUEST, Json(ProviderKeyResponse {
                provider: request.provider, key_masked: String::new(), has_key: false,
            }));
        }
        if let Err(e) = state.vault.set(&request.provider, &request.api_key) {
            tracing::error!("Failed to store API key: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ProviderKeyResponse {
                provider: request.provider, key_masked: String::new(), has_key: false,
            }));
        }
        let masked = if request.api_key.len() > 8 {
            format!("{}...{}", &request.api_key[..4], &request.api_key[request.api_key.len()-4..])
        } else { "****".to_string() };
        (StatusCode::OK, Json(ProviderKeyResponse {
            provider: request.provider, key_masked: masked, has_key: true,
        }))
    }

    pub async fn delete_key(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(provider): axum::extract::Path<String>,
    ) -> (StatusCode, Json<ProviderKeyResponse>) {
        if let Err(e) = state.vault.delete(&provider) {
            tracing::error!("Failed to delete key: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ProviderKeyResponse {
                provider, key_masked: String::new(), has_key: false,
            }));
        }
        (StatusCode::OK, Json(ProviderKeyResponse {
            provider, key_masked: String::new(), has_key: false,
        }))
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
        projects.into_iter().find(|p| p.id == id)
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

        let entry = ProjectEntry {
            id: id.clone(),
            name: request.name,
            description: request.description,
            created_at: now.clone(),
            last_active: now,
            forge_toml,
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
        let idx = projects.iter().position(|p| p.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;

        let entry = &mut projects[idx];
        if let Some(name) = request.name { entry.name = name; }
        if let Some(desc) = request.description { entry.description = desc; }
        if let Some(toml) = request.forge_toml { entry.forge_toml = toml; }
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
        let entry = projects.into_iter().find(|p| p.id == id)
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
        let idx = projects.iter().position(|p| p.id == id)
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
        .get("roles").and_then(|v| v.as_table())
        .map(|table| table.iter().map(|(name, value)| {
            let detail = RoleDetail {
                model: value.get("model").and_then(|v| v.as_str()).unwrap_or("gpt-4o").to_string(),
                temperature: value.get("temperature").and_then(|v| v.as_float()).unwrap_or(0.3),
                max_tokens: value.get("max_tokens").and_then(|v| v.as_integer()).unwrap_or(4096) as u32,
                system_prompt: value.get("system_prompt").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                tools: value.get("tools").and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                description: value.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            };
            (name.clone(), detail)
        }).collect()).unwrap_or_default();

    let providers: HashMap<String, ProviderDetail> = parsed
        .get("providers").and_then(|v| v.as_table())
        .map(|table| table.iter().map(|(name, value)| {
            let detail = ProviderDetail {
                base_url: value.get("base_url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                api_key_ref: value.get("api_key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                default_model: value.get("default_model").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            };
            (name.clone(), detail)
        }).collect()).unwrap_or_default();

    let goals: Vec<GoalDetail> = parsed
        .get("goals").and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_table()).map(|table| GoalDetail {
            name: table.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            agents: table.get("agents").and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            max_iterations: table.get("max_iterations").and_then(|v| v.as_integer()).unwrap_or(10) as u32,
            gates: table.get("gates").and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
        }).collect()).unwrap_or_default();

    let limits = LimitsDetail {
        max_iterations_per_goal: parsed.get("limits").and_then(|l| l.get("max_iterations_per_goal")).and_then(|v| v.as_integer()).unwrap_or(50) as u32,
        max_iterations_per_phase: parsed.get("limits").and_then(|l| l.get("max_iterations_per_phase")).and_then(|v| v.as_integer()).unwrap_or(5) as u32,
        session_ttl_seconds: parsed.get("limits").and_then(|l| l.get("session_ttl_seconds")).and_then(|v| v.as_integer()).unwrap_or(3600) as u32,
        phase_timeout_seconds: parsed.get("limits").and_then(|l| l.get("phase_timeout_seconds")).and_then(|v| v.as_integer()).unwrap_or(300) as u32,
    };

    let project_version = parsed.get("project").and_then(|p| p.get("version")).and_then(|v| v.as_str()).unwrap_or("0.1.0").to_string();

    ParsedConfig { roles, providers, goals, limits, project_version }
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
            let registry = state.session_registry.read().unwrap();
            registry.clone()
        };

        // Merge sessions from the event store (persisted sessions from other processes)
        if let Some(store) = &state.event_store {
            if let Ok(session_ids) = store.list_aggregates("session").await {
                for sid in &session_ids {
                    let sid_str = sid.to_string();
                    // Skip if already in the in-memory registry
                    if all.iter().any(|s| s.id == sid_str) {
                        continue;
                    }
                    if let Ok(Some(snap)) = store.get_snapshot(*sid).await {
                        let phase = snap.state["phase"].as_str().unwrap_or("unknown");
                        let status = if phase == "Completed" || phase == "Failed" || phase == "Cancelled" {
                            phase.to_lowercase()
                        } else {
                            "running".to_string()
                        };
                        all.push(SessionEntry {
                            id: sid_str,
                            project: snap.state["project"].as_str().unwrap_or("default").to_string(),
                            goal: snap.state["goal"].as_str().unwrap_or("unknown").to_string(),
                            phase: phase.to_string(),
                            iteration: snap.state["iteration"].as_u64().unwrap_or(0) as u32,
                            status,
                            started_at: snap.updated_at.clone(),
                            completed_at: None,
                        });
                    }
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
            let sessions = state.session_registry.read().unwrap();
            if let Some(session) = sessions.iter().find(|s| s.id == id) {
                return Ok(Json(session.clone()));
            }
        }

        // Fall back to the event store
        if let Some(store) = &state.event_store {
            if let Ok(sid) = id.parse::<uuid::Uuid>() {
                if let Ok(Some(snap)) = store.get_snapshot(sid).await {
                    let phase = snap.state["phase"].as_str().unwrap_or("unknown");
                    let status = if phase == "Completed" || phase == "Failed" || phase == "Cancelled" {
                        phase.to_lowercase()
                    } else {
                        "running".to_string()
                    };
                    return Ok(Json(SessionEntry {
                        id,
                        project: snap.state["project"].as_str().unwrap_or("default").to_string(),
                        goal: snap.state["goal"].as_str().unwrap_or("unknown").to_string(),
                        phase: phase.to_string(),
                        iteration: snap.state["iteration"].as_u64().unwrap_or(0) as u32,
                        status,
                        started_at: snap.updated_at.clone(),
                        completed_at: None,
                    }));
                }
            }
        }

        Err(StatusCode::NOT_FOUND)
    }

    pub async fn stop(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> Result<Json<serde_json::Value>, StatusCode> {
        // Write a stop-injection for this session
        let injections_dir = state.data_dir.join("injections");
        let _ = std::fs::create_dir_all(&injections_dir);

        let msg = serde_json::json!({
            "target_agent": "all",
            "message_type": "stop",
            "content": "Stop the current session immediately.",
            "created_at": chrono::Utc::now().to_rfc3339(),
        });

        let filename = format!("{}_stop_{}.json", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0), id);
        let path = injections_dir.join(&filename);
        let content = serde_json::to_string_pretty(&msg).unwrap_or_default();
        let _ = std::fs::write(&path, &content);

        // Mark session as completed in registry
        if let Ok(mut guard) = state.session_registry.write() {
            if let Some(session) = guard.iter_mut().find(|s| s.id == id) {
                session.status = "completed".to_string();
                session.completed_at = Some(chrono::Utc::now().to_rfc3339());
            }
        }

        Ok(Json(serde_json::json!({
            "status": "stopped",
            "session_id": id,
        })))
    }

    #[derive(Serialize)]
    pub struct AgentSummary {
        pub name: String,
        pub role: String,
        pub model: String,
        pub tools: Vec<String>,
        pub status: String,
    }

    pub async fn list_agents(
        State(state): State<Arc<AppState>>,
    ) -> Json<Vec<AgentSummary>> {
        // Return agents from the most recent session, or default list
        let sessions = state.session_registry.read().unwrap();
        if let Some(_latest) = sessions.last() {
            // Parse forge.toml from the session project for agent list
            // For now, return the built-in agent list
            Json(vec![
                AgentSummary { name: "architect".into(), role: "Architect".into(), model: "claude-sonnet-4-20250514".into(), tools: vec!["filesystem".into(), "web_search".into()], status: "idle".into() },
                AgentSummary { name: "coder".into(), role: "Coder".into(), model: "gpt-4o".into(), tools: vec!["filesystem".into(), "execute_command".into()], status: "idle".into() },
                AgentSummary { name: "reviewer".into(), role: "Reviewer".into(), model: "gpt-4o".into(), tools: vec!["filesystem".into()], status: "idle".into() },
                AgentSummary { name: "security".into(), role: "Security".into(), model: "claude-sonnet-4-20250514".into(), tools: vec!["filesystem".into(), "grep".into()], status: "idle".into() },
                AgentSummary { name: "tester".into(), role: "Tester".into(), model: "gpt-4o".into(), tools: vec!["filesystem".into(), "execute_command".into()], status: "idle".into() },
            ])
        } else {
            Json(vec![])
        }
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
        let counters = state.token_counters.read().unwrap();
        Json(TokenMetricsResponse {
            total_input: counters.total_input,
            total_output: counters.total_output,
            total_tokens: counters.total_input + counters.total_output,
            by_provider: counters.by_provider.clone(),
            by_model: counters.by_model.clone(),
        })
    }

    pub async fn context_metrics(State(state): State<Arc<AppState>>) -> Json<ContextMetricsResponse> {
        let session_count = state.session_registry.read().unwrap().len() as u32;
        Json(ContextMetricsResponse {
            avg_pressure: 0.0, max_pressure: 0.0,
            total_compressions: 0, active_sessions: session_count,
        })
    }

    pub async fn inject(
        State(state): State<Arc<AppState>>,
        Json(req): Json<InjectRequest>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        // Write injection to {data_dir}/injections/ as a JSON file
        let injections_dir = state.data_dir.join("injections");
        std::fs::create_dir_all(&injections_dir)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create injections dir: {}", e)))?;

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
        let content = serde_json::to_string_pretty(&msg)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to serialize: {}", e)))?;
        std::fs::write(&path, &content)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write injection: {}", e)))?;

        tracing::info!("Injection written to: {:?}", path);

        Ok(Json(serde_json::json!({
            "status": "injected",
            "file": filename,
            "target_agent": req.target_agent,
            "message_type": req.message_type,
        })))
    }

    pub async fn metrics_summary(State(state): State<Arc<AppState>>) -> Json<MetricsSummaryResponse> {
        let uptime = chrono::Utc::now()
            .signed_duration_since(state.started_at)
            .num_seconds() as u64;
        let session_count = state.session_registry.read().unwrap().len() as u32;
        let total_tokens = {
            let counters = state.token_counters.read().unwrap();
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
}
