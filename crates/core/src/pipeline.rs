//! Goal pipeline: run_goal, resume_goal, and supporting helpers.
//!
//! This module contains the main agent execution loop and all private
//! helpers it depends on (drift evaluation, tool execution, checkpointing,
//! context management, delegation, injection draining). Extracted from
//! the former god-module `lib.rs` to keep file sizes manageable.
//!
//! All `impl CoreRuntime` methods here are part of the same impl block
//! spread across [`crate::runtime`] and this module.

use crate::completion::{self, default_coding_criterion};
use crate::config::{ForgeConfig, load_forge_config};
use crate::r#loop;
use crate::machine;
use crate::orchestrator;
use crate::orchestrator::TaskResult;
use crate::runtime::CoreRuntime;
use crate::workflow;
use crate::{CoreError, InjectedMessage, Result};

use praxis_agent_traits::persistence::EventStore;
use praxis_vault::VaultService;
use std::sync::Arc;

use praxis_memory::embedding::EmbeddingService;

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

/// Result of running a goal through the pipeline.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GoalResult {
    pub goal: String,
    pub passed: bool,
    pub agent_results: Vec<orchestrator::TaskResult>,
    pub total_duration_ms: u64,
}

impl CoreRuntime {
    /// Generate a consolidated summary for the current session.
    async fn summarize_current_session(&self) {
        let Some(ref agent) = self.summarizer_agent else {
            return;
        };
        let Some(session_id) = self.session_id else {
            return;
        };

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

    /// Evict episodic memory chunks older than 30 days (TTL cleanup).
    ///
    /// Summaries in consolidated memory are kept forever; only raw episodic
    /// chunks are TTL-evicted. Called at session end so the next session
    /// starts with a clean episodic store.
    async fn cleanup_episodic_memory(&self) {
        let Some(ref episodic) = self.episodic_memory else {
            return;
        };
        const EPISODIC_TTL: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 60 * 60);
        let removed = episodic.write().await.cleanup(EPISODIC_TTL);
        if removed > 0 {
            tracing::info!(
                "Episodic memory TTL cleanup: removed {} chunks older than 30 days",
                removed
            );
        }
    }
    /// Late-binding: attach an embedding service to the existing MemoryKeeper.
    ///
    /// Called from `run_goal()` after the LLM provider is initialized, so the
    /// EmbeddingService wraps the real provider for semantic vector generation.
    pub async fn with_embedding_provider(
        &self,
        provider: std::sync::Arc<dyn praxis_agent_traits::provider::LLMProvider>,
    ) {
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

        self.context_budget.as_ref().map_or(5, |budget| {
            let rag_budget = budget.section_budget(praxis_memory::context::Section::MemoryRag);
            let k = rag_budget / AVG_CHUNK_TOKENS;
            k.clamp(MIN_RAG_CHUNKS, MAX_RAG_CHUNKS)
        })
    }
    /// Update the current context pressure estimate (0.0–1.0, stored as f32*1000 in AtomicU32).
    ///
    /// Called after each agent execution to reflect how full the context window is.
    fn set_context_pressure(&self, pressure: f32) {
        let scaled = (pressure.clamp(0.0, 1.0) * 1000.0) as u32;
        self.context_pressure
            .store(scaled, std::sync::atomic::Ordering::Relaxed);
    }
    /// Compute context pressure from accumulated session tokens vs the budget.
    ///
    /// Pressure = `tokens_used` (session-wide, accumulated across all agents and
    /// iterations) / `hard_limit`. This grows over the session so EMC (emergency
    /// consolidation at >85%) actually triggers when the context window fills up.
    ///
    /// Previously this used a single LLM call's tokens (`result.token_usage.total`),
    /// which never reached the threshold — a single call is ~1–5k tokens vs an
    /// 89,600 hard_limit, so pressure was always ~0.01 and EMC was dead code.
    fn compute_context_pressure(&self) -> f32 {
        self.context_budget.as_ref().map_or(0.0, |budget| {
            let used = self.loop_controller.tokens_used as f32;
            let limit = budget.hard_limit as f32;
            if limit > 0.0 {
                (used / limit).clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
    }
    /// Truncate `task.context` from the front if it exceeds the context budget.
    ///
    /// Uses the real BPE tokenizer (cl100k_base) for accurate token counting.
    /// Keeps the most recent content (front-truncation) so the agent always sees
    /// the latest state. Logs a warning when truncation occurs.
    fn clamp_context_to_budget(&self, task: &mut orchestrator::Task) {
        let Some(budget) = &self.context_budget else {
            return;
        };
        let counter = praxis_memory::context::TokenCounter::default_token_counter();
        let token_count = counter.count_tokens(&task.context);
        if token_count as usize > budget.hard_limit {
            let max_chars = (budget.hard_limit as f32 * 4.0) as usize;
            if task.context.len() > max_chars {
                let start = task.context.len() - max_chars;
                let truncated_start = task.context[start..]
                    .find('\n')
                    .map(|pos| start + pos + 1)
                    .unwrap_or(start);
                let removed = truncated_start;
                task.context = task.context[truncated_start..].to_string();
                tracing::warn!(
                    "Context for agent '{}' exceeded budget ({} tokens > {} limit), \
                     truncated {} chars from front",
                    task.role,
                    token_count,
                    budget.hard_limit,
                    removed
                );
            }
        }
    }
    /// Return the effective model for an agent, applying any drift-recovery override.
    fn effective_model<'a>(&'a self, configured: &'a str) -> &'a str {
        self.model_override.as_deref().unwrap_or(configured)
    }
    /// Push an agent interaction (input + output) to the sliding window.
    ///
    /// Called after each agent execution. The sliding window auto-evicts the
    /// oldest interaction when over its count (50) or token (62,720) limit.
    /// This gives `ContextManager::prepare()` real history to compress.
    fn push_agent_interaction(&self, agent_id: &str, input: &str, output: &str, token_count: u32) {
        let Some(hot_memory) = &self.hot_memory else {
            return;
        };
        let Some(session_id) = self.session_id else {
            return;
        };

        let counter = praxis_memory::context::TokenCounter::default_token_counter();
        let input_tokens = counter.count_tokens(input);
        let output_tokens = counter.count_tokens(output);
        let total_tokens = token_count.max(input_tokens + output_tokens);

        hot_memory.push_interaction(
            &session_id.to_string(),
            agent_id,
            praxis_memory::hot::Interaction {
                role: "assistant".to_string(),
                content: format!("INPUT:\n{input}\n\nOUTPUT:\n{output}"),
                token_count: total_tokens,
                timestamp: chrono::Utc::now().to_rfc3339(),
            },
        );
    }
    /// Inject compressed interaction history into `task.context`.
    ///
    /// Builds a `ContextWindow` from the agent's sliding window history plus
    /// the current task context, then runs `ContextManager::prepare()` which
    /// triggers the full compression pipeline (truncate tool results → compress
    /// history → reduce RAG → prune project context → EMC) when over budget.
    /// The compressed history is prepended to `task.context` as a
    /// "--- Recent History ---" section.
    fn prepare_context_with_history(&mut self, task: &mut orchestrator::Task, agent_id: &str) {
        let Some(hot_memory) = &self.hot_memory else {
            return;
        };
        let Some(context_manager) = &mut self.context_manager else {
            return;
        };
        let Some(session_id) = self.session_id else {
            return;
        };

        let window = hot_memory.get_context(&session_id.to_string(), agent_id);
        let Some(window) = window else {
            return;
        };
        if window.is_empty() {
            return;
        }

        // Build a ContextWindow from the sliding window's interactions
        let mut ctx_window = praxis_memory::context::ContextWindow::new();
        for interaction in window.interactions() {
            ctx_window.push(praxis_memory::context::Message {
                role: interaction.role.clone(),
                content: interaction.content.clone(),
            });
        }

        // Run the compression pipeline (triggers EMC when pressure > 85%)
        context_manager.prepare(&mut ctx_window);

        // Inject compressed history as a "Recent History" section
        if !ctx_window.is_empty() {
            let history = ctx_window
                .messages
                .iter()
                .map(|m| m.content.clone())
                .collect::<Vec<_>>()
                .join("\n\n---\n\n");

            if !history.is_empty() {
                let history_section =
                    format!("--- Recent History ---\n{history}\n--- End History ---");
                task.context = if task.context.is_empty() {
                    history_section
                } else {
                    format!("{history_section}\n\n{}", task.context)
                };
            }
        }
    }
    /// Evaluate drift and handle any recovery action.
    async fn evaluate_drift(&mut self, agent_id: Option<&str>) {
        // Force evaluate to get current ASI (doesn't trigger recovery)
        let Some(report) = self.drift_guard.force_evaluate(agent_id) else {
            return;
        };

        // Publish drift alert for dashboard visibility
        self.bus.publish(
            praxis_shared::protocol::MessageKind::DriftAlert(praxis_shared::protocol::DriftAlert {
                agent_id: agent_id.map(|s| s.to_string()),
                old_asi: self.drift_guard.health_summary().overall_asi,
                new_asi: report.asi_score,
                dimension: "overall".to_string(),
                severity: if report.asi_score < 40.0 {
                    praxis_shared::protocol::DriftSeverity::Critical
                } else {
                    praxis_shared::protocol::DriftSeverity::Warning
                },
            }),
            "core",
        );

        // Trigger recovery if below threshold
        if report.asi_score < self.drift_guard.recovery_threshold {
            let action = self
                .drift_guard
                .recovery
                .evaluate(report.status.clone(), agent_id);
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
                // Upgrade to a more capable model. The override applies to all
                // agents in subsequent iterations until the session ends.
                let current = self
                    .model_override
                    .clone()
                    .unwrap_or_else(|| "gpt-4o".to_string());
                let upgraded = match current.as_str() {
                    "gpt-4o-mini" | "gpt-4o" => "gpt-5",
                    "claude-3-haiku" | "claude-3-5-haiku" => "claude-3-5-sonnet",
                    "claude-3-5-sonnet" => "claude-3-5-opus",
                    "gemini-1.5-flash" | "gemini-1.5-pro" => "gemini-2.0-pro",
                    _ => current.as_str(), // Already at max or unknown — no upgrade
                };
                if upgraded != current.as_str() {
                    self.model_override = Some(upgraded.to_string());
                    tracing::info!("Model upgrade: {} → {} (drift recovery)", current, upgraded);
                } else {
                    tracing::warn!(
                        "Model upgrade requested but already at max tier: {}",
                        current
                    );
                }
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
                self.shutdown_requested
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }

            RecoveryKind::LogOnly => {
                // LogOnly is already logged above.
            }
        }
    }
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
            if injections_dir.is_dir()
                && let Ok(entries) = std::fs::read_dir(&injections_dir)
            {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "json")
                        && let Ok(content) = std::fs::read_to_string(&path)
                    {
                        if let Ok(msg) = serde_json::from_str::<InjectedMessage>(&content) {
                            if msg.target_agent == agent_name || msg.target_agent == "all" {
                                tracing::info!(
                                    "Picked up file injection for '{}': {}",
                                    agent_name,
                                    msg.content
                                );
                                relevant.push(msg);
                                // Only delete the file if it was successfully parsed and matched
                                let _ = std::fs::remove_file(&path);
                            }
                        } else {
                            tracing::warn!("Failed to parse injection file: {}", path.display());
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
            // Even without a store, write the state file for human/agent readability
            if self.write_state_file {
                self.write_state_file(goal);
            }
            return;
        };
        let Some(session_id) = self.session_id else {
            return;
        };

        let state = serde_json::json!({
            "goal": goal,
            "project": self.project_name.clone().unwrap_or_default(),
            "phase": format!("{:?}", self.loop_controller.state_machine.current()),
            "iteration": self.loop_controller.iteration,
            "phase_iterations": self.loop_controller.phase_iterations,
            "started_at": self.loop_controller.started_at.elapsed().as_secs(),
            "context_pressure": self.context_pressure.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0,
            "tokens_used": self.loop_controller.tokens_used,
            "cost_usd": self.loop_controller.cost_usd,
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

        // Always write the human/agent-readable state file (if enabled)
        if self.write_state_file {
            self.write_state_file(goal);
        }
    }
    /// Write a human- and agent-readable `STATE.md` file to the working directory.
    ///
    /// This file captures the current loop state in a format that both humans
    /// and agents can read: what's done, what failed, what's next. It serves as
    /// a lightweight progress indicator and crash-recovery reference.
    fn write_state_file(&self, goal: &str) {
        let phase = self.loop_controller.state_machine.current();
        let iteration = self.loop_controller.iteration;
        let elapsed = self.loop_controller.started_at.elapsed();
        let tokens_used = self.loop_controller.tokens_used;
        let cost = self.loop_controller.cost_usd;
        let session = self.session_id.map(|s| s.to_string()).unwrap_or_default();
        let project = self.project_name.as_deref().unwrap_or("unnamed");

        let valid_transitions: Vec<String> = self
            .loop_controller
            .state_machine
            .valid_transitions()
            .iter()
            .map(|p| format!("{:?}", p))
            .collect();

        let phase_iterations: Vec<String> = self
            .loop_controller
            .phase_iterations
            .iter()
            .map(|(p, c)| format!("- {:?}: {} iterations", p, c))
            .collect();

        let content = format!(
            "# Praxis Session State\n\
             \n\
             | Field | Value |\n\
             |-------|-------|\n\
             | Session | `{}` |\n\
             | Project | {} |\n\
             | Goal | {} |\n\
             | Phase | {:?} |\n\
             | Iteration | {} |\n\
             | Elapsed | {}s |\n\
             | Tokens used | {} |\n\
             | Est. cost | ${:.4} |\n\
             | Updated | {} |\n\
             \n\
             ## Done\n\
             \n\
             Phases completed so far (most recent first):\n\
             {}\n\
             \n\
             ## Next\n\
             \n\
             Valid transitions from current phase:\n\
             {}\n\
             \n\
             ## Phase Iterations\n\
             \n\
             {}\n",
            session,
            project,
            goal,
            phase,
            iteration,
            elapsed.as_secs(),
            tokens_used,
            cost,
            chrono::Utc::now().to_rfc3339(),
            if self.loop_controller.state_machine.history().is_empty() {
                "- (none yet)".to_string()
            } else {
                self.loop_controller
                    .state_machine
                    .history()
                    .iter()
                    .rev()
                    .map(|t| format!("- {:?} → {:?}", t.from, t.to))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            if valid_transitions.is_empty() {
                "- (terminal phase — loop ending)".to_string()
            } else {
                valid_transitions
                    .iter()
                    .map(|t| format!("- {}", t))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            if phase_iterations.is_empty() {
                "- (none yet)".to_string()
            } else {
                phase_iterations.join("\n")
            },
        );

        let path = std::path::PathBuf::from("STATE.md");
        if let Err(e) = std::fs::write(&path, &content) {
            tracing::warn!("Failed to write STATE.md: {}", e);
        } else {
            tracing::debug!("State file written: {}", path.display());
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
    /// Publish an event with the current session_id in metadata.
    /// This allows the frontend to filter events by session.
    fn publish_session_event(&self, kind: praxis_shared::protocol::MessageKind) {
        let sid = self.session_id.map(|s| s.to_string());
        self.bus.publish_with_session(kind, "core", sid.as_deref());
    }
    /// Process delegation requests from an agent's output.
    ///
    /// Agents can request delegation by including `DELEGATE:agent_type:task_description`
    /// in their output. This method parses those requests, invokes
    /// `delegate_to_subagent` for each, and appends the subagent results
    /// to the agent's output.
    ///
    /// Only agents whose `AgentDefinition.can_delegate()` is true can delegate.
    /// The parent's budget is derived from the session's LoopController limits.
    async fn process_delegation_requests(&mut self, agent_name: &str, output: &str) -> String {
        // Check if this agent can delegate at all
        let parent_def = match self.agent_registry.resolve(agent_name) {
            Some(a) if a.definition.can_delegate() => a.definition.clone(),
            _ => return output.to_string(), // not delegatable — return as-is
        };

        // Parse DELEGATE:agent_type:task_description lines from output
        let mut delegations = parse_delegate_requests(output);
        if delegations.is_empty() {
            return output.to_string();
        }

        // Enforce max_sub_agents limit (0 = no limit)
        let max = parent_def.max_sub_agents();
        if max > 0 && delegations.len() > max as usize {
            tracing::warn!(
                "Agent '{}' requested {} delegations but max_sub_agents={}. Truncating to {}.",
                agent_name,
                delegations.len(),
                max,
                max
            );
            delegations.truncate(max as usize);
        }

        // Build parent budget from session limits
        let parent_budget = self.session_budget();

        let mut result_output = output.to_string();

        for (child_type, task_desc) in &delegations {
            // Verify the parent can spawn this type
            if !parent_def.can_spawn_type(child_type) {
                tracing::warn!(
                    "Agent '{}' tried to delegate to '{}' but it's not in can_spawn {:?}",
                    agent_name,
                    child_type,
                    parent_def.can_spawn()
                );
                continue;
            }

            tracing::info!(
                "Agent '{}' delegating to '{}': {}",
                agent_name,
                child_type,
                task_desc.chars().take(80).collect::<String>()
            );

            let task = orchestrator::Task::new(child_type, parent_def.model(), task_desc);
            let request = crate::delegation::DelegateRequest {
                agent_type: child_type.clone(),
                task,
                parent_name: agent_name.to_string(),
            };

            // Resolve provider for the child agent
            let child_def = match self.agent_registry.resolve(child_type) {
                Some(a) => a.definition.clone(),
                None => {
                    tracing::warn!("Child agent '{}' not found in registry", child_type);
                    continue;
                }
            };
            let provider = self.resolve_provider_for_model(child_def.model());

            match crate::delegation::delegate_to_subagent(
                &request,
                &parent_budget,
                &self.agent_registry,
                provider,
                Some(&self.bus),
            )
            .await
            {
                Ok(delegate_result) => {
                    // Roll up child budget into session totals
                    let child_tokens = delegate_result.child_budget.used_tokens;
                    let child_cost = delegate_result.child_budget.used_cost;
                    if child_tokens > 0 {
                        self.loop_controller.record_token_usage(
                            child_tokens.try_into().unwrap_or(u32::MAX),
                            child_cost,
                        );
                    }

                    // Append the subagent's result to the parent's output
                    result_output.push_str(&format!(
                        "\n\n─── DELEGATION: {} → {} ───\n{}",
                        agent_name, child_type, delegate_result.result.content
                    ));
                }
                Err(e) => {
                    tracing::warn!(
                        "Delegation from '{}' to '{}' failed: {}",
                        agent_name,
                        child_type,
                        e
                    );
                    result_output.push_str(&format!(
                        "\n\n─── DELEGATION FAILED: {} → {} ───\nError: {}",
                        agent_name, child_type, e
                    ));
                }
            }
        }

        result_output
    }
    /// Build a Budget from the session's LoopController limits.
    fn session_budget(&self) -> praxis_shared::budget::Budget {
        praxis_shared::budget::Budget {
            max_tokens: self.loop_controller.limits.max_tokens,
            max_cost_usd: self.loop_controller.limits.max_cost_usd,
            max_turns: 100, // session-level turn cap
            max_depth: 3,   // max delegation depth from root
            used_tokens: self.loop_controller.tokens_used,
            used_cost: self.loop_controller.cost_usd,
            used_turns: 0,
        }
    }
    /// Resolve a provider for a given model name via the stored ProviderRouter.
    ///
    /// Returns `None` when no router is configured (tests, mock mode) or the
    /// model doesn't match any registered provider. The delegation system uses
    /// this to give child agents real LLM access instead of mock mode.
    fn resolve_provider_for_model(
        &self,
        model: &str,
    ) -> Option<std::sync::Arc<dyn praxis_agent_traits::provider::LLMProvider>> {
        let router = self.provider_router.as_ref()?;
        match router.resolve(model) {
            Ok(provider) => Some(provider),
            Err(e) => {
                tracing::warn!("No provider for model '{}': {}", model, e);
                None
            }
        }
    }
    /// dashboard can show them in real-time.
    ///
    /// Returns the output with tool results appended, plus info about
    /// which tools were called.
    async fn execute_tool_calls(&mut self, output: &str) -> ToolExecResult {
        let mut result = output.to_string();
        let mut tools_called: Vec<ToolCallInfo> = Vec::new();
        let mut search_start = 0;

        while let Some(pos) = output[search_start..].find("```tool") {
            let open_start = search_start + pos;

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
                        server,
                        tool_name,
                        args
                    );

                    let start = std::time::Instant::now();
                    let (tool_result, success) =
                        match self.mcp_host.call_tool(server, tool_name, args).await {
                            Ok(value) => (
                                serde_json::to_string_pretty(&value)
                                    .unwrap_or_else(|_| "{}".to_string()),
                                true,
                            ),
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

        ToolExecResult {
            output: result,
            tools_called,
        }
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
            tracing::info!(
                "Initializing provider: {} ({})",
                name,
                provider_cfg.base_url
            );

            // Resolve API key
            let api_key = self.resolve_api_key(&provider_cfg.api_key_ref, vault, name);

            if api_key.is_empty() {
                tracing::warn!(
                    "No API key for provider '{}'. Agent will use mock behavior.",
                    name
                );
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
                                tracing::warn!(
                                    "Failed to init OpenAI provider '{}': {}. Using mock.",
                                    name,
                                    e.0
                                );
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
                                tracing::warn!(
                                    "Failed to init Anthropic provider '{}': {}. Using mock.",
                                    name,
                                    e.0
                                );
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
                                tracing::warn!(
                                    "Failed to init Gemini provider '{}': {}. Using mock.",
                                    name,
                                    e.0
                                );
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
                                tracing::warn!(
                                    "Failed to init Ollama provider '{}': {}. Using mock.",
                                    name,
                                    e.0
                                );
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
                                tracing::warn!(
                                    "Failed to init provider '{}': {}. Using mock.",
                                    name,
                                    e.0
                                );
                                continue;
                            }
                        }
                    }
                };

            router.register(name, provider, praxis_providers::ModelTier::Balanced);
            tracing::info!(
                "Provider '{}' registered with model '{}'",
                name,
                provider_cfg.default_model
            );
        }

        router
    }
    /// Resolve an API key from vault, env, or config literal.
    ///
    /// Reference formats:
    /// - `keyring:NAME` / `vault:NAME` → look up NAME in VaultService
    /// - `env:VAR_NAME` → look up environment variable
    /// - `""` (empty) → try vault by provider name, then give up
    /// - anything else → treat as literal key (with warning if it looks like a real key)
    fn resolve_api_key(
        &self,
        ref_str: &str,
        vault: Option<&VaultService>,
        provider_name: &str,
    ) -> String {
        // 1. Explicit vault/keyring reference: "keyring:NAME" or "vault:NAME"
        let vault_name = ref_str
            .strip_prefix("keyring:")
            .or_else(|| ref_str.strip_prefix("vault:"));
        if let Some(name) = vault_name {
            if let Some(v) = vault
                && let Ok(Some(key)) = v.get(name)
                && !key.is_empty()
            {
                tracing::info!(
                    "Loaded API key for '{}' from vault (ref: {})",
                    name,
                    ref_str
                );
                return key;
            }
            tracing::warn!(
                "Vault ref '{}' but no key found in vault for '{}'",
                ref_str,
                name
            );
            return String::new();
        }

        // 2. Environment variable reference: "env:VAR_NAME"
        if let Some(var_name) = ref_str.strip_prefix("env:") {
            if let Ok(value) = std::env::var(var_name)
                && !value.is_empty()
            {
                tracing::info!(
                    "Loaded API key for '{}' from env:{}",
                    provider_name,
                    var_name
                );
                return value;
            }
            tracing::warn!(
                "Env var '{}' not set or empty for provider '{}'",
                var_name,
                provider_name
            );
            return String::new();
        }

        // 3. Empty ref: try vault by provider name as fallback
        if ref_str.is_empty() {
            if let Some(v) = vault
                && let Ok(Some(key)) = v.get(provider_name)
                && !key.is_empty()
            {
                tracing::info!(
                    "Loaded API key for '{}' from vault (by name)",
                    provider_name
                );
                return key;
            }
            return String::new();
        }

        // 4. Literal key in config
        if ref_str.starts_with("sk-") || ref_str.starts_with("xai-") {
            tracing::warn!(
                "⚠️  Using literal API key in config for '{}' — consider using Settings page",
                provider_name
            );
        }
        ref_str.to_string()
    }
    /// Inject skills content into a task's context. Called before each agent execution.
    fn inject_skills(&self, task: &mut orchestrator::Task) {
        if let Some(skills) = &self.skills_content {
            if task.context.is_empty() {
                task.context = format!("--- Skills ---\n{}", skills);
            } else {
                task.context = format!(
                    "--- Skills ---\n{}\n\n--- Task Context ---\n{}",
                    skills, task.context
                );
            }
        }
    }
    /// Common setup for `run_goal` and `resume_goal`.
    ///
    /// Loads the forge.toml config, initializes providers, wires the embedding
    /// service, connects MCP servers, applies limits, and registers gates.
    /// Returns the parsed config and the provider router.
    async fn prepare_goal_run(
        &mut self,
        config_path: Option<&std::path::Path>,
        vault: Option<&VaultService>,
    ) -> (ForgeConfig, std::sync::Arc<praxis_providers::ProviderRouter>) {
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

        let provider_router = std::sync::Arc::new(self.init_providers(&config, vault).await);

        // Store the router on the runtime so delegation can resolve real
        // providers for child agents (see `resolve_provider_for_model`).
        self.provider_router = Some(std::sync::Arc::clone(&provider_router));
        // Wire embedding service to MemoryKeeper using the first available provider
        if let Some(provider) = provider_router.first_provider() {
            self.with_embedding_provider(provider).await;
        }

        // Connect MCP servers defined in forge.toml
        self.connect_mcp_servers(&config).await;

        // Apply limits from forge.toml [limits] section
        if let Some(limits) = &config.limits {
            self.loop_controller.limits = limits.clone();
            tracing::info!(
                "Applied forge.toml limits: max_iterations_per_goal={}, max_iterations_per_phase={}, session_ttl={}s, phase_timeout={}s",
                limits.max_iterations_per_goal,
                limits.max_iterations_per_phase,
                limits.session_ttl_seconds,
                limits.phase_timeout_seconds,
            );
        }

        // Initialize context budget if not already set.
        // Default: 128k window, 70% hard limit = 89,600 tokens — covers GPT-4o,
        // Claude 3.5, and Gemini 1.5 context windows.
        if self.context_budget.is_none() {
            self.context_budget = Some(praxis_memory::context::ContextBudget::new(
                128_000,
                praxis_memory::context::BudgetProfile::Balanced,
            ));
        }

        // Initialize hot memory (per-agent sliding windows) and context manager
        // (compression pipeline + EMC) if not already set.
        if self.hot_memory.is_none() {
            self.hot_memory = Some(praxis_memory::hot::HotMemory::new());
        }
        if self.context_manager.is_none() {
            self.context_manager = Some(praxis_memory::context::ContextManager::new(
                128_000,
                praxis_memory::context::BudgetProfile::Balanced,
            ));
        }

        // Register quality gates for review/test/security phases
        self.register_default_gates();

        (config, provider_router)
    }
    #[tracing::instrument(skip(self, vault))]
    pub async fn run_goal(
        &mut self,
        goal: &str,
        config_path: Option<&std::path::Path>,
        vault: Option<&VaultService>,
    ) -> Result<GoalResult> {
        tracing::info!("Starting goal: {}", goal);

        let (config, provider_router) = self.prepare_goal_run(config_path, vault).await;

        if config.roles.is_empty() {
            tracing::info!("No roles defined in config. Using default coder role.");
        }

        // Set up outcome-based completion criterion (default: coding verifier)
        if self.completion_criterion.is_none() {
            self.completion_criterion = Some(default_coding_criterion());
        }
        self.pathology_detector.reset();
        self.model_override = None;

        // Assign a session ID
        self.session_id = Some(uuid::Uuid::new_v4());
        self.propagate_session_to_memory(self.session_id.expect("session_id set above"));
        // Capture a git rollback baseline (HEAD commit + uncommitted diff).
        // Best-effort: skipped silently if no event store or not in a git repo.
        if let Some(store) = &self.event_store
            && let Some(cwd) = std::env::current_dir().ok()
            && let Err(e) = crate::rollback::capture_baseline(
                store,
                self.session_id.expect("session_id set above"),
                &cwd,
            )
        {
            tracing::warn!("Failed to capture rollback baseline: {}", e);
        }

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

        // Resolve the workflow for this goal (if any). When a workflow is
        // active, it drives phase transitions and agent selection instead of
        // the hardcoded get_next_phase / get_agents_for_phase functions.
        let goal_workflow = config.goals.first().and_then(|g| g.workflow.as_deref());
        let mut workflow_engine = workflow::GoalEngine::new()
            .resolve(goal_workflow, &config.workflows)
            .map(workflow::WorkflowEngine::new);
        if workflow_engine.is_some() {
            tracing::info!("Using workflow: {}", goal_workflow.unwrap_or("?"));
        }

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

            // Select agents for this phase. When a workflow is active, use the
            // workflow's agent list; otherwise fall back to the default mapping.
            let parallel_count = config.goals.first().and_then(|g| g.parallel_reviewers);
            let phase_agents = if let Some(engine) = &workflow_engine {
                get_workflow_agents(engine, &config, parallel_count, &current_phase)
            } else {
                get_agents_for_phase(&current_phase, &config, parallel_count)
            };
            let results_before = results.len();

            if phase_agents.len() > 1 && matches!(current_phase, machine::phase::Phase::Reviewing) {
                // ── Parallel execution for review phases ──
                let mut join_set = tokio::task::JoinSet::new();

                for role_config in &phase_agents {
                    let mut task =
                        orchestrator::Task::new(&role_config.name, &role_config.model, goal);

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

                    // Inject skills content (SKILL.md) into the task context
                    self.inject_skills(&mut task);

                    // Inject compressed interaction history + clamp to budget
                    self.prepare_context_with_history(&mut task, &role_config.name);
                    self.clamp_context_to_budget(&mut task);

                    let resolved_role = self
                        .agent_registry
                        .resolve_role(&role_config.name)
                        .unwrap_or_else(|| {
                            orchestrator::roles::ResolvedRole::resolve(role_config, None)
                        });
                    let effective_model = self.effective_model(&role_config.model);
                    let (agent, provider_name) = match provider_router.resolve(effective_model) {
                        Ok(provider) => {
                            let name = provider.provider_name().to_string();
                            (
                                crate::actor::roles::AgentFactory::create_with_provider_and_bus(
                                    &resolved_role,
                                    provider,
                                    self.bus.clone(),
                                ),
                                name,
                            )
                        }
                        Err(_) => {
                            tracing::warn!(
                                "No provider for model '{}'. Using mock agent for '{}'.",
                                effective_model,
                                role_config.name
                            );
                            (
                                crate::actor::roles::AgentFactory::create(&resolved_role),
                                "mock".to_string(),
                            )
                        }
                    };

                    let agent_name = role_config.name.clone();
                    self.publish_session_event(
                        praxis_shared::protocol::MessageKind::AgentStarted {
                            agent: agent_name.clone(),
                            role: agent_name.clone(),
                            phase: current_phase,
                        },
                    );

                    // Spawn agent in parallel (return provider_name alongside result)
                    join_set.spawn(async move {
                        let result = agent.execute(&task).await;
                        (result, provider_name)
                    });
                }

                // Collect parallel results
                while let Some(join_result) = join_set.join_next().await {
                    match join_result {
                        Ok((raw_result, provider_name)) => {
                            let tool_exec = self.execute_tool_calls(&raw_result.content).await;
                            let result = if tool_exec.output != raw_result.content {
                                TaskResult {
                                    content: tool_exec.output,
                                    ..raw_result
                                }
                            } else {
                                raw_result
                            };

                            // Process delegation requests (DELEGATE: lines in output)
                            let result = TaskResult {
                                content: self
                                    .process_delegation_requests(&result.agent_id, &result.content)
                                    .await,
                                ..result
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

                            self.publish_session_event(
                                praxis_shared::protocol::MessageKind::AgentCompleted {
                                    agent: result.agent_id.clone(),
                                    role: result.role.clone(),
                                    status: format!("{:?}", result.status),
                                    duration_ms: result.duration_ms,
                                    output_preview: result.content.chars().take(200).collect(),
                                },
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
                                        provider: provider_name.clone(),
                                        model: result.agent_id.clone(),
                                        input: result.token_usage.input,
                                        output: result.token_usage.output,
                                    },
                                    "core",
                                );
                                // Accumulate session budget
                                let cost = estimate_token_cost(
                                    &provider_name,
                                    &result.agent_id,
                                    result.token_usage.input,
                                    result.token_usage.output,
                                );
                                self.loop_controller
                                    .record_token_usage(result.token_usage.total, cost);
                            }

                            results.push(result);
                            let result = results.last().expect("results populated by push above");

                            // Push interaction to sliding window for history tracking
                            self.push_agent_interaction(
                                &result.agent_id,
                                goal,
                                &result.content,
                                result.token_usage.total,
                            );

                            // ── Drift metrics recording (parallel) ──────────
                            let pressure = self.compute_context_pressure();
                            self.set_context_pressure(pressure);

                            let drift_sample = crate::drift::metrics::MetricSample {
                                iteration: self.loop_controller.iteration,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                latency_ms: result.duration_ms,
                                output_tokens: result.token_usage.output,
                                input_tokens: result.token_usage.input,
                                tool_calls: tool_exec.tools_called.len() as u32,
                                tool_errors: tool_exec
                                    .tools_called
                                    .iter()
                                    .filter(|t| !t.success)
                                    .count() as u32,
                                output_length_chars: result.content.len(),
                                gate_passed: self.last_gate_passed,
                                context_pressure: pressure,
                            };
                            if let Some(report) = self
                                .drift_guard
                                .record_and_evaluate(drift_sample, Some(&result.agent_id))
                                && let Some(action) = &report.recovery_action
                            {
                                self.handle_recovery_action(action, Some(&result.agent_id))
                                    .await;
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
                    verdict.passed,
                    verdict.confidence,
                    review_results.len()
                );
            } else {
                // ── Sequential execution (single agent or non-review phases) ──
                for role_config in &phase_agents {
                    let mut task =
                        orchestrator::Task::new(&role_config.name, &role_config.model, goal);

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

                    // Inject skills content (SKILL.md) into the task context
                    self.inject_skills(&mut task);

                    // ── MemoryRAG injection ─────────────────────────────────────
                    // Search episodic memory for relevant chunks and inject them
                    // into the agent's context (uses embedding-based search when
                    // an EmbeddingService is attached, keyword fallback otherwise).
                    if let Some(ref keeper) = self.memory_keeper {
                        let rag_k = self.calculate_rag_k();
                        let results = keeper.search_rag(goal, rag_k, None).await;
                        if !results.is_empty() {
                            let mut rag_parts: Vec<String> = Vec::new();
                            rag_parts.push("─── RELEVANT MEMORY ───".to_string());
                            for result in &results {
                                rag_parts.push(format!(
                                    "• [score={:.2}] {}",
                                    result.score, result.chunk.content
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

                    // Inject compressed interaction history + clamp to budget
                    self.prepare_context_with_history(&mut task, &role_config.name);
                    self.clamp_context_to_budget(&mut task);

                    let resolved_role = self
                        .agent_registry
                        .resolve_role(&role_config.name)
                        .unwrap_or_else(|| {
                            orchestrator::roles::ResolvedRole::resolve(role_config, None)
                        });
                    let effective_model = self.effective_model(&role_config.model);
                    let (agent, provider_name) = match provider_router.resolve(effective_model) {
                        Ok(provider) => {
                            let name = provider.provider_name().to_string();
                            (
                                crate::actor::roles::AgentFactory::create_with_provider_and_bus(
                                    &resolved_role,
                                    provider,
                                    self.bus.clone(),
                                ),
                                name,
                            )
                        }
                        Err(_) => {
                            tracing::warn!(
                                "No provider for model '{}'. Using mock agent for '{}'.",
                                effective_model,
                                role_config.name
                            );
                            (
                                crate::actor::roles::AgentFactory::create(&resolved_role),
                                "mock".to_string(),
                            )
                        }
                    };

                    // Publish agent start event for live streaming
                    self.publish_session_event(
                        praxis_shared::protocol::MessageKind::AgentStarted {
                            agent: role_config.name.clone(),
                            role: role_config.name.clone(),
                            phase: current_phase,
                        },
                    );

                    let raw_result = if has_feedback {
                        agent.handle_feedback(&task, &feedback).await
                    } else {
                        agent.execute(&task).await
                    };

                    // Execute tool calls from the initial agent response
                    let mut tool_exec = self.execute_tool_calls(&raw_result.content).await;
                    let mut result = if tool_exec.output != raw_result.content {
                        TaskResult {
                            content: tool_exec.output,
                            ..raw_result
                        }
                    } else {
                        raw_result
                    };

                    // Publish ToolCalled events for the initial round
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

                    // Accumulate tool call metrics across all rounds
                    let mut total_tool_calls = tool_exec.tools_called.len() as u32;
                    let mut total_tool_errors =
                        tool_exec.tools_called.iter().filter(|t| !t.success).count() as u32;

                    // Tool call loop: re-invoke agent with tool results, execute
                    // new tool calls from the follow-up response, repeat until no
                    // tools are called or max rounds reached.
                    const MAX_TOOL_ROUNDS: usize = 5;
                    for round in 1..=MAX_TOOL_ROUNDS {
                        if tool_exec.tools_called.is_empty() {
                            break;
                        }

                        tracing::info!(
                            "Agent {} called {} tool(s) in round {}, re-invoking with results",
                            result.agent_id,
                            tool_exec.tools_called.len(),
                            round
                        );

                        let follow_up_agent = match provider_router
                            .resolve(self.effective_model(&role_config.model))
                        {
                            Ok(provider) => {
                                crate::actor::roles::AgentFactory::create_with_provider_and_bus(
                                    &resolved_role,
                                    provider,
                                    self.bus.clone(),
                                )
                            }
                            Err(_) => crate::actor::roles::AgentFactory::create(&resolved_role),
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
                            budget: None,
                        };
                        let follow_up_result = follow_up_agent.execute(&follow_up_task).await;

                        // Execute tool calls from the follow-up response only
                        // (not the full accumulated content — avoids re-executing
                        // tools from previous rounds)
                        tool_exec = self.execute_tool_calls(&follow_up_result.content).await;

                        result = TaskResult {
                            content: format!(
                                "{}\n\n─── FOLLOW-UP AFTER TOOL RESULTS (round {}) ───\n{}",
                                result.content, round, tool_exec.output
                            ),
                            ..result
                        };

                        // Publish ToolCalled events for this round
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

                        total_tool_calls += tool_exec.tools_called.len() as u32;
                        total_tool_errors +=
                            tool_exec.tools_called.iter().filter(|t| !t.success).count() as u32;

                        if round == MAX_TOOL_ROUNDS && !tool_exec.tools_called.is_empty() {
                            tracing::warn!(
                                "Agent {} hit max tool rounds ({}) — stopping tool call loop",
                                result.agent_id,
                                MAX_TOOL_ROUNDS
                            );
                        }
                    }

                    // Process delegation requests from the agent's output
                    // (agents can request subagent delegation via DELEGATE: lines)
                    result = TaskResult {
                        content: self
                            .process_delegation_requests(&role_config.name, &result.content)
                            .await,
                        ..result
                    };

                    // Publish agent completion event
                    self.publish_session_event(
                        praxis_shared::protocol::MessageKind::AgentCompleted {
                            agent: result.agent_id.clone(),
                            role: result.role.clone(),
                            status: format!("{:?}", result.status),
                            duration_ms: result.duration_ms,
                            output_preview: result.content.chars().take(200).collect(),
                        },
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
                                provider: provider_name.clone(),
                                model: result.agent_id.clone(),
                                input: result.token_usage.input,
                                output: result.token_usage.output,
                            },
                            "core",
                        );
                        // Accumulate session budget
                        let cost = estimate_token_cost(
                            &provider_name,
                            &result.agent_id,
                            result.token_usage.input,
                            result.token_usage.output,
                        );
                        self.loop_controller
                            .record_token_usage(result.token_usage.total, cost);
                    }

                    results.push(result);
                    let result = results.last().expect("results populated by push above");

                    // Push interaction to sliding window for history tracking
                    self.push_agent_interaction(
                        &result.agent_id,
                        goal,
                        &result.content,
                        result.token_usage.total,
                    );

                    // ── Drift metrics recording ─────────────────────────
                    // Estimate context pressure from token usage vs budget
                    let pressure = self.compute_context_pressure();
                    self.set_context_pressure(pressure);

                    let drift_sample = crate::drift::metrics::MetricSample {
                        iteration: self.loop_controller.iteration,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        latency_ms: result.duration_ms,
                        output_tokens: result.token_usage.output,
                        input_tokens: result.token_usage.input,
                        tool_calls: total_tool_calls,
                        tool_errors: total_tool_errors,
                        output_length_chars: result.content.len(),
                        gate_passed: self.last_gate_passed,
                        context_pressure: pressure,
                    };
                    if let Some(report) = self
                        .drift_guard
                        .record_and_evaluate(drift_sample, Some(&result.agent_id))
                        && let Some(action) = &report.recovery_action
                    {
                        self.handle_recovery_action(action, Some(&result.agent_id))
                            .await;
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
                self.last_gate_passed = gates_pass;

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

                    // When a workflow is active, let it decide the next phase
                    // (conditional branching on GateFailed). Otherwise default
                    // to Fixing.
                    let fail_phase = if let Some(engine) = &mut workflow_engine {
                        engine.next_phase(workflow::GateOutcome::Failed)
                    } else {
                        machine::phase::Phase::Fixing
                    };
                    tracing::info!(
                        "Gate failed on {:?}. Going to {:?}. Feedback: {} chars",
                        current_phase,
                        fail_phase,
                        feedback.len()
                    );
                    current_phase = fail_phase;
                    self.loop_controller
                        .advance(fail_phase)
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
            let pressure = self
                .context_pressure
                .load(std::sync::atomic::Ordering::Relaxed) as f32
                / 1000.0;
            if pressure > 0.85 {
                tracing::warn!(
                    "EMC triggered: context pressure {:.1}% > 85%. Forcing consolidation.",
                    pressure * 100.0
                );
                self.summarize_current_session().await;
                self.set_context_pressure(0.5);
                // Force a context reset via drift guard
                self.drift_guard.recovery.execute_context_reset(
                    &self
                        .session_id
                        .map_or("unknown".to_string(), |s| s.to_string()),
                    "EMC: emergency consolidation",
                    goal,
                );
            }
            // ── End drift evaluation + EMC ──────────────────────────

            // ── Pathology detection ──────────────────────────────
            // Check every agent that ran in THIS iteration (not just results.last()).
            // In parallel review phases, a stuck agent may not be the last result
            // added — JoinSet completion order is non-deterministic.
            let phase_str = format!("{:?}", current_phase);
            let mut fatal_pathology = false;
            for result in &results[results_before..] {
                let token_count = result.token_usage.output;
                if let Some(alert) = self.pathology_detector.record_iteration(
                    self.loop_controller.iteration,
                    &result.content,
                    &phase_str,
                    Some(token_count),
                ) {
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

                    // Cross-model verification: ask another LLM for a second opinion
                    if alert.severity >= r#loop::PathologySeverity::Warning {
                        if let Some(second_provider) = provider_router.first_provider() {
                            let verification = self
                                .pathology_detector
                                .verify_with_model(
                                    second_provider.as_ref(),
                                    goal,
                                    &result.content,
                                    &alert,
                                )
                                .await;

                            let is_no = verification.to_lowercase().starts_with("no");
                            if is_no {
                                // Escalate severity
                                tracing::warn!(
                                    "Cross-model verification says NO — escalating {:?} to Critical",
                                    alert.kind
                                );
                                // Re-publish with escalated severity
                                let escalated_kind = format!("{:?}", alert.kind);
                                self.bus.publish(
                                    praxis_shared::protocol::MessageKind::PathologyDetected(
                                        praxis_shared::protocol::PathologyAlert {
                                            kind: escalated_kind.clone(),
                                            severity: "Critical".to_string(),
                                            details: format!(
                                                "{} — Cross-model verification: {}",
                                                alert.details, verification
                                            ),
                                            action: format!("{:?}", alert.recommended_action),
                                            iteration: alert.iteration,
                                        },
                                    ),
                                    "core",
                                );
                            } else {
                                tracing::info!("Cross-model verification: {}", verification);
                            }
                        } else {
                            tracing::debug!("No provider available for cross-model verification");
                        }
                    }

                    // Fatal pathology → kill the loop immediately
                    if alert.severity == r#loop::PathologySeverity::Fatal {
                        tracing::error!(
                            "Fatal pathology: {}. Stopping loop immediately.",
                            alert.details
                        );
                        fatal_pathology = true;
                        break;
                    }
                }
            }
            if fatal_pathology {
                break;
            }

            // ── Completion criterion (outcome-based) ─────────────
            // Verify if the goal is achieved after each phase, not just review
            // phases. This lets simple goals (e.g. with a CommandOutcomeVerifier
            // like `until:cargo test`) complete early in Planning/Implementing
            // without wasting iterations on Reviewing/Testing/SecurityScan.
            if matches!(
                current_phase,
                machine::phase::Phase::Planning
                    | machine::phase::Phase::Implementing
                    | machine::phase::Phase::Reviewing
                    | machine::phase::Phase::Testing
                    | machine::phase::Phase::SecurityScan
                    | machine::phase::Phase::Finalizing
            ) && let Some(criterion) = &mut self.completion_criterion
            {
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
                        tracing::warn!("Goal exhausted: {}. Stopping loop.", reason);
                        break;
                    }
                    completion::OutcomeResult::NotAchieved { reason } => {
                        tracing::info!("Goal not yet achieved: {}. Continuing.", reason);
                    }
                }
            }

            let next_phase = if let Some(engine) = &mut workflow_engine {
                engine.next_phase(workflow::GateOutcome::Passed)
            } else {
                get_next_phase(&current_phase)
            };
            match self.loop_controller.advance(next_phase) {
                Ok(transition) => {
                    self.bus.publish(
                        praxis_shared::protocol::MessageKind::PhaseChanged(
                            praxis_shared::protocol::PhaseTransition {
                                from: transition.from,
                                to: transition.to,
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

            // Capture undo/redo change snapshot (best-effort).
            if let Some(store) = &self.event_store
                && let Some(sid) = self.session_id
                && let Some(cwd) = std::env::current_dir().ok()
            {
                let desc = format!(
                    "Phase: {:?}, Iteration {}",
                    current_phase, self.loop_controller.iteration
                );
                if let Err(e) = crate::undo::capture_change(store, sid, &cwd, &desc) {
                    tracing::warn!("Failed to capture undo change: {}", e);
                }
            }
        }

        self.loop_controller.stop();

        // Save final checkpoint
        self.save_checkpoint(goal).await;

        // Summarize entire session into consolidated memory
        self.summarize_current_session().await;

        // Evict episodic chunks older than 30 days (TTL cleanup)
        self.cleanup_episodic_memory().await;

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

    #[tracing::instrument(skip(self, vault))]
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
            .and_then(|v| serde_json::from_value::<machine::phase::Phase>(v.clone()).ok())
            .unwrap_or(machine::phase::Phase::Planning);

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
        // Restore the state machine to the saved phase (bypasses transition validation)
        self.loop_controller
            .state_machine
            .restore_phase(saved_phase);

        // Common setup: load config, init providers, connect MCP, apply limits, register gates
        let (config, provider_router) = self.prepare_goal_run(config_path, vault).await;

        // Re-register completion criterion (gates already registered by prepare_goal_run)
        if self.completion_criterion.is_none() {
            self.completion_criterion = Some(default_coding_criterion());
        }
        self.pathology_detector.reset();
        self.model_override = None;

        self.loop_controller.start();

        let mut results = Vec::new();
        let mut feedback = String::new();
        let mut current_phase = saved_phase;

        // Resolve the workflow for this goal (same as run_goal).
        let goal_workflow = config.goals.first().and_then(|g| g.workflow.as_deref());
        let mut workflow_engine = workflow::GoalEngine::new()
            .resolve(goal_workflow, &config.workflows)
            .map(workflow::WorkflowEngine::new);

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
            let phase_agents = if let Some(engine) = &workflow_engine {
                get_workflow_agents(engine, &config, parallel_count, &current_phase)
            } else {
                get_agents_for_phase(&current_phase, &config, parallel_count)
            };
            let results_before = results.len();

            for role_config in &phase_agents {
                let mut task =
                    orchestrator::Task::new(&role_config.name, &role_config.model, &goal);

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
                        "Injected message into task for agent '{}'",
                        role_config.name
                    );
                }

                // Inject skills content (SKILL.md) into the task context
                self.inject_skills(&mut task);

                // Inject compressed interaction history + clamp to budget
                self.prepare_context_with_history(&mut task, &role_config.name);
                self.clamp_context_to_budget(&mut task);

                let resolved_role = self
                    .agent_registry
                    .resolve_role(&role_config.name)
                    .unwrap_or_else(|| {
                        orchestrator::roles::ResolvedRole::resolve(role_config, None)
                    });
                let agent = match provider_router.resolve(self.effective_model(&role_config.model))
                {
                    Ok(provider) => {
                        crate::actor::roles::AgentFactory::create_with_provider_and_bus(
                            &resolved_role,
                            provider,
                            self.bus.clone(),
                        )
                    }
                    Err(_) => crate::actor::roles::AgentFactory::create(&resolved_role),
                };

                let result = if has_feedback {
                    agent.handle_feedback(&task, &feedback).await
                } else {
                    agent.execute(&task).await
                };

                results.push(result);
                let result = results.last().expect("results populated by push above");

                // Push interaction to sliding window for history tracking
                self.push_agent_interaction(
                    &result.agent_id,
                    &goal,
                    &result.content,
                    result.token_usage.total,
                );

                // Accumulate session budget (resume path)
                if result.token_usage.total > 0 {
                    let cost = estimate_token_cost(
                        &role_config.model,
                        &result.agent_id,
                        result.token_usage.input,
                        result.token_usage.output,
                    );
                    self.loop_controller
                        .record_token_usage(result.token_usage.total, cost);
                }

                // ── Drift metrics recording (resume_goal) ────────────
                let pressure = self.compute_context_pressure();
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
                    gate_passed: self.last_gate_passed,
                    context_pressure: pressure,
                };
                if let Some(report) = self
                    .drift_guard
                    .record_and_evaluate(drift_sample, Some(&result.agent_id))
                    && let Some(action) = &report.recovery_action
                {
                    self.handle_recovery_action(action, Some(&result.agent_id))
                        .await;
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
                self.last_gate_passed = gates_pass;

                if !gates_pass {
                    feedback = consolidate_feedback(&results);
                    let fail_phase = if let Some(engine) = &mut workflow_engine {
                        engine.next_phase(workflow::GateOutcome::Failed)
                    } else {
                        machine::phase::Phase::Fixing
                    };
                    current_phase = fail_phase;
                    self.loop_controller
                        .advance(fail_phase)
                        .map_err(CoreError::StateMachine)?;
                    self.loop_controller.increment_iteration();
                    continue;
                } else {
                    feedback.clear();
                }
            }

            // ── Drift evaluation + EMC (resume_goal) ──────────────
            self.evaluate_drift(None).await;

            let pressure = self
                .context_pressure
                .load(std::sync::atomic::Ordering::Relaxed) as f32
                / 1000.0;
            if pressure > 0.85 {
                tracing::warn!(
                    "EMC triggered (resume): context pressure {:.1}% > 85%. Forcing consolidation.",
                    pressure * 100.0
                );
                self.summarize_current_session().await;
                self.set_context_pressure(0.5);
            }
            // ── End drift evaluation + EMC ──────────────────────────

            // ── Pathology detection (resume_goal) ──────────────
            // Check every agent that ran in THIS iteration, not just results.last().
            let phase_str = format!("{:?}", current_phase);
            let mut fatal_pathology = false;
            for result in &results[results_before..] {
                let token_count = result.token_usage.output;
                if let Some(alert) = self.pathology_detector.record_iteration(
                    self.loop_controller.iteration,
                    &result.content,
                    &phase_str,
                    Some(token_count),
                ) {
                    tracing::error!(
                        "Loop pathology detected: {:?} — {}",
                        alert.kind,
                        alert.details
                    );

                    // Cross-model verification
                    if alert.severity >= r#loop::PathologySeverity::Warning {
                        if let Some(second_provider) = provider_router.first_provider() {
                            let verification = self
                                .pathology_detector
                                .verify_with_model(
                                    second_provider.as_ref(),
                                    &goal,
                                    &result.content,
                                    &alert,
                                )
                                .await;

                            if verification.to_lowercase().starts_with("no") {
                                tracing::warn!(
                                    "Cross-model verification says NO — escalating {:?} to Critical",
                                    alert.kind
                                );
                            } else {
                                tracing::info!("Cross-model verification: {}", verification);
                            }
                        } else {
                            tracing::debug!("No provider available for cross-model verification");
                        }
                    }

                    if alert.severity == r#loop::PathologySeverity::Fatal {
                        fatal_pathology = true;
                        break;
                    }
                }
            }
            if fatal_pathology {
                break;
            }

            if matches!(
                current_phase,
                machine::phase::Phase::Planning
                    | machine::phase::Phase::Implementing
                    | machine::phase::Phase::Reviewing
                    | machine::phase::Phase::Testing
                    | machine::phase::Phase::SecurityScan
                    | machine::phase::Phase::Finalizing
            ) && let Some(criterion) = &mut self.completion_criterion
            {
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

            let next_phase = if let Some(engine) = &mut workflow_engine {
                engine.next_phase(workflow::GateOutcome::Passed)
            } else {
                get_next_phase(&current_phase)
            };
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

            // Capture undo/redo change snapshot (best-effort).
            if let Some(store) = &self.event_store
                && let Some(sid) = self.session_id
                && let Some(cwd) = std::env::current_dir().ok()
            {
                let desc = format!(
                    "Phase: {:?}, Iteration {}",
                    current_phase, self.loop_controller.iteration
                );
                if let Err(e) = crate::undo::capture_change(store, sid, &cwd, &desc) {
                    tracing::warn!("Failed to capture undo change: {}", e);
                }
            }
        }

        self.loop_controller.stop();
        self.save_checkpoint(&goal).await;

        self.summarize_current_session().await;

        // Evict episodic chunks older than 30 days (TTL cleanup)
        self.cleanup_episodic_memory().await;

        let total_duration: u64 = results.iter().map(|r| r.duration_ms).sum();
        let passed = current_phase == machine::phase::Phase::Completed;

        Ok(Some(GoalResult {
            goal,
            passed,
            agent_results: results,
            total_duration_ms: total_duration,
        }))
    }

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
}

/// Estimate the USD cost of a token usage based on provider and model.
///
/// Uses rough 2025-era pricing per 1M tokens. Returns 0.0 for unknown
/// providers/models — cost tracking only works for known pricing.
fn estimate_token_cost(provider: &str, model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
    const PRICING: &[(&str, &str, f64, f64)] = &[
        // (provider_prefix, model_prefix, input_per_1m, output_per_1m)
        ("openai", "gpt-4o", 2.50, 10.00),
        ("openai", "gpt-4", 2.50, 10.00),
        ("openai", "o1", 15.00, 60.00),
        ("openai", "o3", 10.00, 40.00),
        ("openai", "gpt-4o-mini", 0.15, 0.60),
        ("anthropic", "claude-4-opus", 15.00, 75.00),
        ("anthropic", "claude-3-opus", 15.00, 75.00),
        ("anthropic", "claude-4-sonnet", 3.00, 15.00),
        ("anthropic", "claude-3-5-sonnet", 3.00, 15.00),
        ("anthropic", "claude-3-sonnet", 3.00, 15.00),
        ("anthropic", "claude-3-haiku", 0.25, 1.25),
        ("gemini", "gemini-2", 1.25, 5.00),
        ("gemini", "gemini-1.5", 1.25, 5.00),
        ("ollama", "", 0.0, 0.0), // local — free
    ];

    let provider_lower = provider.to_lowercase();
    let model_lower = model.to_lowercase();

    for (prov_prefix, model_prefix, in_per_1m, out_per_1m) in PRICING {
        if provider_lower.starts_with(prov_prefix) && model_lower.starts_with(model_prefix) {
            return (input_tokens as f64 / 1_000_000.0 * in_per_1m)
                + (output_tokens as f64 / 1_000_000.0 * out_per_1m);
        }
    }

    0.0
}

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
        machine::phase::Phase::Implementing => lookup("coder").into_iter().collect(),
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
                    role.system_prompt = role
                        .system_prompt
                        .or_else(|| {
                            Some(default_role("reviewer").system_prompt.unwrap_or_default())
                        })
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
        machine::phase::Phase::Testing => lookup("tester").into_iter().collect(),
        machine::phase::Phase::SecurityScan => lookup("security").into_iter().collect(),
        machine::phase::Phase::Finalizing => Vec::new(),
        _ => Vec::new(),
    }
}

/// Resolve agents for a phase using a workflow engine.
///
/// Looks up each agent name from the workflow's phase definition in the
/// config's roles. Falls back to `default_role` when no roles are configured
/// (mock mode). Applies `parallel_reviewers` expansion for Reviewing phases.
fn get_workflow_agents(
    engine: &workflow::WorkflowEngine,
    config: &ForgeConfig,
    parallel_reviewers: Option<u32>,
    current_phase: &machine::phase::Phase,
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

    let agent_names = engine.agents_for_phase();

    // Apply parallel reviewer expansion for Reviewing phases.
    if matches!(current_phase, machine::phase::Phase::Reviewing)
        && let Some(count) = parallel_reviewers.filter(|c| *c > 0)
        && agent_names.len() <= 1
    {
        let base_name = agent_names
            .first()
            .map(|s| s.as_str())
            .unwrap_or("reviewer");
        let mut agents: Vec<orchestrator::RoleConfig> = Vec::new();
        for index in 0..count {
            let mut role = lookup(base_name).unwrap_or_else(|| default_role(base_name));
            role.name = format!("{}-{}", base_name, index + 1);
            let angles = [
                "Focus on correctness, edge cases, and logic errors.",
                "Focus on code style, readability, and best practices.",
                "Focus on performance, resource usage, and optimization opportunities.",
                "Focus on security vulnerabilities and unsafe code patterns.",
                "Focus on test coverage and maintainability.",
            ];
            let angle = angles[index as usize % angles.len()];
            role.system_prompt = role
                .system_prompt
                .or_else(|| Some(default_role(base_name).system_prompt.unwrap_or_default()))
                .map(|p| format!("{}\n\nYour specific focus: {}", p, angle));
            agents.push(role);
        }
        return agents;
    }

    agent_names.iter().filter_map(|name| lookup(name)).collect()
}

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

/// Parse `DELEGATE:agent_type:task_description` lines from agent output.
///
/// Format: each delegation request on its own line, prefixed with `DELEGATE:`.
/// Example: `DELEGATE:researcher:investigate async patterns in Rust 2024`
///
/// Returns a list of (agent_type, task_description) pairs.
pub(crate) fn parse_delegate_requests(output: &str) -> Vec<(String, String)> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed.strip_prefix("DELEGATE:")?;
            let colon_pos = rest.find(':')?;
            let agent_type = rest[..colon_pos].trim().to_string();
            let task_desc = rest[colon_pos + 1..].trim().to_string();
            if agent_type.is_empty() || task_desc.is_empty() {
                return None;
            }
            Some((agent_type, task_desc))
        })
        .collect()
}

/// Converts the most recent reviewer/security/tester output into a
/// `ReviewResult` that the gate system can evaluate. Parses the agent's
/// text output for PASS/FAIL keywords.
pub(crate) fn extract_review_results(
    results: &[orchestrator::TaskResult],
) -> Vec<machine::gate::ReviewResult> {
    results
        .iter()
        .rev()
        .find(|r| matches!(r.role.as_str(), "reviewer" | "security" | "tester"))
        .map(|r| {
            let content_lower = r.content.to_lowercase();
            // Explicit PASS/FAIL markers take priority
            let has_explicit_fail = content_lower.contains("review: fail")
                || content_lower.contains("scan: fail")
                || content_lower.contains("test: fail")
                || content_lower.contains("status: fail")
                || content_lower.contains("result: fail");
            let has_explicit_pass = content_lower.contains("review: pass")
                || content_lower.contains("scan: pass")
                || content_lower.contains("test: pass")
                || content_lower.contains("status: pass")
                || content_lower.contains("result: pass")
                || content_lower.contains("0 critical")
                || content_lower.contains("no critical")
                || content_lower.contains("no issues");

            let passed = if has_explicit_fail {
                false
            } else if has_explicit_pass {
                true
            } else {
                // No explicit marker: default to pass (don't block on ambiguous output)
                true
            };

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

pub(crate) fn consolidate_feedback(results: &[orchestrator::TaskResult]) -> String {
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
