//! # praxis Core Runtime
//!
//! The heart of the system: actor model, state machine, orchestrator,
//! loop controller, drift detection, and context management.

pub mod actor;
pub mod api;
pub mod bus;
pub mod completion;
pub mod drift;
pub mod r#loop;
pub mod machine;
pub mod workflow;
pub mod orchestrator;

#[cfg(test)]
mod integration_tests;

// Re-exports for convenience
pub use actor::*;
pub use bus::EventBus;
pub use drift::*;
pub use r#loop::*;
pub use machine::*;
pub use workflow::*;
pub use orchestrator::{RoleConfig, RoleOverride, GoalConfig, ResolvedRole};
pub use orchestrator::roles::ResolvedRole as AgentRoleResolved;
pub use orchestrator::{Task, TaskResult, TaskStatus};
pub use completion::{
    CompletionCriterion, OutcomeResult, OutcomeVerifier,
    CodingOutcomeVerifier, ManualCompletionVerifier, default_coding_criterion,
};

use praxis_mcp_host::McpHost;
use praxis_vault::VaultService;
use praxis_agent_traits::persistence::EventStore;
use praxis_memory::episodic::EpisodicMemory;
use praxis_memory::embedding::EmbeddingService;
use tokio::sync::RwLock;
use std::sync::Arc;

use thiserror::Error;

// ─── Error Types ──────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Actor error: {0}")]
    Actor(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("State machine error: {0}")]
    StateMachine(String),

    #[error("Context error: {0}")]
    Context(String),

    #[error("Event bus error: {0}")]
    EventBus(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

// ─── Injection ─────────────────────────────────────────────────

/// A message injected mid-loop into a running agent session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InjectedMessage {
    pub target_agent: String,  // "coder", "all", etc.
    pub message_type: String,  // "instruction", "context", "correction", "halt"
    pub content: String,
    pub created_at: String,
}

// ─── Runtime ──────────────────────────────────────────────────

/// The central runtime that manages the entire system.
pub struct CoreRuntime {
    pub bus: EventBus,
    pub supervisor: ractor::ActorRef<actor::SupervisorMessage>,
    pub loop_controller: crate::r#loop::LoopController,
    pub drift_guard: crate::drift::DriftGuard,
    pub mcp_host: McpHost,
    pub pathology_detector: crate::r#loop::LoopPathologyDetector,
    pub completion_criterion: Option<CompletionCriterion>,
    /// Optional event store for checkpointing and event sourcing.
    pub event_store: Option<std::sync::Arc<praxis_persistence::SqliteEventStore>>,
    /// Current session ID (set when run_goal starts).
    pub session_id: Option<uuid::Uuid>,
    /// Flag set by Ctrl+C to request graceful shutdown.
    pub shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Pending mid-loop injections (drained before each agent execution).
    pub injections: std::sync::Arc<std::sync::RwLock<Vec<InjectedMessage>>>,
    /// Data directory for file-based injections and persistence.
    pub data_dir: Option<std::path::PathBuf>,
    /// Episodic memory for RAG context retrieval (shared with MemoryKeeper).
    pub episodic_memory: Option<std::sync::Arc<tokio::sync::RwLock<EpisodicMemory>>>,
    /// Background MemoryKeeper task handle (for lifecycle management).
    pub memory_keeper: Option<std::sync::Arc<crate::actor::roles::memory_keeper::MemoryKeeper>>,
    /// Consolidated memory for long-term summarization (shared with SummarizerAgent).
    pub consolidated_memory: Option<std::sync::Arc<tokio::sync::RwLock<praxis_memory::consolidated::ConsolidatedMemory>>>,
    /// SummarizerAgent for generating session summaries.
    pub summarizer_agent: Option<crate::actor::roles::summarizer::SummarizerAgent>,
    /// Context budget for MemoryRAG allocation (budget-aware RAG injection).
    pub context_budget: Option<praxis_memory::context::ContextBudget>,
    /// Current context pressure (0.0–1.0) for drift detection (stored as atomic f32*1000).
    pub context_pressure: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

/// Result of executing tool calls from agent output.
struct ToolExecResult {
    /// The full output with tool results appended.
    output: String,
    /// Info about each tool that was called.
    tools_called: Vec<ToolCallInfo>,
}

/// Info about a single tool call.
struct ToolCallInfo {
    server: String,
    tool_name: String,
    duration_ms: u64,
    success: bool,
}

impl CoreRuntime {
    /// Create and start a new CoreRuntime.
    pub async fn new() -> Result<Self> {
        let bus = EventBus::new();
        let supervisor = actor::Supervisor::spawn().await?;
        let loop_controller = crate::r#loop::LoopController::new();
        let drift_guard = crate::drift::DriftGuard::new();
        let mcp_host = McpHost::new("praxis");
        let pathology_detector = crate::r#loop::LoopPathologyDetector::new();

        Ok(Self {
            bus,
            supervisor,
            loop_controller,
            drift_guard,
            mcp_host,
            pathology_detector,
            completion_criterion: None,
            event_store: None,
            session_id: None,
            shutdown_requested: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            injections: std::sync::Arc::new(std::sync::RwLock::new(Vec::new())),
            data_dir: None,
            episodic_memory: None,
            memory_keeper: None,
            consolidated_memory: None,
            summarizer_agent: None,
            context_budget: None,
            context_pressure: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
        })
    }

    /// Attach a SQLite event store for checkpointing and event sourcing.
    pub fn with_event_store(mut self, store: praxis_persistence::SqliteEventStore) -> Self {
        self.event_store = Some(std::sync::Arc::new(store));
        self
    }

    /// Set a custom completion criterion (e.g., from CLI `--completion` flag).
    pub fn with_completion(mut self, criterion: CompletionCriterion) -> Self {
        self.completion_criterion = Some(criterion);
        self
    }

    /// Set the data directory for file-based injections and persistence.
    pub fn with_data_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.data_dir = Some(dir);
        self
    }

    /// Attach episodic memory and start the MemoryKeeper background indexer.
    ///
    /// The MemoryKeeper subscribes to the EventBus and automatically chunks,
    /// embeds, and stores agent outputs into episodic memory for RAG retrieval.
    ///
    /// Optionally configure an [`EmbeddingService`] for semantic vector generation.
    /// When omitted, chunks are stored without embeddings (text-search only).
    /// Attach episodic memory and start the MemoryKeeper background indexer.
    ///
    /// The MemoryKeeper subscribes to the EventBus and automatically chunks,
    /// embeds, and stores agent outputs into episodic memory for RAG retrieval.
    ///
    /// Optionally configure an [`EmbeddingService`] for semantic vector generation.
    /// When omitted, chunks are stored without embeddings (text-search only).
    pub fn with_memory(
        mut self,
        memory: EpisodicMemory,
        embedding: Option<EmbeddingService>,
    ) -> Self {
        let memory = std::sync::Arc::new(RwLock::new(memory));
        let mut keeper = crate::actor::roles::memory_keeper::MemoryKeeper::new(
            self.bus.clone(),
            memory.clone(),
        );
        if let Some(es) = embedding {
            keeper = keeper.with_embedding_service(std::sync::Arc::new(es));
        }
        let keeper = std::sync::Arc::new(keeper);
        let _handle = Arc::clone(&keeper).start();
        self.episodic_memory = Some(memory);
        self.memory_keeper = Some(keeper);
        self
    }

    /// Convenience: attach episodic memory with defaults (1000 chunks, no embedding).
    pub fn with_default_memory(self) -> Self {
        self.with_memory(EpisodicMemory::default_store(), None)
    }

    /// Attach consolidated memory and SummarizerAgent for long-term summarization.
    ///
    /// The SummarizerAgent reads chunks from the MemoryKeeper's episodic memory
    /// and generates structured `MemorySummary` objects that capture key decisions,
    /// errors, and learnings across a session.
    ///
    /// Call `summarize_current_session()` from `run_goal()` after each phase.
    pub fn with_consolidated_memory(
        mut self,
        max_summaries: usize,
    ) -> Self {
        let consolidated = std::sync::Arc::new(tokio::sync::RwLock::new(
            praxis_memory::consolidated::ConsolidatedMemory::new(max_summaries),
        ));

        // Wire the SummarizerAgent to the same episodic memory used by MemoryKeeper
        if let Some(ref episodic) = self.episodic_memory {
            let agent = crate::actor::roles::summarizer::SummarizerAgent::new(
                episodic.clone(),
                consolidated.clone(),
            );
            self.consolidated_memory = Some(consolidated);
            self.summarizer_agent = Some(agent);
            tracing::info!("SummarizerAgent attached for session summarization");
        } else {
            tracing::warn!(
                "with_consolidated_memory: no episodic memory configured. \
                 Call with_default_memory() or with_memory() first."
            );
        }

        self
    }

    /// Generate a consolidated summary for the current session.
    async fn summarize_current_session(&self) {
        let Some(ref agent) = self.summarizer_agent else { return };
        let Some(session_id) = self.session_id else { return };

        let sid = session_id.to_string();
        let _summary = agent.summarize_session(&sid, "default").await;

        // Publish event so the dashboard / logs can see the summary
        self.bus.publish(
            praxis_shared::protocol::MessageKind::CheckpointSaved(
                praxis_shared::protocol::CheckpointInfo {
                    session_id,
                    phase: praxis_shared::types::Phase::Finalizing,
                    iteration: self.loop_controller.iteration,
                    token_usage: praxis_shared::types::TokenUsage::new(0, 0),
                },
            ),
            "core",
        );
    }

    /// Late-binding: attach an embedding service to the existing MemoryKeeper.
    ///
    /// Called from `run_goal()` after the LLM provider is initialized, so the
    /// EmbeddingService wraps the real provider for semantic vector generation.
    pub async fn with_embedding_provider(&self, provider: std::sync::Arc<dyn praxis_agent_traits::provider::LLMProvider>) {
        let es = std::sync::Arc::new(EmbeddingService::new_default(provider));
        if let Some(ref keeper) = self.memory_keeper {
            keeper.set_embedding_service(es).await;
            tracing::info!("EmbeddingService attached to MemoryKeeper for semantic RAG");
        }
    }

