//! MCP initialize handshake protocol.

use super::messages::{JsonRpcRequest, JsonRpcResponse};

/// MCP server capabilities
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerCapabilities {
    pub tools: Option<serde_json::Value>,
    pub resources: Option<serde_json::Value>,
    pub prompts: Option<serde_json::Value>,
}

/// MCP client info
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// Build an initialize request
pub fn build_initialize_request(id: u64, client_info: &ClientInfo) -> JsonRpcRequest {
    JsonRpcRequest::new(
        id,
        "initialize",
        Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": client_info.name,
                "version": client_info.version,
            }
        })),
    )
}

/// Build an initialized notification (sent after receiving server response)
pub fn build_initialized_notification() -> super::messages::JsonRpcNotification {
    super::messages::JsonRpcNotification::new("notifications/initialized", None)
}

/// Parse server capabilities from initialize response
pub fn parse_server_capabilities(response: &JsonRpcResponse) -> Result<ServerCapabilities, String> {
    let result = response.result.as_ref().ok_or("No result in response")?;
    let capabilities = result.get("capabilities")
        .ok_or("No capabilities in result")?;
    
    Ok(ServerCapabilities {
        tools: capabilities.get("tools").cloned(),
        resources: capabilities.get("resources").cloned(),
        prompts: capabilities.get("prompts").cloned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_initialize() {
        let client = ClientInfo {
            name: "project-x".to_string(),
            version: "0.1.0".to_string(),
        };
        let req = build_initialize_request(1, &client);
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, 1);
    }

    #[test]
    fn test_parse_capabilities() {
        let resp = JsonRpcResponse::success(1, serde_json::json!({
            "capabilities": {
                "tools": {"listChanged": true},
                "resources": {}
            }
        }));
        let caps = parse_server_capabilities(&resp).unwrap();
        assert!(caps.tools.is_some());
        assert!(caps.resources.is_some());
        assert!(caps.prompts.is_none());
    }
}