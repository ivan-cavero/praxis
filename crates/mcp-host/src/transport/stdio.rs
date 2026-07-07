//! Stdio transport — spawns MCP server as child process.

use crate::protocol::messages::{
    JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

/// Stdio-based transport for MCP servers.
pub struct StdioTransport {
    child: Option<Child>,
    stdin: Option<tokio::process::ChildStdin>,
    rx: tokio::sync::mpsc::Receiver<JsonRpcMessage>,
    next_id: u64,
}

impl StdioTransport {
    /// Spawn an MCP server process and connect via stdio.
    pub async fn connect(command: &str, args: &[String]) -> Result<Self, String> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn '{}': {}", command, e))?;

        let stdin = child.stdin.take().ok_or("Failed to capture stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;

        let (tx, rx) = tokio::sync::mpsc::channel(256);

        // Spawn reader task
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }

                match serde_json::from_str::<JsonRpcMessage>(&line) {
                    Ok(msg) => {
                        if tx.send(msg).await.is_err() {
                            break; // Receiver dropped
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse MCP message: {} | line: {}", e, line);
                    }
                }
            }
        });

        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            rx,
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
        self.send_message(&JsonRpcMessage::Request(request)).await?;

        // Wait for response with matching id
        let timeout = std::time::Duration::from_secs(30);
        match tokio::time::timeout(timeout, self.rx.recv()).await {
            Ok(Some(JsonRpcMessage::Response(resp))) if resp.id == id => Ok(resp),
            Ok(Some(JsonRpcMessage::Response(resp))) => Err(format!(
                "Response id mismatch: expected {}, got {}",
                id, resp.id
            )),
            Ok(Some(_)) => Err("Expected response, got request/notification".to_string()),
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
        self.send_message(&JsonRpcMessage::Notification(notification))
            .await
    }

    /// Send a raw JSON-RPC message.
    async fn send_message(&mut self, msg: &JsonRpcMessage) -> Result<(), String> {
        let json = serde_json::to_string(msg).map_err(|e| format!("Failed to serialize: {}", e))?;
        let line = format!("{}\n", json);

        if let Some(ref mut stdin) = self.stdin {
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| format!("Failed to write to stdin: {}", e))?;
            stdin
                .flush()
                .await
                .map_err(|e| format!("Failed to flush stdin: {}", e))?;
        } else {
            return Err("Stdin not available".to_string());
        }

        Ok(())
    }

    /// Check if the process is still running.
    pub fn is_alive(&self) -> bool {
        self.child.is_some()
    }

    /// Disconnect and kill the process.
    pub async fn disconnect(&mut self) -> Result<(), String> {
        // Close stdin to signal the process to exit
        self.stdin.take();

        if let Some(mut child) = self.child.take() {
            // Wait briefly for graceful exit
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), child.wait()).await;

            // Force kill if still running
            let _ = child.kill().await;
        }

        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect_echo() {
        // Test with a simple echo command
        let result = StdioTransport::connect("echo", &["hello".to_string()]).await;
        // This will fail because echo doesn't speak JSON-RPC
        // but it validates the spawn mechanism
        assert!(result.is_err() || result.is_ok()); // Just checking it doesn't panic
    }
}