    /// Set current session ID on the MemoryKeeper (fire-and-forget).
    fn propagate_session_to_memory(&self, session_id: uuid::Uuid) {
        let sid = session_id.to_string();
        if let Some(ref keeper) = self.memory_keeper {
            let keeper = Arc::clone(keeper);
            tokio::spawn(async move {
                keeper.set_session(Some(sid)).await;
            });
        }
    }

    /// Calculate the number of RAG chunks to retrieve based on available context budget.
    ///
    /// Uses the `MemoryRag` section allocation from the `ContextBudget` to determine
    /// how many chunks at `avg_chunk_tokens` (default 512) can fit. Returns at least 1
    /// and at most 20 chunks.
    fn calculate_rag_k(&self) -> usize {
        const AVG_CHUNK_TOKENS: usize = 512;
        const MAX_RAG_CHUNKS: usize = 20;
        const MIN_RAG_CHUNKS: usize = 1;

        let k = self.context_budget.as_ref().map_or(5, |budget| {
            let rag_budget = budget.section_budget(praxis_memory::context::Section::MemoryRag);
            let k = rag_budget / AVG_CHUNK_TOKENS;
            k.clamp(MIN_RAG_CHUNKS, MAX_RAG_CHUNKS)
        });
        k
    }

    /// Attach a context budget for budget-aware RAG injection and pressure tracking.
    ///
    /// The budget determines how many MemoryRAG tokens can be injected per agent call.
    /// Pass a ContextBudget matching your model context window (e.g., 128_000 for GPT-5).
    pub fn with_context_budget(mut self, budget: praxis_memory::context::ContextBudget) -> Self {
        self.context_budget = Some(budget);
        self
    }

    /// Update the current context pressure estimate (0.0–1.0, stored as f32*1000 in AtomicU32).
    ///
    /// Called after each agent execution to reflect how full the context window is.
    fn set_context_pressure(&self, pressure: f32) {
        let scaled = (pressure.clamp(0.0, 1.0) * 1000.0) as u32;
        self.context_pressure.store(scaled, std::sync::atomic::Ordering::Relaxed);
    }

    /// Evaluate drift and handle any recovery action.
    async fn evaluate_drift(&mut self, agent_id: Option<&str>) {
        // Force evaluate to get current ASI (doesn't trigger recovery)
        let Some(report) = self.drift_guard.force_evaluate(agent_id) else { return };

        // Publish drift alert for dashboard visibility
        self.bus.publish(
            praxis_shared::protocol::MessageKind::DriftAlert(
                praxis_shared::protocol::DriftAlert {
                    agent_id: agent_id.map(|s| s.to_string()),
                    old_asi: self.drift_guard.health_summary().overall_asi,
                    new_asi: report.asi_score,
                    dimension: "overall".to_string(),
                    severity: if report.asi_score < 40.0 {
                        praxis_shared::protocol::DriftSeverity::Critical
                    } else if report.asi_score < 60.0 {
                        praxis_shared::protocol::DriftSeverity::Warning
                    } else {
                        praxis_shared::protocol::DriftSeverity::Warning
                    },
                },
            ),
            "core",
        );

        // Trigger recovery if below threshold
        if report.asi_score < self.drift_guard.recovery_threshold {
            let action = self.drift_guard.recovery.evaluate(
                report.status.clone(),
                agent_id,
            );
            if let Some(action) = action {
                self.handle_recovery_action(&action, agent_id).await;
            }
        }
    }

    /// Execute a recovery action with actual side effects.
    async fn handle_recovery_action(
        &mut self,
        action: &crate::drift::RecoveryAction,
        agent_id: Option<&str>,
    ) {
        use crate::drift::RecoveryKind;

        tracing::warn!(
            "Recovery action: {:?} — {} (agent: {:?})",
            action.kind,
            action.reason,
            agent_id,
        );

        match action.kind {
            RecoveryKind::ForceConsolidation | RecoveryKind::ContextReset => {
                // Force consolidate current session into memory
                self.summarize_current_session().await;
                // Reset context pressure
                self.set_context_pressure(0.0);
            }

            RecoveryKind::ModelUpgrade => {
                // The RecoveryOrchestrator already tracked the tier upgrade.
                // Log it; the next iteration may use a different model tier.
                tracing::info!(
                    "Model upgrade recommended. Current tier: {:?}",
                    self.drift_guard.recovery.current_tier()
                );
            }

            RecoveryKind::SessionHandoff => {
                // Save checkpoint with current state, then reset
                if let Some(sid) = self.session_id {
                    self.save_checkpoint("handoff").await;
                    tracing::info!(
                        "Session handoff from {}. Creating fresh session with consolidated learnings.",
                        sid
                    );
                }
                // Reset drift tracking for the new session
                self.drift_guard.metrics = crate::drift::MetricsCollector::new();
                self.set_context_pressure(0.0);
            }

            RecoveryKind::KillSession => {
                tracing::error!("KillSession: severe drift, stopping execution.");
                self.shutdown_requested.store(true, std::sync::atomic::Ordering::SeqCst);
            }

            RecoveryKind::LogOnly | RecoveryKind::PauseAgent => {
                // LogOnly is already logged above.
                // PauseAgent: just warn; the loop will continue but the event is visible.
            }
        }
    }

    /// Get a handle to the shutdown flag. Set it to true to request graceful
    /// shutdown from outside the runtime (e.g., Ctrl+C handler).
    pub fn shutdown_handle(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.shutdown_requested.clone()
    }

    /// Inject a mid-loop message into a running session.
    ///
    /// The message will be picked up at the next agent execution boundary.
    /// Currently works in-process (same `CoreRuntime` instance). Cross-process
    /// injection requires the API server.
    pub fn inject(&self, target_agent: &str, message_type: &str, content: &str) {
        let msg = InjectedMessage {
            target_agent: target_agent.to_string(),
            message_type: message_type.to_string(),
            content: content.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        if let Ok(mut guard) = self.injections.write() {
            guard.push(msg);
            tracing::info!("Injected message for agent '{}' (type: {})", target_agent, message_type);
        }
    }

    /// Drain pending injection messages for a given agent.
    /// Returns formatted context string to prepend to the agent's task.
    ///
    /// Checks both in-memory queue and file-based injections (from the data
    /// directory) so injections work cross-process (CLI inject → file → runtime).
    fn drain_injections(&self, agent_name: &str) -> String {
        let mut relevant: Vec<InjectedMessage> = Vec::new();

        // 1. Drain in-memory queue
        if let Ok(mut guard) = self.injections.write() {
            guard.retain(|msg| {
                if msg.target_agent == agent_name || msg.target_agent == "all" {
                    relevant.push(msg.clone());
                    false
                } else {
                    true
                }
            });
        }

        // 2. Read file-based injections from data_dir/injections/
        if let Some(ref data_dir) = self.data_dir {
            let injections_dir = data_dir.join("injections");
            if injections_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&injections_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|ext| ext == "json") {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                if let Ok(msg) = serde_json::from_str::<InjectedMessage>(&content) {
                                    if msg.target_agent == agent_name || msg.target_agent == "all" {
                                        tracing::info!("Picked up file injection for '{}': {}", agent_name, msg.content);
                                        relevant.push(msg);
                                    }
                                }
                            }
                            // Remove file after reading (whether parsed or not)
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }

        if relevant.is_empty() {
            return String::new();
        }

        let mut parts: Vec<String> = relevant
            .iter()
            .map(|msg| {
                format!(
                    "[{} from operator]: {}",
                    msg.message_type.to_uppercase(),
                    msg.content
                )
            })
            .collect();
        parts.insert(0, "─── OPERATOR INJECTION ───".to_string());
        parts.push("─── END INJECTION ───".to_string());
        parts.join("\n")
    }

