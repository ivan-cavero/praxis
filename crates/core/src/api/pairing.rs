//! Pairing system — QR-based device pairing for remote connections.
//!
//! ## Future design note (Fase B)
//!
//! When `praxis.dev` exists, the `issuer` field in JWTs will change from the
//! server's own key to `praxis.dev`'s key. The pairing flow stays the same:
//! instead of the QR pointing to `http://VPS:8080/api/pair/{code}`, it will
//! point to `https://app.praxis.dev/pair?code={code}&host=VPS:8080&pubkey=...`.
//! The `PairingCode` struct already has room for extra fields.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
};
use base64::Engine;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// A pairing code — short-lived, used to authorize a remote device.
#[derive(Debug, Clone)]
pub struct PairingCode {
    pub code: String,
    pub created_at: Instant,
    pub expires_at: Instant,
    pub qr_path: String,   // e.g. "/api/pair/ABC12345"
    pub challenge: String, // random bytes base64 — prevents CSRF on confirm
    pub claimed_at: Option<Instant>,
    pub claimed_by_device: Option<String>, // device name
    pub device_id: Option<String>,         // UUID assigned on claim
}

impl PairingCode {
    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    fn is_claimed(&self) -> bool {
        self.claimed_at.is_some()
    }

    /// Remaining seconds before expiry (rounded up).
    fn remaining_secs(&self) -> u64 {
        let remaining = self.expires_at.saturating_duration_since(Instant::now());
        remaining.as_secs()
    }
}

/// A registered device that has paired successfully.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub token_hash: String, // SHA-256 of the JWT
    pub last_seen: String,  // RFC3339
    pub created_at: String, // RFC3339
}

/// Devices stored in `{data_dir}/devices.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceList {
    pub devices: Vec<Device>,
}

/// In-memory pairing state with background expiry cleanup.
pub struct PairingState {
    codes: Arc<RwLock<HashMap<String, PairingCode>>>,
    code_ttl: Duration,
}

impl PairingState {
    pub fn new(code_ttl_secs: u64) -> Self {
        let state = Self {
            codes: Arc::new(RwLock::new(HashMap::new())),
            code_ttl: Duration::from_secs(code_ttl_secs),
        };

        // Background task: clean up expired codes every 30 seconds
        let codes = state.codes.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let mut map = codes.write().await;
                map.retain(|_, pc| !pc.is_expired());
            }
        });

        state
    }

    /// Generate a new pairing code. Returns the code string and QR path.
    pub async fn generate(&self) -> (String, String) {
        let mut map = self.codes.write().await;
        let code = generate_code(&map);
        let challenge = generate_challenge();
        let now = Instant::now();

        let pc = PairingCode {
            code: code.clone(),
            created_at: now,
            expires_at: now + self.code_ttl,
            qr_path: format!("/api/pair/{code}"),
            challenge,
            claimed_at: None,
            claimed_by_device: None,
            device_id: None,
        };

        let path = format!("/api/pair/{code}");
        map.insert(code.clone(), pc);
        (code, path)
    }

    /// Get a pairing code by its code string.
    pub async fn get(&self, code: &str) -> Option<PairingCode> {
        let map = self.codes.read().await;
        map.get(code).cloned()
    }

    /// Claim a pairing code (called from the browser confirmation page).
    /// Returns `None` if the code is expired or doesn't exist.
    /// Returns `Some(Ok(device_id))` on success.
    /// Returns `Some(Err("already claimed"))` if already claimed.
    pub async fn claim(
        &self,
        code: &str,
        challenge: &str,
        device_name: &str,
    ) -> Option<Result<String, &'static str>> {
        let mut map = self.codes.write().await;
        let pc = map.get_mut(code)?;

        if pc.is_expired() {
            return Some(Err("expired"));
        }

        if pc.challenge != challenge {
            return Some(Err("invalid challenge"));
        }

        if pc.claimed_at.is_some() {
            return Some(Err("already claimed"));
        }

        let device_id = uuid::Uuid::new_v4().to_string();
        pc.claimed_at = Some(Instant::now());
        pc.claimed_by_device = Some(device_name.to_string());
        pc.device_id = Some(device_id.clone());

        Some(Ok(device_id))
    }

    /// Mark the pairing code as having its token retrieved.
    pub async fn mark_token_retrieved(&self, code: &str) {
        let mut map = self.codes.write().await;
        if let Some(_pc) = map.get_mut(code) {
            // We keep the code for a short while after retrieval so the
            // polling client can still check status, then it gets cleaned
            // up by the background expiry task.
        }
    }
}

