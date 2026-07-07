//! HTTP API server — REST + WebSocket for dashboard and external clients.

pub mod auth;
pub mod pairing;
pub mod routes;
pub mod ws;

pub use routes::{ApiServer, ApiServerConfig, AppState};
pub use ws::ws_handler;
