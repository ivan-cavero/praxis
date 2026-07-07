//! MCP protocol: JSON-RPC 2.0, initialize, tools/list, tools/call.

pub mod initialize;
pub mod messages;

pub use initialize::{
    ClientInfo, ServerCapabilities, build_initialize_request, build_initialized_notification,
    parse_server_capabilities,
};
pub use messages::{JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
