//! MCP Host — Manages Model Context Protocol servers.
//!
//! Handles server lifecycle: connect, negotiate, list tools, call tools, disconnect.

use crate::protocol::initialize::{build_initialize_request, parse_server_capabilities, ClientInfo, ServerCapabilities};
use crate::transport::stdio::StdioTransport;
use std::collections::HashMap;

/// Information about a connected MCP server.
pub struct McpServer {
    pub name: String,
    pub transport: StdioTransport,
    pub capabilities: ServerCapabilities,
    pub tools: Vec<McpTool>,
}

/// A tool provided by an MCP server.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub server_name: String,
}

/// The MCP Host manages all connected servers and their tools.
pub struct McpHost {
    servers: HashMap<String, McpServer>,
    client_info: ClientInfo,
}

impl McpHost {
    /// Create a new MCP Host.
    pub fn new(client_name: &str) -> Self {
        Self {
            servers: HashMap::new(),
            client_info: ClientInfo {
                name: client_name.to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }

    /// Connect to an MCP server and negotiate capabilities.
    pub async fn connect_server(&mut self, name: &str, command: &str, args: &[String]) -> Result<(), String> {
        tracing::info!("Connecting to MCP server '{}' ({} {:?})", name, command, args);

        // Spawn process and connect
        let mut transport = StdioTransport::connect(command, args).await?;

        // Initialize handshake
        let init_req = build_initialize_request(1, &self.client_info);
        let init_resp = transport.request("initialize", init_req.params).await?;

        // Parse capabilities
        let capabilities = parse_server_capabilities(&init_resp)
            .map_err(|e| format!("Failed to parse capabilities: {}", e))?;

        // Send initialized notification
        transport.notify("notifications/initialized", None).await?;

        tracing::info!("MCP server '{}' connected, capabilities: tools={}, resources={}, prompts={}",
            name,
            capabilities.tools.is_some(),
            capabilities.resources.is_some(),
            capabilities.prompts.is_some(),
        );

        // List tools if server supports them
        let tools = if capabilities.tools.is_some() {
            self.list_tools_from_server(&mut transport).await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let server = McpServer {
            name: name.to_string(),
            transport,
            capabilities,
            tools,
        };

        self.servers.insert(name.to_string(), server);
        Ok(())
    }

    /// List tools from a connected server.
    async fn list_tools_from_server(&self, transport: &mut StdioTransport) -> Result<Vec<McpTool>, String> {
        let response = transport.request("tools/list", None).await?;

        let tools_value = response.result.ok_or("No result in tools/list")?;
        let tools_array = tools_value.get("tools")
            .and_then(|v| v.as_array())
            .ok_or("No tools array in result")?;

        let tools: Vec<McpTool> = tools_array.iter().filter_map(|tool| {
            let name = tool.get("name")?.as_str()?.to_string();
            let description = tool.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let input_schema = tool.get("inputSchema").cloned().unwrap_or(serde_json::json!({}));

            Some(McpTool {
                name,
                description,
                input_schema,
                server_name: String::new(), // Will be set by caller
            })
        }).collect();

        Ok(tools)
    }

    /// Call a tool on a specific server.
    pub async fn call_tool(&mut self, server_name: &str, tool_name: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
        let server = self.servers.get_mut(server_name)
            .ok_or_else(|| format!("Server '{}' not found", server_name))?;

        let response = server.transport.request("tools/call", Some(serde_json::json!({
            "name": tool_name,
            "arguments": args,
        }))).await?;

        if let Some(error) = response.error {
            return Err(format!("Tool error: {}", error.message));
        }

        Ok(response.result.unwrap_or(serde_json::Value::Null))
    }

    /// Get all tools from all connected servers.
    pub fn all_tools(&self) -> Vec<McpTool> {
        self.servers.values()
            .flat_map(|s| s.tools.iter().cloned())
            .collect()
    }

    /// Get tools from a specific server.
    pub fn tools_for(&self, server_name: &str) -> Vec<McpTool> {
        self.servers.get(server_name)
            .map(|s| s.tools.clone())
            .unwrap_or_default()
    }

    /// List connected servers.
    pub fn list_servers(&self) -> Vec<(String, Vec<String>)> {
        self.servers.iter()
            .map(|(name, server)| {
                let tool_names: Vec<String> = server.tools.iter().map(|t| t.name.clone()).collect();
                (name.clone(), tool_names)
            })
            .collect()
    }

    /// Disconnect a specific server.
    pub async fn disconnect_server(&mut self, name: &str) -> Result<(), String> {
        if let Some(mut server) = self.servers.remove(name) {
            server.transport.disconnect().await?;
            tracing::info!("Disconnected from MCP server '{}'", name);
        }
        Ok(())
    }

    /// Disconnect all servers.
    pub async fn disconnect_all(&mut self) -> Result<(), String> {
        for (name, _) in self.servers.drain() {
            tracing::info!("Disconnecting '{}'", name);
        }
        Ok(())
    }

    /// Get server info.
    pub fn server_info(&self, name: &str) -> Option<(&ServerCapabilities, Vec<&McpTool>)> {
        self.servers.get(name).map(|s| {
            let tools: Vec<&McpTool> = s.tools.iter().collect();
            (&s.capabilities, tools)
        })
    }
}

impl Drop for McpHost {
    fn drop(&mut self) {
        for (_, mut server) in self.servers.drain() {
            let _ = server.transport.disconnect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_host_new() {
        let host = McpHost::new("praxis");
        assert!(host.servers.is_empty());
        assert_eq!(host.client_info.name, "praxis");
    }

    #[test]
    fn test_all_tools_empty() {
        let host = McpHost::new("test");
        assert!(host.all_tools().is_empty());
    }

    #[test]
    fn test_list_servers_empty() {
        let host = McpHost::new("test");
        assert!(host.list_servers().is_empty());
    }
}