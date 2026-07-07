//! CoreRuntime: the central runtime that manages the entire system.
//!
//! The runtime owns the event bus, supervisor, loop controller, drift guard,
//! MCP host, and all optional subsystems (memory, event store, skills). It
//! exposes builder methods (`with_*`) for configuration and simple accessors.
//! The goal execution loop lives in [`crate::pipeline`].

use crate::actor;
use crate::bus::EventBus;
use crate::completion::CompletionCriterion;
use crate::config::ForgeConfig;
use crate::{Result, InjectedMessage};

use praxis_mcp_host::McpHost;
use praxis_memory::embedding::EmbeddingService;
use praxis_memory::episodic::{EpisodicMemory, SqliteBackend};
use std::sync::Arc;
use tokio::sync::RwLock;
use praxis_agent_traits::persistence::EventStore;


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
    pub consolidated_memory: Option<
        std::sync::Arc<tokio::sync::RwLock<praxis_memory::consolidated::ConsolidatedMemory>>,
    >,
    /// SummarizerAgent for generating session summaries.
    pub summarizer_agent: Option<crate::actor::roles::summarizer::SummarizerAgent>,
    /// Context budget for MemoryRAG allocation (budget-aware RAG injection).
    pub context_budget: Option<praxis_memory::context::ContextBudget>,
    /// Current context pressure (0.0–1.0) for drift detection (stored as atomic f32*1000).
    pub context_pressure: std::sync::Arc<std::sync::atomic::AtomicU32>,
    /// Result of the most recent gate evaluation, fed into drift MetricSamples.
    ///
    /// Drift metrics are recorded per-agent right after execution, but gates
    /// run later (only in Reviewing/Testing/SecurityScan phases). Without this,
    /// the ASI's gate_pass_rate dimension (weight 0.15) always saw 0% and
    /// permanently flagged drift. This field carries the last real gate result
    /// into the next iteration's metric samples (one-iteration lag).
    pub last_gate_passed: bool,
    /// Project name for checkpoint metadata (set by CLI/API when loading a project).
    pub project_name: Option<String>,
    /// Whether to write a human-readable STATE.md file to the working directory.
    /// Enabled by CLI, disabled in tests to avoid polluting the workspace.
    pub write_state_file: bool,
    /// Skills content loaded from SKILL.md files, injected into every agent's context.
    pub skills_content: Option<String>,
    /// Agent registry — resolves agent definitions from .md files (3 scopes).
    /// When set, agent system prompts come from the registry instead of TOML config.
    pub agent_registry: crate::agents::AgentRegistry,
    /// Hot memory — per-agent sliding windows for interaction history.
    /// Tracks recent interactions (input + output) per (session, agent) pair.
    /// Auto-evicts oldest when over count (50) or token (62,720) limits.
    pub hot_memory: Option<praxis_memory::hot::HotMemory>,
    /// Context manager — runs the compression pipeline (truncate tool results,
    /// compress history, reduce RAG, prune project context, EMC) when context
    /// exceeds the budget. Connected to the loop via `prepare_context_with_history`.
    pub context_manager: Option<praxis_memory::context::ContextManager>,
    /// Model override set by drift recovery (ModelUpgrade action).
    /// When set, all agents use this model instead of their configured one.
    /// Cleared at session start.
    pub model_override: Option<String>,
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
            last_gate_passed: true,
            project_name: None,
            write_state_file: false,
            skills_content: None,
            agent_registry: crate::agents::AgentRegistry::builtin_only(),
            hot_memory: None,
            context_manager: None,
            model_override: None,
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
    /// Set the project name for checkpoint metadata.
    pub fn with_project_name(mut self, name: String) -> Self {
        self.project_name = Some(name);
        self
    }
    /// after each checkpoint. Useful for live progress monitoring.
    pub fn with_state_file(mut self) -> Self {
        self.write_state_file = true;
        self
    }
    /// When set, agent system prompts come from the registry instead of TOML config.
    pub fn with_agent_registry(mut self, registry: crate::agents::AgentRegistry) -> Self {
        self.agent_registry = registry;
        self
    }
    /// The content is injected into every agent's task context.
    pub fn with_skills(mut self) -> Self {
        let mut content = String::new();

        // Load single SKILL.md if it exists
        if let Ok(skill) = std::fs::read_to_string("SKILL.md") {
            content.push_str("# Project Skills\n\n");
            content.push_str(&skill);
            content.push_str("\n\n");
        }

        // Load all .md files from skills/ directory if it exists
        if let Ok(entries) = std::fs::read_dir("skills") {
            let mut skill_files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                .collect();
            skill_files.sort_by_key(|e| e.path());

            for entry in skill_files {
                if let Ok(skill) = std::fs::read_to_string(entry.path()) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    content.push_str(&format!("# Skill: {}\n\n", name));
                    content.push_str(&skill);
                    content.push_str("\n\n");
                }
            }
        }

        if content.is_empty() {
            tracing::debug!(
                "No SKILL.md or skills/ directory found. Agents will run without skills."
            );
        } else {
            tracing::info!(
                "Loaded skills ({} bytes) for agent context injection.",
                content.len()
            );
            self.skills_content = Some(content);
        }

        self
    }
    pub fn with_builtin_skills(mut self, skill_ids: &[&str]) -> Self {
        let mut content = String::new();

        // Load built-in skills by ID
        let builtin_content = crate::skills::load_skills_by_ids(skill_ids);
        if !builtin_content.is_empty() {
            content.push_str("# Built-in Skills\n\n");
            content.push_str(&builtin_content);
        }

        // Load single SKILL.md if it exists
        if let Ok(skill) = std::fs::read_to_string("SKILL.md") {
            content.push_str("# Project Skills\n\n");
            content.push_str(&skill);
            content.push_str("\n\n");
        }

        // Load all .md files from skills/ directory if it exists
        if let Ok(entries) = std::fs::read_dir("skills") {
            let mut skill_files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                .collect();
            skill_files.sort_by_key(|e| e.path());

            for entry in skill_files {
                if let Ok(skill) = std::fs::read_to_string(entry.path()) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    content.push_str(&format!("# Skill: {}\n\n", name));
                    content.push_str(&skill);
                    content.push_str("\n\n");
                }
            }
        }

        if content.is_empty() {
            tracing::debug!("No skills loaded. Agents will run without skills.");
        } else {
            tracing::info!(
                "Loaded skills ({} bytes) for agent context injection.",
                content.len()
            );
            self.skills_content = Some(content);
        }

        self
    }
    pub fn with_memory(
        mut self,
        memory: EpisodicMemory,
        embedding: Option<EmbeddingService>,
    ) -> Self {
        let memory = std::sync::Arc::new(RwLock::new(memory));
        let mut keeper =
            crate::actor::roles::memory_keeper::MemoryKeeper::new(self.bus.clone(), memory.clone());
        if let Some(es) = embedding {
            keeper = keeper.with_embedding_service(std::sync::Arc::new(es));
        }
        let keeper = std::sync::Arc::new(keeper);
        let _handle = Arc::clone(&keeper).start();
        self.episodic_memory = Some(memory);
        self.memory_keeper = Some(keeper);
        self
    }
    pub fn with_default_memory(self) -> Self {
        self.with_memory(EpisodicMemory::default_store(), None)
            .with_consolidated_memory(100)
    }
    pub fn with_sqlite_memory(
        self,
        pool: r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
    ) -> Self {
        let sqlite = SqliteBackend::from_pool(pool);
        let memory = EpisodicMemory::default_store().with_sqlite(sqlite);
        let mut memory = memory;
        // Hydrate in-memory store from SQLite
        let _ = memory.load_from_sqlite();
        self.with_memory(memory, None).with_consolidated_memory(100)
    }
    pub fn with_consolidated_memory(mut self, max_summaries: usize) -> Self {
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
    /// Attach a context budget for budget-aware RAG injection and pressure tracking.
    ///
    /// The budget determines how many MemoryRAG tokens can be injected per agent call.
    /// Pass a ContextBudget matching your model context window (e.g., 128_000 for GPT-5).
    pub fn with_context_budget(mut self, budget: praxis_memory::context::ContextBudget) -> Self {
        self.context_budget = Some(budget);
        self
    }
    pub fn shutdown_handle(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.shutdown_requested.clone()
    }
    pub fn tokens_used(&self) -> u64 {
        self.loop_controller.tokens_used
    }
    pub fn cost_usd_for_session(&self) -> f64 {
        self.loop_controller.cost_usd
    }
    pub fn inject(&self, target_agent: &str, message_type: &str, content: &str) {
        let msg = InjectedMessage {
            target_agent: target_agent.to_string(),
            message_type: message_type.to_string(),
            content: content.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        if let Ok(mut guard) = self.injections.write() {
            guard.push(msg);
            tracing::info!(
                "Injected message for agent '{}' (type: {})",
                target_agent,
                message_type
            );
        }
    }
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
    pub async fn connect_mcp_servers(&mut self, config: &ForgeConfig) {
        for server_config in &config.mcp_servers {
            tracing::info!(
                "Connecting to MCP server: {} ({} {:?})",
                server_config.name,
                server_config.command,
                server_config.args
            );
            match self
                .mcp_host
                .connect_server(
                    &server_config.name,
                    &server_config.command,
                    &server_config.args,
                )
                .await
            {
                Ok(()) => {
                    let tools = self.mcp_host.tools_for(&server_config.name);
                    tracing::info!(
                        "MCP server '{}' connected with {} tools",
                        server_config.name,
                        tools.len()
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to connect MCP server '{}': {}",
                        server_config.name,
                        e
                    );
                }
            }
        }
    }
    pub async fn spawn_echo_agent(&self, name: &str) -> Result<actor::AgentHandle> {
        actor::spawn_echo(&self.supervisor, name).await
    }
    pub async fn echo_to(&self, child_name: &str, content: &str) -> Result<String> {
        actor::supervisor_echo_to(&self.supervisor, child_name, content).await
    }
    pub async fn list_agents(&self) -> Result<Vec<actor::AgentHandle>> {
        actor::list_children(&self.supervisor).await
    }
    pub async fn shutdown(&self) -> Result<()> {
        actor::shutdown_all(&self.supervisor).await
    }
}