    /// Save a checkpoint of the current session state to the event store.
    ///
    /// Called after each phase transition. If the process crashes, `resume_goal`
    /// can load this checkpoint and continue from where it left off.
    async fn save_checkpoint(&self, goal: &str) {
        let Some(store) = &self.event_store else {
            return;
        };
        let Some(session_id) = self.session_id else {
            return;
        };

        let state = serde_json::json!({
            "goal": goal,
            "phase": format!("{:?}", self.loop_controller.state_machine.current()),
            "iteration": self.loop_controller.iteration,
            "phase_iterations": self.loop_controller.phase_iterations,
            "started_at": self.loop_controller.started_at.elapsed().as_secs(),
        });

        let snapshot = praxis_agent_traits::persistence::StoredSnapshot {
            aggregate_id: session_id,
            aggregate_type: "session".to_string(),
            state,
            version: self.loop_controller.iteration as i64,
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        if let Err(e) = store.save_snapshot(snapshot).await {
            tracing::warn!("Failed to save checkpoint: {}", e);
        } else {
            tracing::debug!(
                "Checkpoint saved: session={}, iteration={}",
                session_id,
                self.loop_controller.iteration
            );
        }
    }

    /// Load the last checkpoint for a session, if one exists.
    pub async fn load_checkpoint(
        &self,
        session_id: uuid::Uuid,
    ) -> Option<praxis_agent_traits::persistence::StoredSnapshot> {
        let store = self.event_store.as_ref()?;
        match store.get_snapshot(session_id).await {
            Ok(snap) => snap,
            Err(e) => {
                tracing::warn!("Failed to load checkpoint: {}", e);
                None
            }
        }
    }

    /// Connect MCP servers defined in the forge.toml config.
    pub async fn connect_mcp_servers(&mut self, config: &ForgeConfig) {
        for server_config in &config.mcp_servers {
            tracing::info!("Connecting to MCP server: {} ({} {:?})",
                server_config.name, server_config.command, server_config.args);
            match self.mcp_host.connect_server(
                &server_config.name,
                &server_config.command,
                &server_config.args,
            ).await {
                Ok(()) => {
                    let tools = self.mcp_host.tools_for(&server_config.name);
                    tracing::info!("MCP server '{}' connected with {} tools",
                        server_config.name, tools.len());
                }
                Err(e) => {
                    tracing::warn!("Failed to connect MCP server '{}': {}",
                        server_config.name, e);
                }
            }
        }
    }

    /// Prepare a context string describing available MCP tools for an agent.
    /// Used to inject tool descriptions into the agent's task context so the
    /// LLM knows what tools are available.
    fn prepare_tool_context(&self, tool_names: &[String]) -> String {
        if tool_names.is_empty() {
            return String::new();
        }

        let all_tools = self.mcp_host.all_tools();
        if all_tools.is_empty() {
            return String::new();
        }

        let mut lines: Vec<String> = Vec::new();
        lines.push("─── AVAILABLE TOOLS ───".to_string());
        lines.push(
            "You have access to the following tools. To call a tool, output a JSON block "
            .to_string() + "on its own line in this format:\n"
            + "```tool\n{\"server\": \"<server>\", \"tool\": \"<tool_name>\", \"arguments\": {...}}\n```\n"
            + "The tool results will be returned to you."
        );

        // Filter tools relevant to this agent (by server name matching tool_names)
        for tool_name in tool_names {
            let server_tools = self.mcp_host.tools_for(tool_name);
            for tool in &server_tools {
                lines.push(format!(
                    "  - [{}/{}] {}: {}",
                    tool.server_name, tool.name, tool.name, tool.description
                ));
            }
        }

        lines.push("─── END TOOLS ───".to_string());
        lines.join("\n")
    }

/// Parse and execute tool calls from agent output.
    /// Publishes `AgentOutput` streaming deltas for each tool call so the
    /// dashboard can show them in real-time.
    ///
    /// Returns the output with tool results appended, plus info about
    /// which tools were called.
    async fn execute_tool_calls(&mut self, output: &str) -> ToolExecResult {
        let mut result = output.to_string();
        let mut tools_called: Vec<ToolCallInfo> = Vec::new();
        let mut search_start = 0;

        loop {
            // Find the next ```tool marker
            let open_start = match output[search_start..].find("```tool") {
                Some(pos) => search_start + pos,
                None => break,
            };

            // Find the opening newline after ```tool
            let content_start = match output[open_start..].find('\n') {
                Some(pos) => open_start + pos + 1,
                None => break,
            };

            // Find the closing ```
            let close_marker = match output[content_start..].find("```") {
                Some(pos) => content_start + pos,
                None => break,
            };

            let json_str = &output[content_start..close_marker];
            match serde_json::from_str::<serde_json::Value>(json_str) {
                Ok(tool_call) => {
                    let server = tool_call["server"].as_str().unwrap_or("");
                    let tool_name = tool_call["tool"].as_str().unwrap_or("");
                    let args = tool_call
                        .get("arguments")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));

                    if server.is_empty() || tool_name.is_empty() {
                        search_start = close_marker + 3;
                        continue;
                    }

                    tracing::info!(
                        "Agent called tool: {}/{} with args: {:?}",
                        server, tool_name, args
                    );

                    let start = std::time::Instant::now();
                    let (tool_result, success) = match self.mcp_host.call_tool(server, tool_name, args).await
                    {
                        Ok(value) => {
                            (serde_json::to_string_pretty(&value)
                                .unwrap_or_else(|_| "{}".to_string()), true)
                        }
                        Err(e) => (format!("ERROR: {}", e), false),
                    };
                    let duration_ms = start.elapsed().as_millis() as u64;

                    tools_called.push(ToolCallInfo {
                        server: server.to_string(),
                        tool_name: tool_name.to_string(),
                        duration_ms,
                        success,
                    });

                    result = format!(
                        "{}\n\n─── TOOL RESULT: {}/{} ───\n{}\n─── END TOOL RESULT ───",
                        result, server, tool_name, tool_result
                    );

                    search_start = close_marker + 3;
                }
                Err(e) => {
                    tracing::warn!("Failed to parse tool call JSON: {}", e);
                    search_start = close_marker + 3;
                }
            }
        }

