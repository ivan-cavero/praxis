//! WebSocket handler — real-time event streaming from the EventBus.
//!
//! Each connected client receives all system events as JSON.
//! Clients can send commands (inject, subscribe, unsubscribe).
//!
//! All connections require a valid Bearer JWT token.

use super::auth::Claims;
use super::routes::AppState;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket};
use axum::http::header;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Handle an incoming WebSocket connection.
pub async fn ws_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(state): axum::extract::State<Arc<AppState>>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let claims = match extract_bearer_token(&request) {
        Some(token) => match state.auth.validate_token(&token) {
            Ok(c) => c,
            Err(_) => {
                return ws.on_upgrade(|_| async {});
            }
        },
        None => {
            return ws.on_upgrade(|_| async {});
        }
    };

    tracing::info!("WS authenticated client: {}@{}", claims.sub, claims.role);
    ws.on_upgrade(move |socket| handle_socket(socket, claims, state))
}

fn extract_bearer_token(request: &axum::extract::Request) -> Option<String> {
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| {
            s.strip_prefix("Bearer ")
                .map(|stripped| stripped.to_string())
        })
}

async fn handle_socket(socket: WebSocket, claims: Claims, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let client_id = claims.sub.clone();
    let mut rx = state.bus.subscribe();

    let send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let kind_value = serde_json::to_value(&event.kind).unwrap_or_default();
                    let msg = serde_json::json!({
                        "id": event.id,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "kind": kind_value,
                        "source": event.source,
                        "metadata": event.metadata,
                    });
                    let payload = serde_json::to_string(&msg).unwrap_or_default();
                    if sender.send(Message::Text(payload.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::debug!("Client lagged by {} events", n);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                handle_client_message(&text, &client_id);
            }
            Ok(Message::Close(_)) => break,
            Err(e) => {
                tracing::debug!("WS error: {}", e);
                break;
            }
            _ => {}
        }
    }

    send_task.abort();
}

fn handle_client_message(text: &str, client_id: &str) {
    let msg: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => {
            tracing::debug!("WS {} invalid JSON: {}", client_id, text);
            return;
        }
    };

    let msg_type = msg["type"].as_str().unwrap_or("unknown");

    match msg_type {
        "ping" => {
            tracing::debug!("WS {} ping received", client_id);
        }
        "inject" => {
            let target = msg["target"].as_str().unwrap_or("unknown");
            let message = msg["message"].as_str().unwrap_or("");
            tracing::info!("WS {} inject to {}: {}", client_id, target, message);
        }
        "subscribe" | "unsubscribe" => {
            tracing::debug!("WS {} {} requested", client_id, msg_type);
        }
        _ => {
            tracing::debug!("WS {} unknown command: {}", client_id, msg_type);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_client_message_ping() {
        handle_client_message(r#"{"type": "ping"}"#, "test-client");
    }

    #[test]
    fn test_handle_client_message_inject() {
        handle_client_message(
            r#"{"type": "inject", "target": "coder", "message": "use thiserror"}"#,
            "test-client",
        );
    }

    #[test]
    fn test_handle_client_message_invalid_json() {
        handle_client_message("not-json", "test-client");
    }

    #[test]
    fn test_handle_client_message_unknown() {
        handle_client_message(r#"{"type": "unknown_command"}"#, "test-client");
    }
}