/// Helper: generate a random 8-character alphanumeric code, unique in the map.
fn generate_code(map: &HashMap<String, PairingCode>) -> String {
    let mut rng = rand::rng();
    let chars: Vec<char> = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".chars().collect();
    loop {
        let code: String = (0..8)
            .map(|_| chars[rng.random_range(0..chars.len())])
            .collect();
        if !map.contains_key(&code) {
            return code;
        }
    }
}

/// Helper: generate a random challenge string (base64).
fn generate_challenge() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

// ─── Device persistence ─────────────────────────────────────────

/// Load devices from `{data_dir}/devices.json`.
pub fn load_devices(data_dir: &std::path::Path) -> DeviceList {
    let path = data_dir.join("devices.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => DeviceList::default(),
    }
}

/// Save devices to `{data_dir}/devices.json`.
pub fn save_devices(data_dir: &std::path::Path, devices: &DeviceList) -> Result<(), String> {
    let path = data_dir.join("devices.json");
    serde_json::to_string_pretty(devices)
        .map_err(|e| e.to_string())
        .and_then(|content| std::fs::write(&path, &content).map_err(|e| e.to_string()))
}

/// Add a device and return the updated list.
pub fn add_device(devices: &mut DeviceList, device: Device) {
    // Remove existing device with same name, then add
    devices.devices.retain(|d| d.name != device.name);
    devices.devices.push(device);
}

/// Remove a device by ID.
pub fn remove_device(devices: &mut DeviceList, device_id: &str) {
    devices.devices.retain(|d| d.id != device_id);
}

// ─── API handlers ───────────────────────────────────────────────

/// Response from `POST /api/pair`.
#[derive(Serialize)]
pub struct PairResponse {
    pub code: String,
    pub qr_url: String,
    pub expires_in: u64,
}

/// Generate a new pairing code. No auth required (this starts the pairing flow).
pub async fn create_pairing(
    State(state): State<Arc<super::routes::AppState>>,
) -> Result<Json<PairResponse>, StatusCode> {
    let pairing_state = state
        .pairing
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // Determine the host from the request, or use a reasonable default
    let host = "localhost"; // We'll try to be smarter in Fase B

    let (pair_code, path) = pairing_state.generate().await;
    let pc = pairing_state
        .get(&pair_code)
        .await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let qr_url = format!("http://{host}:{}{path}", state.port);

    Ok(Json(PairResponse {
        code: pair_code,
        qr_url,
        expires_in: pc.remaining_secs().max(1),
    }))
}

/// Status of a pairing code. No auth required.
#[derive(Serialize)]
pub struct PairStatusResponse {
    pub status: String, // "pending", "claimed", "expired"
    pub device_name: Option<String>,
    pub device_id: Option<String>,
    pub expires_in: u64,
}

pub async fn get_pairing_status(
    State(state): State<Arc<super::routes::AppState>>,
    Path(code): Path<String>,
) -> Result<Json<PairStatusResponse>, StatusCode> {
    let pairing_state = state
        .pairing
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let pc = pairing_state
        .get(&code)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    if pc.is_expired() {
        return Ok(Json(PairStatusResponse {
            status: "expired".to_string(),
            device_name: None,
            device_id: None,
            expires_in: 0,
        }));
    }

    if let Some(device_id) = &pc.device_id {
        Ok(Json(PairStatusResponse {
            status: "claimed".to_string(),
            device_name: pc.claimed_by_device.clone(),
            device_id: Some(device_id.clone()),
            expires_in: pc.remaining_secs(),
        }))
    } else {
        Ok(Json(PairStatusResponse {
            status: "pending".to_string(),
            device_name: None,
            device_id: None,
            expires_in: pc.remaining_secs(),
        }))
    }
}

/// Claim a pairing code — called from the browser confirmation page.
/// Returns a JWT for the new device, or stores a pending claim.
#[derive(Deserialize)]
pub struct ConfirmPairingBody {
    pub challenge: String,
    pub device_name: String,
}

#[derive(Serialize)]
pub struct ConfirmPairingResponse {
    pub success: bool,
    pub jwt: Option<String>,
    pub device_id: Option<String>,
    pub error: Option<String>,
}