        ToolExecResult { output: result, tools_called }
    }

    /// Initialize a ProviderRouter from forge.toml providers + vault/env.
    ///
    /// For each provider in forge.toml, resolves the API key from:
    /// 1. VaultService (if key stored via Settings)
    /// 2. Environment variable (fallback for env:VAR_NAME references)
    /// 3. Literal key in config (warning: insecure)
    pub async fn init_providers(
        &self,
        config: &ForgeConfig,
        vault: Option<&VaultService>,
    ) -> praxis_providers::ProviderRouter {
        let mut router = praxis_providers::ProviderRouter::new();

        for (name, provider_cfg) in &config.providers {
            tracing::info!("Initializing provider: {} ({})", name, provider_cfg.base_url);

            // Resolve API key
            let api_key = self.resolve_api_key(&provider_cfg.api_key_ref, vault, name);

            if api_key.is_empty() {
                tracing::warn!("No API key for provider '{}'. Agent will use mock behavior.", name);
                continue;
            }

            let provider: std::sync::Arc<dyn praxis_providers::LLMProvider> =
                match provider_cfg.name.as_str() {
                    "nan" | "openai" | "openai_compat" => {
                        match praxis_providers::OpenAIProvider::new(
                            api_key,
                            provider_cfg.default_model.clone(),
                            Some(provider_cfg.base_url.clone()),
                            None,
                            None,
                        ) {
                            Ok(p) => std::sync::Arc::new(p),
                            Err(e) => {
                                tracing::warn!("Failed to init OpenAI provider '{}': {}. Using mock.", name, e.0);
                                continue;
                            }
                        }
                    }
                    "anthropic" => {
                        match praxis_providers::AnthropicProvider::new(
                            api_key,
                            provider_cfg.default_model.clone(),
                            Some(provider_cfg.base_url.clone()),
                            None,
                            None,
                        ) {
                            Ok(p) => std::sync::Arc::new(p),
                            Err(e) => {
                                tracing::warn!("Failed to init Anthropic provider '{}': {}. Using mock.", name, e.0);
                                continue;
                            }
                        }
                    }
                    "gemini" => {
                        match praxis_providers::GeminiProvider::new(
                            api_key,
                            provider_cfg.default_model.clone(),
                            Some(provider_cfg.base_url.clone()),
                            None,
                            None,
                        ) {
                            Ok(p) => std::sync::Arc::new(p),
                            Err(e) => {
                                tracing::warn!("Failed to init Gemini provider '{}': {}. Using mock.", name, e.0);
                                continue;
                            }
                        }
                    }
                    "ollama" => {
                        match praxis_providers::OllamaProvider::new(
                            provider_cfg.default_model.clone(),
                            Some(provider_cfg.base_url.clone()),
                        ) {
                            Ok(p) => std::sync::Arc::new(p),
                            Err(e) => {
                                tracing::warn!("Failed to init Ollama provider '{}': {}. Using mock.", name, e.0);
                                continue;
                            }
                        }
                    }
                    _ => {
                        match praxis_providers::OpenAIProvider::new(
                            api_key,
                            provider_cfg.default_model.clone(),
                            Some(provider_cfg.base_url.clone()),
                            None,
                            None,
                        ) {
                            Ok(p) => std::sync::Arc::new(p),
                            Err(e) => {
                                tracing::warn!("Failed to init provider '{}': {}. Using mock.", name, e.0);
                                continue;
                            }
                        }
                    }
                };

            router.register(name, provider, praxis_providers::ModelTier::Balanced);
            tracing::info!("Provider '{}' registered with model '{}'", name, provider_cfg.default_model);
        }

        router
    }

    /// Resolve an API key from vault, env, or config literal.
    fn resolve_api_key(&self, ref_str: &str, vault: Option<&VaultService>, provider_name: &str) -> String {
        // 1. Try vault first (keys stored via Settings)
        if let Some(v) = vault {
            if let Ok(Some(key)) = v.get(provider_name) {
                if !key.is_empty() {
                    tracing::info!("Loaded API key for '{}' from vault", provider_name);
                    return key;
                }
            }
        }

        // 2. Try env:VAR_NAME reference
        if let Some(var_name) = ref_str.strip_prefix("env:") {
            if let Ok(value) = std::env::var(var_name) {
                if !value.is_empty() {
                    tracing::info!("Loaded API key for '{}' from env:{}", provider_name, var_name);
                    return value;
                }
            }
        }

        // 3. Try literal key in config
        if !ref_str.is_empty() {
            if ref_str.starts_with("sk-") || ref_str.starts_with("xai-") {
                tracing::warn!("⚠️  Using literal API key in config for '{}' — consider using Settings page", provider_name);
            }
            return ref_str.to_string();
        }

        String::new()
    }

    /// Run a goal through the agent pipeline with a real iteration loop.
    ///
    /// The loop iterates: Planning → Designing → Implementing → Reviewing.
    /// If review gates fail → Fixing → Implementing → Reviewing (loop).
    /// If gates pass → Testing → SecurityScan → Finalizing → Completed.
    /// Stops when goal is complete or hard limits are reached.
    ///
    /// If `vault` is provided, providers are initialized from forge.toml + vault keys.
    /// When no forge.toml exists, runs using default mock agents.
    pub async fn run_goal(
        &mut self,
        goal: &str,
        config_path: Option<&std::path::Path>,
        vault: Option<&VaultService>,
    ) -> Result<GoalResult> {
        tracing::info!("Starting goal: {}", goal);

        let config = match config_path.map(load_forge_config) {
            Some(Ok(cfg)) => cfg,
            Some(Err(e)) => {
                tracing::warn!("Failed to load config: {}. Using defaults.", e);
                ForgeConfig::empty()
            }
            None => {
                tracing::info!("No forge.toml found. Using default mock agents.");
                ForgeConfig::empty()
            }
        };

        let provider_router = self.init_providers(&config, vault).await;

        // Wire embedding service to MemoryKeeper using the first available provider
        if let Some(provider) = provider_router.first_provider() {
            self.with_embedding_provider(provider).await;
        }

        // Connect MCP servers defined in forge.toml
        self.connect_mcp_servers(&config).await;

        if config.roles.is_empty() {
            tracing::info!("No roles defined in config. Using default coder role.");
        }

        // Register quality gates for review/test/security phases
        self.register_default_gates();

        // Set up outcome-based completion criterion (default: coding verifier)
        if self.completion_criterion.is_none() {
            self.completion_criterion = Some(default_coding_criterion());
        }
        self.pathology_detector.reset();

        // Assign a session ID
        self.session_id = Some(uuid::Uuid::new_v4());
        self.propagate_session_to_memory(self.session_id.unwrap());

        self.loop_controller.start();
        self.bus.publish(
            praxis_shared::protocol::MessageKind::SessionHeartbeat,
            "core",
        );

        self.loop_controller
            .advance(machine::phase::Phase::Planning)
            .map_err(CoreError::StateMachine)?;

        let mut results = Vec::new();
        let mut feedback = String::new();
        let mut current_phase = machine::phase::Phase::Planning;

        loop {
            if current_phase.is_terminal() {
                break;
            }

            // Check for graceful shutdown request (Ctrl+C)
            if self
                .shutdown_requested
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                tracing::info!("Shutdown requested. Saving checkpoint and stopping.");
                self.save_checkpoint(goal).await;
                break;
            }

            if let Some(violation) = self.loop_controller.check_limits() {
                tracing::warn!("Limit reached: {}. Stopping loop.", violation);
                self.save_checkpoint(goal).await;
                break;
            }

            tracing::info!(
                "Phase: {} (iteration {})",
                current_phase,
                self.loop_controller.iteration
            );

            // Check for parallel_reviewers in goal config (first matching goal)
            let parallel_count = config.goals.first().and_then(|g| g.parallel_reviewers);
            let phase_agents = get_agents_for_phase(&current_phase, &config, parallel_count);

            if phase_agents.len() > 1 && matches!(current_phase, machine::phase::Phase::Reviewing) {
                // ── Parallel execution for review phases ──
                let mut join_set = tokio::task::JoinSet::new();

                for role_config in &phase_agents {
                    let mut task = orchestrator::Task::new(
                        &role_config.name,
                        &role_config.model,
                        goal,
                    );

                    // Inject MCP tool context
                    let tool_context = self.prepare_tool_context(&role_config.tools);
                    if !tool_context.is_empty() {
                        task.context = tool_context;
                    }

                    // Inject pending injections
                    let injection = self.drain_injections(&role_config.name);
                    if !injection.is_empty() {
                        if task.context.is_empty() {
                            task.context = injection;
                        } else {
                            task.context = format!("{}\n\n{}", task.context, injection);
                        }
                    }

                    let resolved_role =
                        orchestrator::roles::ResolvedRole::resolve(role_config, None);
                    let agent = match provider_router.resolve(&role_config.model) {
                        Ok(provider) => {
                            crate::actor::roles::AgentFactory::create_with_provider_and_bus(
                                &resolved_role,
                                provider,
                                self.bus.clone(),
                            )
                        }
                        Err(_) => {
                            tracing::warn!(
                                "No provider for model '{}'. Using mock agent for '{}'.",
                                role_config.model,
                                role_config.name
                            );
                            crate::actor::roles::AgentFactory::create(&resolved_role)
                        }
                    };

                    let agent_name = role_config.name.clone();
                    self.bus.publish(
                        praxis_shared::protocol::MessageKind::AgentStarted {
                            agent: agent_name.clone(),
                            role: agent_name.clone(),
                            phase: current_phase,
                        },
                        "core",
                    );

                    // Spawn agent in parallel
                    join_set.spawn(async move {
                        let result = agent.execute(&task).await;
                        result
                    });
                }

                // Collect parallel results
                while let Some(join_result) = join_set.join_next().await {
                    match join_result {
                        Ok(raw_result) => {
                            let tool_exec = self.execute_tool_calls(&raw_result.content).await;
let result = if tool_exec.output != raw_result.content {
                                TaskResult {
                                    content: tool_exec.output,
                                    ..raw_result
                                }
                            } else {
                                raw_result
                            };

                            // Publish ToolCalled events for each tool that was invoked
                            for tc in &tool_exec.tools_called {
                                self.bus.publish(
                                    praxis_shared::protocol::MessageKind::ToolCalled {
                                        agent: result.agent_id.clone(),
                                        tool: format!("{}/{}", tc.server, tc.tool_name),
                                        duration_ms: tc.duration_ms,
                                        success: tc.success,
                                    },
                                    "core",
                                );
                            }

                            self.bus.publish(
                                praxis_shared::protocol::MessageKind::AgentCompleted {
                                    agent: result.agent_id.clone(),
                                    role: result.role.clone(),
                                    status: format!("{:?}", result.status),
                                    duration_ms: result.duration_ms,
                                    output_preview: result.content.chars().take(200).collect(),
                                },
                                "core",
                            );

                            tracing::info!(
                                "Agent {} completed: status={:?}, duration={}ms",
                                result.agent_id,
                                result.status,
                                result.duration_ms
                            );

                            // Publish token usage for live tracking
                            if result.token_usage.total > 0 {
                                self.bus.publish(
                                    praxis_shared::protocol::MessageKind::TokenUsed {
                                        provider: "unknown".into(),
                                        model: result.agent_id.clone(),
                                        input: result.token_usage.input,
                                        output: result.token_usage.output,
                                    },
                                    "core",
                                );
                            }

                            results.push(result);
                            let result = results.last().unwrap();

                            // ── Drift metrics recording (parallel) ──────────
                            let pressure = self.context_budget.as_ref().map_or(0.0, |budget| {
                                let used = result.token_usage.total as f32;
                                let limit = budget.hard_limit as f32;
                                if limit > 0.0 { (used / limit).clamp(0.0, 1.0) } else { 0.0 }
                            });
                            self.set_context_pressure(pressure);

                            let drift_sample = crate::drift::metrics::MetricSample {
                                iteration: self.loop_controller.iteration,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                latency_ms: result.duration_ms,
                                output_tokens: result.token_usage.output,
                                input_tokens: result.token_usage.input,
                                tool_calls: tool_exec.tools_called.len() as u32,
                                tool_errors: tool_exec.tools_called.iter().filter(|t| !t.success).count() as u32,
                                output_length_chars: result.content.len(),
                                gate_passed: false,
                                context_pressure: pressure,
                            };
                            if let Some(report) = self.drift_guard.record_and_evaluate(drift_sample, Some(&result.agent_id)) {
                                if let Some(action) = &report.recovery_action {
                                    self.handle_recovery_action(action, Some(&result.agent_id)).await;
                                }
                            }
                            // ── End drift metrics ──────────────────────────
                        }
                        Err(e) => {
                            tracing::warn!("Agent in parallel execution panicked: {}", e);
                        }
                    }
                }

                // Apply ConsensusConsolidator for multi-reviewer phases
                let review_results = extract_review_results(&results);
                let verdict = crate::orchestrator::verification::ConsensusConsolidator::consolidate(
                    results.clone(),
                    &crate::orchestrator::verification::ConsensusStrategy::AllPass,
                );
                tracing::info!(
                    "Consensus verdict: passed={}, confidence={:.1}%, reviewers={}",
                    verdict.passed, verdict.confidence, review_results.len()
                );
            } else {
                // ── Sequential execution (single agent or non-review phases) ──
                for role_config in &phase_agents {
                    let mut task = orchestrator::Task::new(
                        &role_config.name,
                        &role_config.model,
                        goal,
                    );

                    let has_feedback = !feedback.is_empty() && role_config.name == "coder";
                    if has_feedback {
                        task.context = feedback.clone();
                    }

                    // Drain pending injection messages for this agent
                    let injection = self.drain_injections(&role_config.name);
                    if !injection.is_empty() {
                        if task.context.is_empty() {
                            task.context = injection;
                        } else {
                            task.context = format!("{}\n\n{}", task.context, injection);
                        }
                        tracing::info!(
                            "Injected mid-loop message into task for agent '{}' at phase {:?}, iteration {}",
                            role_config.name,
                            current_phase,
                            self.loop_controller.iteration,
                        );
                        self.bus.publish(
                            praxis_shared::protocol::MessageKind::InjectionTriggered {
                                target: role_config.name.clone(),
                                phase: current_phase,
                                iteration: self.loop_controller.iteration,
                            },
                            "core",
                        );
                    }

                    // Inject MCP tool context into the task so the LLM knows
                    // what tools are available and how to call them
                    let tool_context = self.prepare_tool_context(&role_config.tools);
                    if !tool_context.is_empty() {
                        if task.context.is_empty() {
                            task.context = tool_context;
                        } else {
                            task.context = format!("{}\n\n{}", task.context, tool_context);
                        }
                    }

                    // ── MemoryRAG injection ─────────────────────────────────────
                    // Search episodic memory for relevant chunks and inject them
                    // into the agent's context (uses embedding-based search when
                    // an EmbeddingService is attached, keyword fallback otherwise).
                    if let Some(ref keeper) = self.memory_keeper {
                        let rag_k = self.calculate_rag_k();
                        let results = keeper.search_rag(goal, rag_k).await;
                        if !results.is_empty() {
                            let mut rag_parts: Vec<String> = Vec::new();
                            rag_parts.push("─── RELEVANT MEMORY ───".to_string());
                            for result in &results {
                                rag_parts.push(format!(
                                    "• [score={:.2}] {}",
                                    result.score,
                                    result.chunk.content
                                ));
                            }
                            rag_parts.push("─── END MEMORY ───".to_string());
                            let rag_context = rag_parts.join("\n\n");

                            if task.context.is_empty() {
                                task.context = rag_context;
                            } else {
                                task.context = format!("{}\n\n{}", task.context, rag_context);
                            }
                            tracing::debug!(
                                "Injected {} memory chunks into context for agent '{}'",
                                results.len(),
                                role_config.name
                            );
                        }
                    }
                    // ── End MemoryRAG injection ──────────────────────────────────

                    let resolved_role =
                        orchestrator::roles::ResolvedRole::resolve(role_config, None);
                    let agent = match provider_router.resolve(&role_config.model) {
                        Ok(provider) => {
                            crate::actor::roles::AgentFactory::create_with_provider_and_bus(
                                &resolved_role,
                                provider,
                                self.bus.clone(),
                            )
                        }
                        Err(_) => {
                            tracing::warn!(
                                "No provider for model '{}'. Using mock agent for '{}'.",
                                role_config.model,
                                role_config.name
                            );
                            crate::actor::roles::AgentFactory::create(&resolved_role)
                        }
                    };

                    // Publish agent start event for live streaming
                    self.bus.publish(
                        praxis_shared::protocol::MessageKind::AgentStarted {
                            agent: role_config.name.clone(),
                            role: role_config.name.clone(),
                            phase: current_phase,
                        },
                        "core",
                    );

                    let raw_result = if has_feedback {
                        agent.handle_feedback(&task, &feedback).await
                    } else {
                        agent.execute(&task).await
                    };

                    // Execute any tool calls found in the agent's output
                    let tool_exec = self.execute_tool_calls(&raw_result.content).await;
                    let mut result = if tool_exec.output != raw_result.content {
                        TaskResult {
                            content: tool_exec.output,
                            ..raw_result
                        }
                    } else {
                        raw_result
                    };

                    // Publish ToolCalled events for each tool that was invoked
                    for tc in &tool_exec.tools_called {
                        self.bus.publish(
                            praxis_shared::protocol::MessageKind::ToolCalled {
                                agent: result.agent_id.clone(),
                                tool: format!("{}/{}", tc.server, tc.tool_name),
                                duration_ms: tc.duration_ms,
                                success: tc.success,
                            },
                            "core",
                        );
                    }

                    // If tools were called, re-invoke the agent with tool results
                    // so the LLM can react to what the tools returned
                    if !tool_exec.tools_called.is_empty() {
                        tracing::info!(
                            "Agent {} called {} tool(s), re-invoking with results",
                            result.agent_id,
                            tool_exec.tools_called.len()
                        );
                        let follow_up_agent = match provider_router.resolve(&role_config.model) {
                            Ok(provider) => {
                                crate::actor::roles::AgentFactory::create_with_provider_and_bus(
                                    &resolved_role,
                                    provider,
                                    self.bus.clone(),
                                )
                            }
                            Err(_) => {
                                crate::actor::roles::AgentFactory::create(&resolved_role)
                            }
                        };
                        let follow_up_task = orchestrator::task::Task {
                            id: uuid::Uuid::new_v4().to_string(),
                            description: task.description.clone(),
                            context: result.content.clone(),
                            phase: task.phase.clone(),
                            max_iterations: task.max_iterations,
                            timeout: task.timeout,
                            role: task.role.clone(),
                            model: task.model.clone(),
                        };
                        let follow_up_result = follow_up_agent.execute(&follow_up_task).await;
                        result = TaskResult {
                            content: format!(
                                "{}\n\n─── FOLLOW-UP AFTER TOOL RESULTS ───\n{}",
                                result.content,
                                follow_up_result.content
                            ),
                            ..result
                        };
                    }

                    // Publish agent completion event
                    self.bus.publish(
                        praxis_shared::protocol::MessageKind::AgentCompleted {
                            agent: result.agent_id.clone(),
                            role: result.role.clone(),
                            status: format!("{:?}", result.status),
                            duration_ms: result.duration_ms,
                            output_preview: result.content.chars().take(200).collect(),
                        },
                        "core",
                    );

                    tracing::info!(
                        "Agent {} completed: status={:?}, duration={}ms",
                        result.agent_id,
                        result.status,
                        result.duration_ms
                    );

                    // Publish token usage for live tracking
                    if result.token_usage.total > 0 {
                        self.bus.publish(
                            praxis_shared::protocol::MessageKind::TokenUsed {
                                provider: "unknown".into(),
                                model: result.agent_id.clone(),
                                input: result.token_usage.input,
                                output: result.token_usage.output,
                            },
                            "core",
                        );
                    }

                    results.push(result);
                    let result = results.last().unwrap();

                    // ── Drift metrics recording ─────────────────────────
                    // Estimate context pressure from token usage vs budget
                    let pressure = self.context_budget.as_ref().map_or(0.0, |budget| {
                        let used = result.token_usage.total as f32;
                        let limit = budget.hard_limit as f32;
                        if limit > 0.0 { (used / limit).clamp(0.0, 1.0) } else { 0.0 }
                    });
                    self.set_context_pressure(pressure);

                    let drift_sample = crate::drift::metrics::MetricSample {
                        iteration: self.loop_controller.iteration,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        latency_ms: result.duration_ms,
                        output_tokens: result.token_usage.output,
                        input_tokens: result.token_usage.input,
                        tool_calls: tool_exec.tools_called.len() as u32,
                        tool_errors: tool_exec.tools_called.iter().filter(|t| !t.success).count() as u32,
                        output_length_chars: result.content.len(),
                        gate_passed: false,
                        context_pressure: pressure,
                    };
                    if let Some(report) = self.drift_guard.record_and_evaluate(drift_sample, Some(&result.agent_id)) {
                        if let Some(action) = &report.recovery_action {
                            self.handle_recovery_action(action, Some(&result.agent_id)).await;
                        }
                    }
                    // ── End drift metrics ──────────────────────────────
                }
            }

            // Evaluate gates for quality-check phases
            if matches!(
                current_phase,
                machine::phase::Phase::Reviewing
                    | machine::phase::Phase::Testing
                    | machine::phase::Phase::SecurityScan
            ) {
                let review_results = extract_review_results(&results);
                self.loop_controller.add_results(review_results);
                let gates_pass = self.loop_controller.all_gates_pass();

                if !gates_pass {
                    // Use CrossModelFeedbackLoop for structured feedback,
                    // fall back to consolidate_feedback for simple cases
                    if results.len() > 1 {
                        let fake_task = orchestrator::Task::new("reviewer", "", goal);
                        feedback = crate::orchestrator::verification::CrossModelFeedbackLoop::generate_feedback(
                            &results,
                            &fake_task,
                        );
                    } else {
                        feedback = consolidate_feedback(&results);
                    }

                    // Check if any gate has exceeded its retry limit
                    let phase = self.loop_controller.state_machine.current();
                    let gates_exceeded: Vec<&machine::gate::Gate> = self
                        .loop_controller
                        .gates
                        .gates_for(&phase)
                        .into_iter()
                        .filter(|g| g.is_exceeded())
                        .collect();

                    if !gates_exceeded.is_empty() {
                        let gate_names: Vec<&str> =
                            gates_exceeded.iter().map(|g| g.name.as_str()).collect();
                        tracing::warn!(
                            "Gate retry limit exceeded for: {}. Marking goal as failed.",
                            gate_names.join(", ")
                        );
                        current_phase = machine::phase::Phase::Failed;
                        self.loop_controller
                            .advance(machine::phase::Phase::Failed)
                            .map_err(CoreError::StateMachine)?;
                        break;
                    }

                    tracing::info!(
                        "Gate failed on {:?}. Going to Fixing. Feedback: {} chars",
                        current_phase,
                        feedback.len()
                    );
                    current_phase = machine::phase::Phase::Fixing;
                    self.loop_controller
                        .advance(machine::phase::Phase::Fixing)
                        .map_err(CoreError::StateMachine)?;
                    self.loop_controller.increment_iteration();
                    continue;
                } else {
                    if !feedback.is_empty() {
                        tracing::info!("Gates passed after fix. Clearing feedback.");
                        feedback.clear();
                    }
                }
            }

            // ── Drift evaluation + EMC ──────────────────────────────
            // After gate evaluation, check drift and emergency consolidate if needed.
            self.evaluate_drift(None).await;

            // EMC: emergency consolidation when context pressure > 85%
            let pressure = self.context_pressure.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
            if pressure > 0.85 {
                tracing::warn!(
                    "EMC triggered: context pressure {:.1}% > 85%. Forcing consolidation.",
                    pressure * 100.0
                );
                self.summarize_current_session().await;
                self.set_context_pressure(0.5);
                // Force a context reset via drift guard
                self.drift_guard.recovery.execute_context_reset(
                    &self.session_id.map_or("unknown".to_string(), |s| s.to_string()),
                    "EMC: emergency consolidation",
                    goal,
                );
            }
            // ── End drift evaluation + EMC ──────────────────────────

            // ── Pathology detection ──────────────────────────────
            // Check the last agent's output for destructive/stuck patterns.
            if let Some(last_result) = results.last() {
                let phase_str = format!("{:?}", current_phase);
                let token_count = last_result.token_usage.output;
                if let Some(alert) = self.pathology_detector.record_iteration(
                    self.loop_controller.iteration,
                    &last_result.content,
                    &phase_str,
                    Some(token_count),
                ) {
                    // Publish pathology alert on the EventBus
                    self.bus.publish(
                        praxis_shared::protocol::MessageKind::PathologyDetected(
                            praxis_shared::protocol::PathologyAlert {
                                kind: format!("{:?}", alert.kind),
                                severity: format!("{:?}", alert.severity),
                                details: alert.details.clone(),
                                action: format!("{:?}", alert.recommended_action),
                                iteration: alert.iteration,
                            },
                        ),
                        "core",
                    );
                    tracing::error!(
                        "Loop pathology detected: {:?} — {}",
                        alert.kind,
                        alert.details
                    );

                    // Fatal pathology → kill the loop immediately
                    if alert.severity == r#loop::PathologySeverity::Fatal {
                        tracing::error!(
                            "Fatal pathology: {}. Stopping loop immediately.",
                            alert.details
                        );
                        break;
                    }
                }
            }

            // ── Completion criterion (outcome-based) ─────────────
            // After quality-check phases, verify if the goal is actually achieved.
            if matches!(
                current_phase,
                machine::phase::Phase::Reviewing
                    | machine::phase::Phase::Testing
                    | machine::phase::Phase::SecurityScan
                    | machine::phase::Phase::Finalizing
            ) {
                if let Some(criterion) = &mut self.completion_criterion {
                    let outcome = criterion.evaluate(goal, &results).await;

                    match outcome {
                        completion::OutcomeResult::Achieved { evidence, .. } => {
                            tracing::info!(
                                "Goal achieved (verified by {}). Evidence: {}",
                                criterion.verifier_name(),
                                &evidence[..evidence.len().min(200)]
                            );
                            current_phase = machine::phase::Phase::Completed;
                            self.loop_controller
                                .advance(machine::phase::Phase::Completed)
                                .map_err(CoreError::StateMachine)?;
                            break;
                        }
                        completion::OutcomeResult::Exhausted { reason } => {
                            tracing::warn!(
                                "Goal exhausted: {}. Stopping loop.",
                                reason
                            );
                            break;
                        }
                        completion::OutcomeResult::NotAchieved { reason } => {
                            tracing::info!(
                                "Goal not yet achieved: {}. Continuing.",
                                reason
                            );
                        }
                    }
                }
            }

            let next_phase = get_next_phase(&current_phase);
            match self.loop_controller.advance(next_phase) {
                Ok(_transition) => {
                    self.bus.publish(
                        praxis_shared::protocol::MessageKind::PhaseChanged(
                            praxis_shared::protocol::PhaseTransition {
                                from: praxis_shared::types::Phase::Planning,
                                to: praxis_shared::types::Phase::Implementing,
                                condition: "automatic".to_string(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            },
                        ),
                        "core",
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to advance phase: {}", e);
                    break;
                }
            }

            current_phase = next_phase;
            self.loop_controller.increment_iteration();

            // Save checkpoint after each phase transition
            self.save_checkpoint(goal).await;
        }

        self.loop_controller.stop();

        // Save final checkpoint
        self.save_checkpoint(goal).await;

        // Summarize entire session into consolidated memory
        self.summarize_current_session().await;

        let total_duration: u64 = results.iter().map(|r| r.duration_ms).sum();
        let passed = current_phase == machine::phase::Phase::Completed;

        tracing::info!(
            "Goal '{}' finished: phases={}, iterations={}, agents={}, passed={}, duration={}ms",
            goal,
            self.loop_controller.state_machine.history().len(),
            self.loop_controller.iteration,
            results.len(),
            passed,
            total_duration,
        );

        Ok(GoalResult {
            goal: goal.to_string(),
            passed,
            agent_results: results,
            total_duration_ms: total_duration,
        })
    }

    /// Resume a goal from the last checkpoint.
    ///
    /// Loads the session state from the event store and continues the loop
    /// from where it left off. Returns `None` if no checkpoint exists.
    pub async fn resume_goal(
        &mut self,
        session_id: uuid::Uuid,
        config_path: Option<&std::path::Path>,
        vault: Option<&VaultService>,
    ) -> Result<Option<GoalResult>> {
        let checkpoint = match self.load_checkpoint(session_id).await {
            Some(snap) => snap,
            None => {
                tracing::info!("No checkpoint found for session {}", session_id);
                return Ok(None);
            }
        };

        let goal = checkpoint
            .state
            .get("goal")
            .and_then(|v| v.as_str())
            .unwrap_or("resumed goal")
            .to_string();

        let saved_iteration = checkpoint
            .state
            .get("iteration")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let saved_phase = checkpoint
            .state
            .get("phase")
            .and_then(|v| v.as_str())
            .unwrap_or("Planning");

        tracing::info!(
            "Resuming session {} at phase={}, iteration={}",
            session_id,
            saved_phase,
            saved_iteration
        );

        // Restore session state
        self.session_id = Some(session_id);
        self.propagate_session_to_memory(session_id);
        self.loop_controller.iteration = saved_iteration;

        // Re-register gates and completion criterion
        self.register_default_gates();
        if self.completion_criterion.is_none() {
            self.completion_criterion = Some(default_coding_criterion());
        }
        self.pathology_detector.reset();

        // Load config
        let config = match config_path.map(load_forge_config) {
            Some(Ok(cfg)) => cfg,
            _ => ForgeConfig::empty(),
        };
        let provider_router = self.init_providers(&config, vault).await;

        self.loop_controller.start();

        let mut results = Vec::new();
        let mut feedback = String::new();
        let mut current_phase = machine::phase::Phase::Planning;

        // Same loop as run_goal
        loop {
            if current_phase.is_terminal() {
                break;
            }

            if self
                .shutdown_requested
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                tracing::info!("Shutdown requested. Saving checkpoint and stopping.");
                self.save_checkpoint(&goal).await;
                break;
            }

            if let Some(violation) = self.loop_controller.check_limits() {
                tracing::warn!("Limit reached: {}. Stopping loop.", violation);
                self.save_checkpoint(&goal).await;
                break;
            }

            tracing::info!(
                "Phase: {} (iteration {})",
                current_phase,
                self.loop_controller.iteration
            );

            let parallel_count = config.goals.first().and_then(|g| g.parallel_reviewers);
            let phase_agents = get_agents_for_phase(&current_phase, &config, parallel_count);

            for role_config in &phase_agents {
                let mut task = orchestrator::Task::new(
                    &role_config.name,
                    &role_config.model,
                    &goal,
                );

                let has_feedback = !feedback.is_empty() && role_config.name == "coder";
                if has_feedback {
                    task.context = feedback.clone();
                }

                // Drain pending injection messages for this agent
                let injection = self.drain_injections(&role_config.name);
                if !injection.is_empty() {
                    if task.context.is_empty() {
                        task.context = injection;
                    } else {
                        task.context = format!("{}\n\n{}", task.context, injection);
                    }
                    tracing::info!("Injected message into task for agent '{}'", role_config.name);
                }

                let resolved_role =
                    orchestrator::roles::ResolvedRole::resolve(role_config, None);
                let agent = match provider_router.resolve(&role_config.model) {
                    Ok(provider) => {
                        crate::actor::roles::AgentFactory::create_with_provider_and_bus(
                            &resolved_role,
                            provider,
                            self.bus.clone(),
                        )
                    }
                    Err(_) => {
                        crate::actor::roles::AgentFactory::create(&resolved_role)
                    }
                };

                let result = if has_feedback {
                    agent.handle_feedback(&task, &feedback).await
                } else {
                    agent.execute(&task).await
                };

                results.push(result);
                let result = results.last().unwrap();

                // ── Drift metrics recording (resume_goal) ────────────
                let pressure = self.context_budget.as_ref().map_or(0.0, |budget| {
                    let used = result.token_usage.total as f32;
                    let limit = budget.hard_limit as f32;
                    if limit > 0.0 { (used / limit).clamp(0.0, 1.0) } else { 0.0 }
                });
                self.set_context_pressure(pressure);

                let drift_sample = crate::drift::metrics::MetricSample {
                    iteration: self.loop_controller.iteration,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    latency_ms: result.duration_ms,
                    output_tokens: result.token_usage.output,
                    input_tokens: result.token_usage.input,
                    tool_calls: 0,
                    tool_errors: 0,
                    output_length_chars: result.content.len(),
                    gate_passed: false,
                    context_pressure: pressure,
                };
                if let Some(report) = self.drift_guard.record_and_evaluate(drift_sample, Some(&result.agent_id)) {
                    if let Some(action) = &report.recovery_action {
                        self.handle_recovery_action(action, Some(&result.agent_id)).await;
                    }
                }
                // ── End drift metrics ──────────────────────────────
            }

            if matches!(
                current_phase,
                machine::phase::Phase::Reviewing
                    | machine::phase::Phase::Testing
                    | machine::phase::Phase::SecurityScan
            ) {
                let review_results = extract_review_results(&results);
                self.loop_controller.add_results(review_results);
                let gates_pass = self.loop_controller.all_gates_pass();

                if !gates_pass {
                    feedback = consolidate_feedback(&results);
                    current_phase = machine::phase::Phase::Fixing;
                    self.loop_controller
                        .advance(machine::phase::Phase::Fixing)
                        .map_err(CoreError::StateMachine)?;
                    self.loop_controller.increment_iteration();
                    continue;
                } else {
                    feedback.clear();
                }
            }

            // ── Drift evaluation + EMC (resume_goal) ──────────────
            self.evaluate_drift(None).await;

            let pressure = self.context_pressure.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
            if pressure > 0.85 {
                tracing::warn!(
                    "EMC triggered (resume): context pressure {:.1}% > 85%. Forcing consolidation.",
                    pressure * 100.0
                );
                self.summarize_current_session().await;
                self.set_context_pressure(0.5);
            }
            // ── End drift evaluation + EMC ──────────────────────────

            if let Some(last_result) = results.last() {
                let phase_str = format!("{:?}", current_phase);
                let token_count = last_result.token_usage.output;
                if let Some(alert) = self.pathology_detector.record_iteration(
                    self.loop_controller.iteration,
                    &last_result.content,
                    &phase_str,
                    Some(token_count),
                ) {
                    tracing::error!(
                        "Loop pathology detected: {:?} — {}",
                        alert.kind,
                        alert.details
                    );
                    if alert.severity == r#loop::PathologySeverity::Fatal {
                        break;
                    }
                }
            }

            if matches!(
                current_phase,
                machine::phase::Phase::Reviewing
                    | machine::phase::Phase::Testing
                    | machine::phase::Phase::SecurityScan
                    | machine::phase::Phase::Finalizing
            ) {
                if let Some(criterion) = &mut self.completion_criterion {
                    let outcome = criterion.evaluate(&goal, &results).await;
                    match outcome {
                        completion::OutcomeResult::Achieved { .. } => {
                            current_phase = machine::phase::Phase::Completed;
                            self.loop_controller
                                .advance(machine::phase::Phase::Completed)
                                .map_err(CoreError::StateMachine)?;
                            break;
                        }
                        completion::OutcomeResult::Exhausted { reason } => {
                            tracing::warn!("Goal exhausted: {}. Stopping.", reason);
                            break;
                        }
                        completion::OutcomeResult::NotAchieved { reason } => {
                            tracing::info!("Goal not yet achieved: {}. Continuing.", reason);
                        }
                    }
                }
            }

            let next_phase = get_next_phase(&current_phase);
            match self.loop_controller.advance(next_phase) {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("Failed to advance phase: {}", e);
                    break;
                }
            }

            current_phase = next_phase;
            self.loop_controller.increment_iteration();
            self.save_checkpoint(&goal).await;
        }

        self.loop_controller.stop();
        self.save_checkpoint(&goal).await;

        self.summarize_current_session().await;

        let total_duration: u64 = results.iter().map(|r| r.duration_ms).sum();
        let passed = current_phase == machine::phase::Phase::Completed;

        Ok(Some(GoalResult {
            goal,
            passed,
            agent_results: results,
            total_duration_ms: total_duration,
        }))
    }

    /// Register default quality gates for the standard pipeline.
    fn register_default_gates(&mut self) {
        use machine::gate::{Gate, GateEvaluator};

        self.loop_controller.gates.register(
            machine::phase::Phase::Reviewing,
            Gate::new("review.pass", GateEvaluator::AllAgentsPass, 3),
        );
        self.loop_controller.gates.register(
            machine::phase::Phase::SecurityScan,
            Gate::new("security.no_critical", GateEvaluator::NoCritical, 3),
        );
        self.loop_controller.gates.register(
            machine::phase::Phase::Testing,
            Gate::new("test.pass", GateEvaluator::AllAgentsPass, 3),
        );
    }

    /// Spawn a new EchoAgent via the supervisor (for testing).
    pub async fn spawn_echo_agent(&self, name: &str) -> Result<actor::AgentHandle> {
        actor::spawn_echo(&self.supervisor, name).await
    }

    /// Send an echo message to a named child agent.
    pub async fn echo_to(&self, child_name: &str, content: &str) -> Result<String> {
        actor::supervisor_echo_to(&self.supervisor, child_name, content).await
    }

    /// List all running child agents.
    pub async fn list_agents(&self) -> Result<Vec<actor::AgentHandle>> {
        actor::list_children(&self.supervisor).await
    }

    /// Shutdown all agents and stop the runtime.
    pub async fn shutdown(&self) -> Result<()> {
        actor::shutdown_all(&self.supervisor).await
    }
}

