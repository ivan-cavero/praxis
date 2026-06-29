//! E2E tests for the Vault API — provider key management.
//!
//! Starts a real Axum server and tests vault CRUD operations.

use axum::{Router, routing::get, Json, extract::State};
use std::sync::Arc;

/// Minimal test server with vault endpoints.
async fn start_vault_server() -> (u16, String) {
    let bus = project_x_core::EventBus::new();
    let auth = std::sync::Arc::new(project_x_core::api::auth::AuthState::new(b"test-secret-key-for-vault-e2e-tests-32b!!"));
    let vault = std::sync::Arc::new(project_x_vault::VaultService::with_path(
        std::env::temp_dir().join(format!("vault-e2e-{}.json", uuid::Uuid::new_v4())),
        None,
    ));

    let state = project_x_core::api::routes::AppState {
        version: "test-0.1.0".to_string(),
        started_at: chrono::Utc::now(),
        bus,
        auth,
        vault: vault.clone(),
    };

    let app = project_x_core::api::ApiServer::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    (port, String::new())
}

// Helper: set a provider key
async fn set_key(port: u16, provider: &str, api_key: &str) {
    let url = format!("http://127.0.0.1:{}/api/vault/keys", port);
    let body = serde_json::json!({
        "provider": provider,
        "api_key": api_key
    });
    reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .unwrap();
}

// ═══════════════════════════════════════════════════════════════
// VAULT E2E TESTS
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn e2e_vault_list_empty() {
    let (port, _) = start_vault_server().await;
    let url = format!("http://127.0.0.1:{}/api/vault/keys", port);

    let resp = reqwest::get(&url).await.unwrap();
    assert!(resp.status().is_success(), "Vault list should return 200");

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["total"], 0);
    assert_eq!(body["providers"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn e2e_vault_set_key() {
    let (port, _) = start_vault_server().await;
    let url = format!("http://127.0.0.1:{}/api/vault/keys", port);

    let body = serde_json::json!({
        "provider": "test_provider",
        "api_key": "sk-test-key-1234567890"
    });

    let resp = reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200, "Vault set response: {:?}", resp.text().await.unwrap_or_default());

    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["provider"], "test_provider");
    assert_eq!(json["has_key"], true);
    assert!(json["key_masked"].as_str().unwrap().contains("..."));
}

#[tokio::test]
async fn e2e_vault_set_then_list() {
    let (port, _) = start_vault_server().await;
    let base = format!("http://127.0.0.1:{}", port);

    // Set a key
    let set_url = format!("{}/api/vault/keys", base);
    let body = serde_json::json!({
        "provider": "openai",
        "api_key": "sk-proj-abc123"
    });
    reqwest::Client::new()
        .post(&set_url)
        .json(&body)
        .send()
        .await
        .unwrap();

    // List should show it
    let list_url = format!("{}/api/vault/keys", base);
    let resp = reqwest::get(&list_url).await.unwrap();
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["providers"][0]["provider"], "openai");
    assert_eq!(json["providers"][0]["has_key"], true);
}

#[tokio::test]
async fn e2e_vault_delete_key() {
    let (port, _) = start_vault_server().await;
    let base = format!("http://127.0.0.1:{}", port);

    // Set a key first
    let set_url = format!("{}/api/vault/keys", base);
    let body = serde_json::json!({
        "provider": "to-delete",
        "api_key": "sk-temp-key"
    });
    reqwest::Client::new()
        .post(&set_url)
        .json(&body)
        .send()
        .await
        .unwrap();

    // Delete it
    let del_url = format!("{}/api/vault/keys/to-delete", base);
    let resp = reqwest::Client::new()
        .delete(&del_url)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "Vault delete should return 200");

    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["provider"], "to-delete");
    assert_eq!(json["has_key"], false);

    // List should be empty
    let list_url = format!("{}/api/vault/keys", base);
    let resp = reqwest::get(&list_url).await.unwrap();
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["total"], 0);
}

#[tokio::test]
async fn e2e_vault_multiple_keys() {
    let (port, _) = start_vault_server().await;
    let base = format!("http://127.0.0.1:{}", port);

    // Set multiple keys
    for provider in &["nan", "openai", "anthropic"] {
        let body = serde_json::json!({
            "provider": provider,
            "api_key": format!("sk-{}-key", provider)
        });
        reqwest::Client::new()
            .post(format!("{}/api/vault/keys", base))
            .json(&body)
            .send()
            .await
            .unwrap();
    }

    // List should show all 3
    let resp = reqwest::get(format!("{}/api/vault/keys", base)).await.unwrap();
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["total"], 3);

    let providers: Vec<String> = json["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["provider"].as_str().unwrap().to_string())
        .collect();
    assert!(providers.contains(&"nan".to_string()));
    assert!(providers.contains(&"openai".to_string()));
    assert!(providers.contains(&"anthropic".to_string()));
}

#[tokio::test]
async fn e2e_vault_key_masking() {
    let (port, _) = start_vault_server().await;
    let url = format!("http://127.0.0.1:{}/api/vault/keys", port);

    let body = serde_json::json!({
        "provider": "masked_test",
        "api_key": "sk-very-long-secret-key-that-should-be-masked-12345"
    });

    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .unwrap();

    let json: serde_json::Value = resp.json().await.unwrap();
    let masked = json["key_masked"].as_str().unwrap();
    // Should show first 4 and last 4 chars with "..." in between
    assert!(masked.contains("..."), "Masked key should contain '...': {}", masked);
    assert!(masked.len() < 20, "Masked key should be short: {}", masked);
}

#[tokio::test]
async fn e2e_vault_overwrite_key() {
    let (port, _) = start_vault_server().await;
    let url = format!("http://127.0.0.1:{}/api/vault/keys", port);

    // Set initial key
    let body1 = serde_json::json!({
        "provider": "overwrite-test",
        "api_key": "sk-original-key"
    });
    reqwest::Client::new()
        .post(&url)
        .json(&body1)
        .send()
        .await
        .unwrap();

    // Overwrite with new key
    let body2 = serde_json::json!({
        "provider": "overwrite_test",
        "api_key": "sk-new-key-updated"
    });
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body2)
        .send()
        .await
        .unwrap();

    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["has_key"], true);

    // Verify list still shows only 1
    let list_resp = reqwest::get(&url).await.unwrap();
    let list_json: serde_json::Value = list_resp.json().await.unwrap();
    assert_eq!(list_json["total"], 1);
}
