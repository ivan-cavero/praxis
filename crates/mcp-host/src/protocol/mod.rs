//! MCP protocol: JSON-RPC 2.0, initialize, tools/list, tools/call.

pub mod messages;
pub mod initialize;

pub use messages::{JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, JsonRpcNotification};
pub use initialize::{
    build_initialize_request, build_initialized_notification,
    parse_server_capabilities, ClientInfo, ServerCapabilities,
};