// ─── Goal Result ──────────────────────────────────────────────

/// Result of running a goal through the pipeline.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GoalResult {
    pub goal: String,
    pub passed: bool,
    pub agent_results: Vec<orchestrator::TaskResult>,
    pub total_duration_ms: u64,
}

// ─── Config ───────────────────────────────────────────────────

/// Parsed forge.toml configuration.
pub struct ForgeConfig {
    pub roles: std::collections::HashMap<String, orchestrator::RoleConfig>,
    pub goals: Vec<orchestrator::GoalConfig>,
    pub mcp_servers: Vec<McpServerConfig>,
    /// Provider definitions from [providers.*] sections. Key is provider name.
    pub providers: std::collections::HashMap<String, ProviderConfig>,
}

/// Provider configuration from forge.toml [providers.*].
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key_ref: String, // "env:VAR" | "vault:provider_name" | "literal-key"
    pub default_model: String,
}

/// MCP server configuration from forge.toml.
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

impl Default for ForgeConfig {
    fn default() -> Self {
        // Deprecated — use ForgeConfig::empty() instead.
        // Default impl kept for backward compatibility with tests.
        Self::empty()
    }
}

impl ForgeConfig {
    /// Create an empty config (no roles, no providers, no goals).
    /// Used when no forge.toml exists — agents run in mock mode.
    pub fn empty() -> Self {
        Self {
            roles: std::collections::HashMap::new(),
            goals: Vec::new(),
            mcp_servers: Vec::new(),
            providers: std::collections::HashMap::new(),
        }
    }
}

