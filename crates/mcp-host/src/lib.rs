//! MCP Host — Manages Model Context Protocol servers as child processes.
//!
//! Discovers, connects, and exposes MCP server tools to the agent runtime.

pub mod transport;
pub mod protocol;
pub mod registry;

pub use registry::{McpHost, McpTool};
pub use transport::stdio::StdioTransport;
pub use protocol::messages::{JsonRpcMessage, JsonRpcRequest, JsonRpcResponse};