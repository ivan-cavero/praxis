//! SSE transport — for remote MCP servers via Server-Sent Events.
//!
//! Connects to an MCP server over HTTP using SSE for the receive channel
//! and POST requests for sending.

use crate::protocol::messages::{
    JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
};
use reqwest::Client;
use tokio::sync::mpsc;

/// SSE-based transport for remote MCP servers.
pub struct SseTransport {
    client: Client,
    /// Base URL of the MCP server (e.g., "http://localhost:8080").
    _base_url: String,
    /// Endpoint for sending requests (POST).
    post_endpoint: String,
    /// Channel for received messages from the SSE stream.
    rx: mpsc::Receiver<JsonRpcMessage>,
    /// Abort handle for the SSE listener task.
    _sse_abort: Option<tokio::task::JoinHandle<()>>,
    next_id: u64,
}

impl SseTransport {
    /// Connect to a remote MCP server via SSE.
    pub async fn connect(url: &str) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let sse_url = format!("{}/sse", url);
        let post_endpoint = format!("{}/message", url);

        let (tx, rx) = mpsc::channel::<JsonRpcMessage>(256);

        // Spawn SSE listener task
        let sse_url_clone = sse_url.clone();
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            let response = client_clone
                .get(&sse_url_clone)
                .header("Accept", "text/event-stream")
                .send()
                .await;

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("SSE connection failed: {}", e);
                    return;
                }
            };

            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();
            use futures_util::StreamExt;

            while let Some(chunk_result) = bytes_stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Process SSE events (data: lines)
                        while let Some(line_end) = buffer.find('\n') {
                            let line = buffer[..line_end].trim().to_string();
                            buffer = buffer[line_end + 1..].to_string();

                            if let Some(data) = line.strip_prefix("data: ")
                                && let Ok(msg) = serde_json::from_str::<JsonRpcMessage>(data)
                                && tx.send(msg).await.is_err()
                            {
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("SSE stream error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            client,
            _base_url: url.to_string(),
            post_endpoint,
            rx,
            _sse_abort: Some(handle),
            next_id: 1,
        })
    }

    /// Send a request and wait for the response.
    pub async fn request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse, String> {
        let id = self.next_id;
        self.next_id += 1;

        let request = JsonRpcRequest::new(id, method, params);
        let json =
            serde_json::to_string(&request).map_err(|e| format!("Serialize error: {}", e))?;

        self.client
            .post(&self.post_endpoint)
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await
            .map_err(|e| format!("POST failed: {}", e))?;

        // Wait for response with matching id
        let timeout = std::time::Duration::from_secs(30);
        match tokio::time::timeout(timeout, self.rx.recv()).await {
            Ok(Some(JsonRpcMessage::Response(resp))) if resp.id == id => Ok(resp),
            Ok(Some(_)) => Err("Response id mismatch or wrong type".to_string()),
            Ok(None) => Err("Channel closed".to_string()),
            Err(_) => Err("Timeout waiting for response".to_string()),
        }
    }

    /// Send a notification (no response expected).
    pub async fn notify(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), String> {
        let notification = JsonRpcNotification::new(method, params);
        let json =
            serde_json::to_string(&notification).map_err(|e| format!("Serialize error: {}", e))?;

        self.client
            .post(&self.post_endpoint)
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await
            .map_err(|e| format!("POST failed: {}", e))?;

        Ok(())
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        if let Some(handle) = self._sse_abort.take() {
            handle.abort();
        }
    }
}

impl Drop for SseTransport {
    fn drop(&mut self) {
        self.disconnect();
    }
}