/// Load forge.toml configuration from a file.
pub fn load_forge_config(path: &std::path::Path) -> Result<ForgeConfig> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| CoreError::Config(format!("Failed to read {}: {}", path.display(), e)))?;

    let value: toml::Value = toml::from_str(&content)
        .map_err(|e| CoreError::Config(format!("Failed to parse {}: {}", path.display(), e)))?;

    let mut roles = std::collections::HashMap::new();
    let mut mcp_servers = Vec::new();
    let mut providers = std::collections::HashMap::new();

    // Parse roles from [roles.*] sections
    if let Some(roles_table) = value.get("roles").and_then(|v| v.as_table()) {
        for (name, role_value) in roles_table {
            let role = orchestrator::RoleConfig {
                name: name.clone(),
                description: role_value.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
                model: role_value.get("model").and_then(|v| v.as_str()).unwrap_or("gpt-4o").to_string(),
                temperature: role_value.get("temperature").and_then(|v| v.as_float()).unwrap_or(0.3) as f32,
                max_tokens: role_value.get("max_tokens").and_then(|v| v.as_integer()).unwrap_or(4096) as u32,
                system_prompt: role_value.get("system_prompt").and_then(|v| v.as_str()).map(|s| s.to_string()),
                tools: role_value.get("tools")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default(),
                context_profile: role_value.get("context_profile").and_then(|v| v.as_str()).map(|s| s.to_string()),
                context_priority: role_value.get("context_priority").and_then(|v| v.as_str()).map(|s| s.to_string()),
            };
            roles.insert(name.clone(), role);
        }
    }

    // Parse providers from [providers.*] sections
    if let Some(providers_table) = value.get("providers").and_then(|v| v.as_table()) {
        for (name, provider_value) in providers_table {
            let base_url = provider_value.get("base_url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://api.openai.com/v1")
                .to_string();
            let api_key_ref = provider_value.get("api_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let default_model = provider_value.get("default_model")
                .and_then(|v| v.as_str())
                .unwrap_or("gpt-4o")
                .to_string();

            providers.insert(name.clone(), ProviderConfig {
                name: name.clone(),
                base_url,
                api_key_ref,
                default_model,
            });
        }
    }

    // Parse MCP servers from [[mcp_servers]] sections
    if let Some(servers_array) = value.get("mcp_servers").and_then(|v| v.as_array()) {
        for server_value in servers_array {
            let name = server_value.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            let command = server_value.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let args = server_value.get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            mcp_servers.push(McpServerConfig { name, command, args });
        }
    }

    Ok(ForgeConfig {
        roles,
        goals: Vec::new(),
        mcp_servers,
        providers,
    })
}

