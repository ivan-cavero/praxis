//! JSON-RPC 2.0 message types.

pub enum JsonRpcMessage {
    Request { id: u64, method: String, params: serde_json::Value },
    Response { id: u64, result: serde_json::Value },
    Error { id: u64, code: i32, message: String, data: Option<serde_json::Value> },
    Notification { method: String, params: serde_json::Value },
}