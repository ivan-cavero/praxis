//! LLMProvider trait — abstraction over different LLM APIs.

use async_trait::async_trait;
use praxis_shared::types::{ModelInfo, TokenUsage};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// A single chat message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

/// Configuration for a chat completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub top_p: Option<f32>,
    pub stop_sequences: Option<Vec<String>>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            model: "gpt-5".to_string(),
            temperature: 0.3,
            max_tokens: 4096,
            top_p: None,
            stop_sequences: None,
            presence_penalty: None,
            frequency_penalty: None,
        }
    }
}

/// Response from a non-streaming chat completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub finish_reason: String,
    pub usage: TokenUsage,
    pub model: String,
}

/// A chunk of a streaming response.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    Delta(String),
    Done(TokenUsage),
    Error(String),
}

/// Receiver for streaming responses.
pub type StreamReceiver = mpsc::Receiver<StreamChunk>;

/// A tool call specification from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Cost information for a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCost {
    pub per_input_token: f64,
    pub per_output_token: f64,
    pub currency: String,
}

/// Tier classification for model selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelTier {
    Fast,
    Balanced,
    Capable,
    Cheapest,
}

/// Budget profiles for context window management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BudgetProfile {
    Balanced,
    Generous,
    Aggressive,
    Research,
}

/// The core LLM provider trait.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Send a chat completion request (non-streaming).
    async fn chat(
        &self,
        messages: &[ChatMessage],
        config: &ChatConfig,
    ) -> crate::Result<ChatResponse>;

    /// Send a streaming chat completion request.
    async fn stream(
        &self,
        messages: &[ChatMessage],
        config: &ChatConfig,
    ) -> crate::Result<StreamReceiver>;

    /// Generate embeddings for the given input strings.
    async fn embed(&self, input: &[String]) -> crate::Result<Vec<Vec<f32>>>;

    /// Count tokens for the given text using this provider's tokenizer.
    fn count_tokens(&self, text: &str) -> usize;

    /// Return information about the currently configured model.
    fn model_info(&self) -> ModelInfo;

    /// Return cost information for the configured model.
    fn model_cost(&self) -> ModelCost;

    /// Return the provider name (e.g., "openai", "anthropic").
    fn provider_name(&self) -> &str;
}