/// Get the agents configured for a specific phase.
///
/// When no roles are configured (no forge.toml), uses default mock roles
/// so the pipeline still runs end-to-end.
fn get_agents_for_phase(
    phase: &machine::phase::Phase,
    config: &ForgeConfig,
    parallel_reviewers: Option<u32>,
) -> Vec<orchestrator::RoleConfig> {
    let lookup = |name: &str| -> Option<orchestrator::RoleConfig> {
        config.roles.get(name).cloned().or_else(|| {
            if config.roles.is_empty() {
                Some(default_role(name))
            } else {
                None
            }
        })
    };

    match phase {
        machine::phase::Phase::Planning | machine::phase::Phase::Designing => {
            lookup("architect").into_iter().collect()
        }
        machine::phase::Phase::Implementing => {
            lookup("coder").into_iter().collect()
        }
        machine::phase::Phase::Reviewing | machine::phase::Phase::Fixing => {
            let mut agents: Vec<orchestrator::RoleConfig> = Vec::new();
            if let Some(count) = parallel_reviewers.filter(|c| *c > 0) {
                // Create N parallel reviewer copies with slightly different prompts
                for index in 0..count {
                    let mut role = lookup("reviewer").unwrap_or_else(|| default_role("reviewer"));
                    role.name = format!("reviewer-{}", index + 1);
                    // Add a unique suffix to the prompt so each reviewer
                    // approaches the review from a different angle
                    let angles = [
                        "Focus on correctness, edge cases, and logic errors.",
                        "Focus on code style, readability, and best practices.",
                        "Focus on performance, resource usage, and optimization opportunities.",
                        "Focus on security vulnerabilities and unsafe code patterns.",
                        "Focus on test coverage and maintainability.",
                    ];
                    let angle = angles[index as usize % angles.len()];
                    role.system_prompt = role.system_prompt
                        .or_else(|| Some(default_role("reviewer").system_prompt.unwrap_or_default()))
                        .map(|p| format!("{}\n\nYour specific focus: {}", p, angle));
                    agents.push(role);
                }
            } else {
                if let Some(role) = lookup("reviewer") {
                    agents.push(role);
                }
            }
            agents
        }
        machine::phase::Phase::Testing | machine::phase::Phase::SecurityScan => {
            vec![lookup("tester"), lookup("security")]
                .into_iter()
                .flatten()
                .collect()
        }
        machine::phase::Phase::Finalizing => Vec::new(),
        _ => Vec::new(),
    }
}

/// Create a default role config for when no forge.toml exists.
fn default_role(name: &str) -> orchestrator::RoleConfig {
    orchestrator::RoleConfig {
        name: name.to_string(),
        description: Some(format!("Default {} agent (mock mode)", name)),
        model: "gpt-4o".to_string(),
        temperature: 0.3,
        max_tokens: 4096,
        system_prompt: Some(format!("You are a helpful {} assistant.", name)),
        tools: Vec::new(),
        context_profile: Some("balanced".to_string()),
        context_priority: Some("normal".to_string()),
    }
}

/// Get the next phase in the pipeline.
fn get_next_phase(current: &machine::phase::Phase) -> machine::phase::Phase {
    match current {
        machine::phase::Phase::Idle => machine::phase::Phase::Planning,
        machine::phase::Phase::Planning => machine::phase::Phase::Designing,
        machine::phase::Phase::Designing => machine::phase::Phase::Implementing,
        machine::phase::Phase::Implementing => machine::phase::Phase::Reviewing,
        machine::phase::Phase::Reviewing => machine::phase::Phase::Testing,
        machine::phase::Phase::Testing => machine::phase::Phase::SecurityScan,
        machine::phase::Phase::SecurityScan => machine::phase::Phase::Finalizing,
        machine::phase::Phase::Finalizing => machine::phase::Phase::Completed,
        machine::phase::Phase::Researching => machine::phase::Phase::Designing,
        machine::phase::Phase::Fixing => machine::phase::Phase::Implementing,
        _ => machine::phase::Phase::Completed,
    }
}

