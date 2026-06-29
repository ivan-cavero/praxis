//! REST API routes.

use axum::{Router, routing::{get, post, delete}, Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Shared state for the API server.
#[derive(Clone)]
pub struct AppState {
    pub version: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub bus: crate::EventBus,
    pub auth: std::sync::Arc<super::auth::AuthState>,
    pub vault: std::sync::Arc<project_x_vault::VaultService>,
}

/// API server configuration.
pub struct ApiServerConfig {
    pub port: u16,
    pub cors_origins: Vec<String>,
    /// Optional master password for vault encryption.
    /// If None, vault stores keys unencrypted (fallback).
    pub vault_password: Option<String>,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            cors_origins: vec!["*".to_string()],
            vault_password: None,
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

    /// Build the Axum router with all routes and auth middleware.
    pub fn router(state: AppState) -> Router {
        let cors = tower_http::cors::CorsLayer::permissive();
        let auth_state = state.auth.clone();

        // Vault routes — exempt from auth (they SET the auth credentials)
        let vault_routes = Router::new()
            .route("/api/vault/keys", get(vault::list_keys))
            .route("/api/vault/keys", post(vault::set_key))
            .route("/api/vault/keys/{provider}", delete(vault::delete_key))
            .with_state(Arc::new(state.clone()));

        // Authenticated routes
        let auth_routes = Router::new()
            // Health (exempt from auth)
            .route("/api/health", get(routes::health))
            // Projects
            .route("/api/projects", get(routes::list_projects))
            .route("/api/projects", post(routes::create_project))
            // Sessions
            .route("/api/sessions", get(routes::list_sessions))
            // Metrics
            .route("/api/metrics/tokens", get(routes::token_metrics))
            .route("/api/metrics/summary", get(routes::metrics_summary))
            // Context
            .route("/api/metrics/context", get(routes::context_metrics))
            // WebSocket (exempt from auth)
            .route("/ws/global", get(super::ws::ws_handler))
            .with_state(Arc::new(state))
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                super::auth::auth_middleware,
            ));

        Router::new()
            .merge(vault_routes)
            .merge(auth_routes)
            .layer(cors)
    }

    /// Start the server (non-blocking).
    pub async fn start(self) -> anyhow::Result<()> {
        let bus = crate::EventBus::new();

        // Initialize JWT auth with auto-generated or loaded secret
        let secret_path = std::path::PathBuf::from(".forge")
            .join("jwt.secret");
        let auth = std::sync::Arc::new(
            super::auth::AuthState::from_file_or_create(&secret_path)
        );

        // Initialize vault service
        let vault_dir = std::path::PathBuf::from(".forge");
        if !vault_dir.exists() {
            std::fs::create_dir_all(&vault_dir)?;
        }
        let vault_path = vault_dir.join("credentials.vault.json");
        let vault = std::sync::Arc::new(
            project_x_vault::VaultService::with_path(vault_path.clone(), self.config.vault_password.clone())
        );
        vault.init()?;

        let state = AppState {
            version: env!("CARGO_PKG_VERSION").to_string(),
            started_at: chrono::Utc::now(),
            bus,
            auth,
            vault,
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

// ─── Vault API Handlers ───────────────────────────────────────

/// Request body for setting a provider API key.
#[derive(Deserialize)]
pub struct SetKeyRequest {
    /// Provider name (e.g., "nan", "openai", "anthropic")
    pub provider: String,
    /// The API key value
    pub api_key: String,
    /// Optional base URL override
    pub base_url: Option<String>,
}

/// Response for a stored provider key.
#[derive(Serialize)]
pub struct ProviderKeyResponse {
    pub provider: String,
    pub key_masked: String,
    pub has_key: bool,
}

/// Response for listing all configured providers.
#[derive(Serialize)]
pub struct ListKeysResponse {
    pub providers: Vec<ProviderKeyResponse>,
    pub total: usize,
}

pub mod vault {
    use super::*;

    /// List all configured provider keys (with masked values).
    pub async fn list_keys(
        State(state): State<Arc<AppState>>,
    ) -> Json<ListKeysResponse> {
        let keys = state.vault.list_keys()
            .unwrap_or_default();

        let providers: Vec<ProviderKeyResponse> = keys
            .into_iter()
            .map(|provider| {
                let has_key = state.vault.get(&provider).unwrap_or_default().is_some();
                let key_masked = if has_key {
                    let key = state.vault.get(&provider).unwrap().unwrap();
                    if key.len() > 8 {
                        format!("{}...{}", &key[..4], &key[key.len()-4..])
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

        let total = providers.len();
        Json(ListKeysResponse {
            providers,
            total,
        })
    }

    /// Set (or update) a provider API key.
    pub async fn set_key(
        State(state): State<Arc<AppState>>,
        Json(request): Json<SetKeyRequest>,
    ) -> (StatusCode, Json<ProviderKeyResponse>) {
        // Validate provider name (alphanumeric + underscore only)
        if !request.provider.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return (
                StatusCode::BAD_REQUEST,
                Json(ProviderKeyResponse {
                    provider: request.provider,
                    key_masked: String::new(),
                    has_key: false,
                }),
            );
        }

        // Store the key in the vault
        if let Err(e) = state.vault.set(&request.provider, &request.api_key) {
            tracing::error!("Failed to store API key for {}: {}", request.provider, e);
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
            format!("{}...{}", &request.api_key[..4], &request.api_key[request.api_key.len()-4..])
        } else {
            "****".to_string()
        };

        tracing::info!("Stored API key for provider: {}", request.provider);

        (StatusCode::OK, Json(ProviderKeyResponse {
            provider: request.provider,
            key_masked: masked,
            has_key: true,
        }))
    }

    /// Delete a provider API key.
    pub async fn delete_key(
        State(state): State<Arc<AppState>>,
        axum::extract::Path(provider): axum::extract::Path<String>,
    ) -> (StatusCode, Json<ProviderKeyResponse>) {
        if let Err(e) = state.vault.delete(&provider) {
            tracing::error!("Failed to delete API key for {}: {}", provider, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ProviderKeyResponse {
                    provider,
                    key_masked: String::new(),
                    has_key: false,
                }),
            );
        }

        tracing::info!("Deleted API key for provider: {}", provider);

        (StatusCode::OK, Json(ProviderKeyResponse {
            provider,
            key_masked: String::new(),
            has_key: false,
        }))
    }
}

// ─── Response Types ───────────────────────────────────────────

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub id: String,
    pub project_id: String,
    pub status: String,
    pub goal: String,
    pub phase: String,
    pub iteration: u32,
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

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: u16,
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

    pub async fn list_projects() -> Json<Vec<ProjectResponse>> {
        Json(vec![])
    }

    pub async fn create_project(
        Json(_request): Json<CreateProjectRequest>,
    ) -> (StatusCode, Json<ProjectResponse>) {
        let response = ProjectResponse {
            id: uuid::Uuid::new_v4().to_string(),
            name: "new-project".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        (StatusCode::CREATED, Json(response))
    }

    pub async fn list_sessions() -> Json<Vec<SessionResponse>> {
        Json(vec![])
    }

    pub async fn token_metrics() -> Json<TokenMetricsResponse> {
        Json(TokenMetricsResponse {
            total_input: 0,
            total_output: 0,
            total_tokens: 0,
            by_provider: std::collections::HashMap::new(),
            by_model: std::collections::HashMap::new(),
        })
    }

    pub async fn context_metrics() -> Json<ContextMetricsResponse> {
        Json(ContextMetricsResponse {
            avg_pressure: 0.0,
            max_pressure: 0.0,
            total_compressions: 0,
            active_sessions: 0,
        })
    }

    pub async fn metrics_summary(State(state): State<Arc<AppState>>) -> Json<MetricsSummaryResponse> {
        let uptime = chrono::Utc::now()
            .signed_duration_since(state.started_at)
            .num_seconds() as u64;

        Json(MetricsSummaryResponse {
            version: state.version.clone(),
            uptime_seconds: uptime,
            active_sessions: 0,
            total_tokens: 0,
            avg_asi_score: 100.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_structure() {
        let vault = project_x_vault::VaultService::with_path(
            std::env::temp_dir().join("test-vault.json"),
            None,
        );
        let state = AppState {
            version: "0.1.0".to_string(),
            started_at: chrono::Utc::now(),
            bus: crate::EventBus::new(),
            auth: std::sync::Arc::new(crate::api::auth::AuthState::new(b"test-secret-key-for-testing-32bytes!!")),
            vault: std::sync::Arc::new(vault),
        };

        let uptime = chrono::Utc::now()
            .signed_duration_since(state.started_at)
            .num_seconds() as u64;

        let response = HealthResponse {
            status: "ok".to_string(),
            version: state.version.clone(),
            uptime_seconds: uptime,
        };

        assert_eq!(response.status, "ok");
        assert_eq!(response.version, "0.1.0");
    }

    #[test]
    fn test_project_response_structure() {
        let response = ProjectResponse {
            id: uuid::Uuid::new_v4().to_string(),
            name: "test-project".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        assert!(!response.id.is_empty());
        assert_eq!(response.name, "test-project");
    }

    #[test]
    fn test_metrics_summary_structure() {
        let response = MetricsSummaryResponse {
            version: "0.1.0".to_string(),
            uptime_seconds: 100,
            active_sessions: 5,
            total_tokens: 10000,
            avg_asi_score: 85.0,
        };
        assert_eq!(response.version, "0.1.0");
        assert_eq!(response.active_sessions, 5);
        assert_eq!(response.avg_asi_score, 85.0);
    }
}
