//! Tool trait — unified interface for all tools (MCP, WASM, Native).

use async_trait::async_trait;
use serde_json::Value;

/// Cost estimate for executing a tool.
#[derive(Debug, Clone)]
pub struct ToolCost {
    pub estimated_tokens: u32,
    pub estimated_duration_ms: u64,
    pub requires_network: bool,
}

/// Output from executing a tool.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub success: bool,
    pub data: Value,
    pub duration_ms: u64,
    pub tokens_used: u32,
}

/// Unified Tool trait — every tool in the system implements this.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique name for this tool.
    fn name(&self) -> &str;

    /// Human-readable description.
    fn description(&self) -> &str;

    /// JSON Schema for the input parameters.
    fn input_schema(&self) -> Value;

    /// Execute the tool with the given input.
    async fn execute(&self, input: Value) -> crate::Result<ToolOutput>;

    /// Estimated cost for budgeting purposes.
    fn cost_estimate(&self) -> ToolCost;

    /// Source of this tool (mcp, wasm, native).
    fn source(&self) -> &str;
}