/// Extract review results from agent task results.
///
/// Converts the most recent reviewer/security/tester output into a
/// `ReviewResult` that the gate system can evaluate. Parses the agent's
/// text output for PASS/FAIL keywords.
fn extract_review_results(results: &[orchestrator::TaskResult]) -> Vec<machine::gate::ReviewResult> {
    results
        .iter()
        .rev()
        .find(|r| matches!(r.role.as_str(), "reviewer" | "security" | "tester"))
        .map(|r| {
            let content_lower = r.content.to_lowercase();
            let passed = !content_lower.contains("fail");

            let has_critical = content_lower.contains("critical")
                && !content_lower.contains("0 critical")
                && !content_lower.contains("no critical");

            let comments = if has_critical {
                vec![machine::gate::ReviewComment {
                    severity: machine::gate::Severity::Critical,
                    file: None,
                    line: None,
                    message: "Critical finding detected".to_string(),
                }]
            } else {
                Vec::new()
            };

            let coverage = if r.role == "tester" {
                if content_lower.contains("coverage") {
                    Some(0.85)
                } else {
                    Some(0.5)
                }
            } else {
                None
            };

            machine::gate::ReviewResult {
                agent: r.agent_id.clone(),
                passed,
                comments,
                coverage,
            }
        })
        .into_iter()
        .collect()
}

/// Consolidate feedback from failed gates into a single message for the coder.
fn consolidate_feedback(results: &[orchestrator::TaskResult]) -> String {
    let review_feedback: Vec<&str> = results
        .iter()
        .rev()
        .filter(|r| matches!(r.role.as_str(), "reviewer" | "security" | "tester"))
        .map(|r| r.content.as_str())
        .collect();

    if review_feedback.is_empty() {
        "Previous iteration had issues. Please review and fix.".to_string()
    } else {
        format!(
            "Previous review feedback:\n{}",
            review_feedback.join("\n---\n")
        )
    }
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_basic() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        bus.publish(
            praxis_shared::protocol::MessageKind::SessionHeartbeat,
            "test",
        );
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout")
            .expect("recv error");
        assert_eq!(event.source, "test");
    }

    #[tokio::test]
    async fn test_event_bus_multiple_subscribers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();
        bus.publish(
            praxis_shared::protocol::MessageKind::SessionHeartbeat,
            "test",
        );
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), rx1.recv()).await.expect("timeout").expect("recv error");
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), rx2.recv()).await.expect("timeout").expect("recv error");
    }

    #[tokio::test]
    async fn test_echo_agent() {
        let (actor_ref, _handle) = ractor::Actor::spawn(
            Some("test-echo".to_string()),
            actor::EchoAgent,
            "test-echo".to_string(),
        )
        .await
        .expect("Failed to spawn EchoAgent");

        let response = actor::echo(&actor_ref, "hello").await.expect("echo failed");
        assert!(response.contains("hello"));

        let pong = actor::ping(&actor_ref).await.expect("ping failed");
        assert_eq!(pong, "pong");

        let stats = actor::get_stats(&actor_ref).await.expect("stats failed");
        assert_eq!(stats.messages_processed, 3);
        assert_eq!(stats.agent_id, "test-echo");

        actor_ref.get_cell().stop(None);
    }

    #[tokio::test]
    async fn test_supervisor() {
        let supervisor = actor::Supervisor::spawn().await.expect("Failed to spawn Supervisor");

        let handle = actor::spawn_echo(&supervisor, "agent-1").await.expect("spawn failed");
        assert_eq!(handle.name, "agent-1");

        let handle2 = actor::spawn_echo(&supervisor, "agent-2").await.expect("spawn failed");
        assert_eq!(handle2.name, "agent-2");

        let response = actor::supervisor_echo_to(&supervisor, "agent-1", "test msg").await.expect("echo failed");
        assert!(response.contains("test msg"));

        let children = actor::list_children(&supervisor).await.expect("list failed");
        assert_eq!(children.len(), 2);

        let _ = actor::shutdown_all(&supervisor).await;
    }

    #[tokio::test]
    async fn test_core_runtime() {
        let runtime = CoreRuntime::new().await.expect("Failed to create runtime");

        let handle = runtime.spawn_echo_agent("test-agent").await.expect("spawn failed");
        assert_eq!(handle.name, "test-agent");

        let response = runtime.echo_to("test-agent", "hello runtime").await.expect("echo failed");
        assert!(response.contains("hello runtime"));

        let agents = runtime.list_agents().await.expect("list failed");
        assert_eq!(agents.len(), 1);

        let _ = runtime.shutdown().await;
    }

    #[tokio::test]
    async fn test_run_goal_completes_with_mock_agents() {
        let mut runtime = CoreRuntime::new().await.expect("Failed to create runtime");

        let result = runtime
            .run_goal("Create a hello world program", None, None)
            .await
            .expect("run_goal failed");

        assert!(!result.agent_results.is_empty(), "should have executed agents");
        assert!(result.passed, "goal should pass with mock agents (all gates pass)");

        let _ = runtime.shutdown().await;
    }

    #[tokio::test]
    async fn test_run_goal_respects_iteration_limit() {
        let mut runtime = CoreRuntime::new().await.expect("Failed to create runtime");
        runtime.loop_controller.limits.max_iterations_per_goal = 3;

        let result = runtime
            .run_goal("Limited goal", None, None)
            .await
            .expect("run_goal failed");

        assert!(
            runtime.loop_controller.iteration <= 3,
            "should not exceed max iterations: got {}",
            runtime.loop_controller.iteration
        );

        let _ = runtime.shutdown().await;
    }

    #[test]
    fn test_extract_review_results_pass() {
        let results = vec![orchestrator::TaskResult::success(
            "t1", "reviewer", "reviewer",
            "Review: PASS\nNo issues found", 100,
        )];
        let review = extract_review_results(&results);
        assert_eq!(review.len(), 1);
        assert!(review[0].passed, "should pass when content says PASS");
    }

    #[test]
    fn test_extract_review_results_fail() {
        let results = vec![orchestrator::TaskResult::success(
            "t1", "reviewer", "reviewer",
            "Review: FAIL\nCritical issue found", 100,
        )];
        let review = extract_review_results(&results);
        assert_eq!(review.len(), 1);
        assert!(!review[0].passed, "should fail when content says FAIL");
        assert!(!review[0].comments.is_empty(), "should have critical comments");
    }

    #[test]
    fn test_consolidate_feedback() {
        let results = vec![
            orchestrator::TaskResult::success("t1", "coder", "coder", "code here", 100),
            orchestrator::TaskResult::success("t2", "reviewer", "reviewer", "Fix the error handling", 100),
        ];
        let feedback = consolidate_feedback(&results);
        assert!(feedback.contains("Fix the error handling"), "should include reviewer feedback");
    }

    #[tokio::test]
    async fn test_checkpoint_saved_and_loaded() {
        let store = praxis_persistence::SqliteEventStore::in_memory()
            .expect("Failed to create store");

        let mut runtime = CoreRuntime::new()
            .await
            .expect("Failed to create runtime")
            .with_event_store(store);

        runtime
            .run_goal("Test checkpointing", None, None)
            .await
            .expect("run_goal failed");

        let session_id = runtime.session_id.expect("session_id should be set");
        let checkpoint = runtime.load_checkpoint(session_id).await;
        assert!(checkpoint.is_some(), "checkpoint should exist after run");

        let checkpoint = checkpoint.unwrap();
        assert_eq!(checkpoint.aggregate_type, "session");
        assert!(
            checkpoint.state.get("goal").is_some(),
            "checkpoint should contain goal"
        );
        assert!(
            checkpoint.state.get("iteration").is_some(),
            "checkpoint should contain iteration"
        );

        let _ = runtime.shutdown().await;
    }

    #[tokio::test]
    async fn test_graceful_shutdown_request() {
        let mut runtime = CoreRuntime::new()
            .await
            .expect("Failed to create runtime");

        let handle = runtime.shutdown_handle();

        // Simulate Ctrl+C before running
        handle.store(true, std::sync::atomic::Ordering::SeqCst);

        let result = runtime
            .run_goal("Should stop immediately", None, None)
            .await
            .expect("run_goal failed");

        // Should have stopped early due to shutdown request
        assert!(
            runtime.loop_controller.iteration <= 1,
            "should stop on first iteration check"
        );

        let _ = runtime.shutdown().await;
    }

    #[tokio::test]
    async fn test_resume_goal_no_checkpoint() {
        let store = praxis_persistence::SqliteEventStore::in_memory()
            .expect("Failed to create store");

        let mut runtime = CoreRuntime::new()
            .await
            .expect("Failed to create runtime")
            .with_event_store(store);

        let fake_session_id = uuid::Uuid::new_v4();
        let result = runtime
            .resume_goal(fake_session_id, None, None)
            .await
            .expect("resume_goal failed");

        assert!(result.is_none(), "should return None when no checkpoint exists");

        let _ = runtime.shutdown().await;
    }

    #[tokio::test]
    async fn test_resume_goal_from_checkpoint() {
        let store = praxis_persistence::SqliteEventStore::in_memory()
            .expect("Failed to create store");

        let mut runtime = CoreRuntime::new()
            .await
            .expect("Failed to create runtime")
            .with_event_store(store);

        // Run a goal to create a checkpoint
        runtime
            .run_goal("Test resume", None, None)
            .await
            .expect("run_goal failed");

        let session_id = runtime.session_id.expect("session_id should be set");

        // Reset runtime state
        runtime.loop_controller = crate::r#loop::LoopController::new();

        // Resume from the checkpoint
        let result = runtime
            .resume_goal(session_id, None, None)
            .await
            .expect("resume_goal failed");

        assert!(result.is_some(), "should resume from checkpoint");
        let result = result.unwrap();
        assert_eq!(result.goal, "Test resume");

        let _ = runtime.shutdown().await;
    }
}