pub async fn confirm_pairing(
    State(state): State<Arc<super::routes::AppState>>,
    Path(code): Path<String>,
    Json(body): Json<ConfirmPairingBody>,
) -> Result<Json<ConfirmPairingResponse>, StatusCode> {
    let pairing_state = state
        .pairing
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let result = pairing_state
        .claim(&code, &body.challenge, &body.device_name)
        .await;

    match result {
        Some(Ok(device_id)) => {
            // Generate JWT for this device
            let now = chrono::Utc::now().timestamp() as u64;
            let claims = super::auth::Claims {
                sub: device_id.clone(),
                iat: now,
                exp: now + 86400 * 365, // 1 year for paired devices
                role: "device".to_string(),
            };
            let jwt = state
                .auth
                .generate_token_from_claims(&claims)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            // Compute token hash and save device
            let token_hash = sha256_hex(&jwt);
            let now_rfc = chrono::Utc::now().to_rfc3339();
            let device = Device {
                id: device_id.clone(),
                name: body.device_name.clone(),
                token_hash,
                last_seen: now_rfc.clone(),
                created_at: now_rfc,
            };

            let mut devices = load_devices(&state.data_dir);
            add_device(&mut devices, device);
            let _ = save_devices(&state.data_dir, &devices);

            pairing_state.mark_token_retrieved(&code).await;

            Ok(Json(ConfirmPairingResponse {
                success: true,
                jwt: Some(jwt),
                device_id: Some(device_id),
                error: None,
            }))
        }
        Some(Err(err)) => Ok(Json(ConfirmPairingResponse {
            success: false,
            jwt: None,
            device_id: None,
            error: Some(err.to_string()),
        })),
        None => Ok(Json(ConfirmPairingResponse {
            success: false,
            jwt: None,
            device_id: None,
            error: Some("invalid code".to_string()),
        })),
    }
}

/// Get the pairing confirmation HTML page. No auth required.
pub async fn pairing_page(
    State(state): State<Arc<super::routes::AppState>>,
    Path(code): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let pairing_state = state
        .pairing
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let pc = pairing_state
        .get(&code)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    if pc.is_expired() {
        return Ok(Html(EXPIRED_PAGE.to_string()));
    }

    if pc.is_claimed() {
        return Ok(Html(ALREADY_CLAIMED_PAGE.to_string()));
    }

    // Render the confirmation page
    let page = CONFIRM_PAGE
        .replace("{code}", &code)
        .replace("{challenge}", &pc.challenge)
        .replace("{hostname}", &state.hostname);
    Ok(Html(page))
}

/// Retrieve the JWT for a claimed pairing code. No auth required.
/// The dashboard calls this after polling detects the code has been claimed.
#[derive(Serialize)]
pub struct TokenResponse {
    pub jwt: String,
    pub device_id: String,
    pub device_name: String,
}

pub async fn get_pairing_token(
    State(state): State<Arc<super::routes::AppState>>,
    Path(code): Path<String>,
) -> Result<Json<TokenResponse>, StatusCode> {
    let pairing_state = state
        .pairing
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let pc = pairing_state
        .get(&code)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    if !pc.is_claimed() {
        return Err(StatusCode::PRECONDITION_FAILED);
    }

    let device_id = pc.device_id.clone().ok_or(StatusCode::NOT_FOUND)?;
    let device_name = pc.claimed_by_device.clone().unwrap_or_default();

    // Generate fresh JWT for this device
    let now = chrono::Utc::now().timestamp() as u64;
    let claims = super::auth::Claims {
        sub: device_id.clone(),
        iat: now,
        exp: now + 86400 * 365, // 1 year validity
        role: "device".to_string(),
    };
    let jwt = state
        .auth
        .generate_token_from_claims(&claims)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    pairing_state.mark_token_retrieved(&code).await;

    Ok(Json(TokenResponse {
        jwt,
        device_id,
        device_name,
    }))
}

/// List paired devices. Requires auth.
#[derive(Serialize)]
pub struct DevicesResponse {
    pub devices: Vec<Device>,
}

pub async fn list_devices(
    State(state): State<Arc<super::routes::AppState>>,
) -> Json<DevicesResponse> {
    let devices = load_devices(&state.data_dir);
    Json(DevicesResponse {
        devices: devices.devices,
    })
}

/// Revoke a device by ID. Requires auth.
pub async fn revoke_device(
    State(state): State<Arc<super::routes::AppState>>,
    Path(device_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut devices = load_devices(&state.data_dir);
    remove_device(&mut devices, &device_id);
    save_devices(&state.data_dir, &devices).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "success": true })))
}

// ─── SHA-256 helper ─────────────────────────────────────────────

