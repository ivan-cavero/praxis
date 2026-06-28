//! MCP protocol: JSON-RPC 2.0, initialize, tools/list, tools/call.

pub mod messages;
pub mod initialize;

pub use messages::JsonRpcMessage;
pub use initialize::Initialize;