fn sha256_hex(input: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(input.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

// ─── HTML pages for browser confirmation ────────────────────────

const CONFIRM_PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Pair device — praxis</title>
<style>
  *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #18181b; color: #f4f4f5;
    display: flex; align-items: center; justify-content: center;
    min-height: 100vh; padding: 1rem;
  }
  .card {
    background: #27272a; border-radius: 12px; padding: 2rem;
    max-width: 420px; width: 100%; box-shadow: 0 4px 24px rgba(0,0,0,0.4);
  }
  h1 { font-size: 1.5rem; margin-bottom: 0.5rem; }
  p { color: #a1a1aa; margin-bottom: 1.5rem; line-height: 1.5; }
  .code-box {
    background: #3f3f46; border-radius: 8px; padding: 1rem;
    text-align: center; font-size: 1.75rem; font-weight: 700;
    letter-spacing: 0.25em; font-family: monospace; margin-bottom: 1.5rem;
  }
  .btn {
    display: block; width: 100%; padding: 0.75rem;
    border: none; border-radius: 8px; font-size: 1rem; font-weight: 600;
    cursor: pointer; transition: opacity 0.15s;
  }
  .btn-primary { background: #22c55e; color: #052e16; }
  .btn-primary:hover { opacity: 0.9; }
  .btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
  .error { color: #ef4444; margin-top: 0.75rem; font-size: 0.875rem; }
  .success { color: #22c55e; margin-top: 0.75rem; font-size: 0.875rem; }
  .footer { margin-top: 1.5rem; color: #71717a; font-size: 0.75rem; text-align: center; }
</style>
</head>
<body>
<div class="card">
  <h1>🔗 Pair device</h1>
  <p>A device is trying to connect to <strong>{hostname}</strong>. Confirm to allow access.</p>
  <div class="code-box">{code}</div>
  <button class="btn btn-primary" id="confirmBtn" onclick="confirmPair()">Confirm pairing</button>
  <div id="status"></div>
  <div class="footer">praxis — pairing code</div>
</div>
<script>
async function confirmPair() {
  const btn = document.getElementById('confirmBtn');
  const status = document.getElementById('status');
  btn.disabled = true;
  status.className = '';
  status.textContent = 'Confirming...';
  try {
    const res = await fetch('/api/pair/{code}/confirm', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ challenge: '{challenge}', device_name: 'Browser (' + navigator.platform + ')' })
    });
    const data = await res.json();
    if (data.success && data.jwt) {
      status.className = 'success';
      status.textContent = '✅ Device paired successfully! You can close this page.';
      btn.textContent = 'Paired ✓';
      // Optionally show JWT for manual copy
      if (data.jwt) {
        const jwtBox = document.createElement('div');
        jwtBox.style.cssText = 'margin-top:1rem;padding:0.5rem;background:#3f3f46;border-radius:4px;font-size:0.7rem;word-break:break-all;color:#a1a1aa';
        jwtBox.textContent = 'JWT: ' + data.jwt.substring(0, 32) + '...';
        status.after(jwtBox);
      }
    } else {
      status.className = 'error';
      status.textContent = '❌ ' + (data.error || 'Pairing failed');
      btn.disabled = false;
    }
  } catch (e) {
    status.className = 'error';
    status.textContent = '❌ Network error: ' + e.message;
    btn.disabled = false;
  }
}
</script>
</body>
</html>"#;

const EXPIRED_PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>Expired — praxis</title>
<style>body{font-family:system-ui,sans-serif;background:#18181b;color:#f4f4f5;display:flex;align-items:center;justify-content:center;min-height:100vh}.card{background:#27272a;border-radius:12px;padding:2rem;text-align:center}h1{font-size:1.5rem;color:#ef4444}p{color:#a1a1aa;margin-top:0.5rem}</style>
</head>
<body><div class="card"><h1>⏰ Code expired</h1><p>This pairing code has expired. Please generate a new one.</p></div></body>
</html>"#;

const ALREADY_CLAIMED_PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>Already paired — praxis</title>
<style>body{font-family:system-ui,sans-serif;background:#18181b;color:#f4f4f5;display:flex;align-items:center;justify-content:center;min-height:100vh}.card{background:#27272a;border-radius:12px;padding:2rem;text-align:center}h1{font-size:1.5rem;color:#22c55e}p{color:#a1a1aa;margin-top:0.5rem}</style>
</head>
<body><div class="card"><h1>✅ Already paired</h1><p>This code has already been claimed. You can close this page.</p></div></body>
</html>"#;
