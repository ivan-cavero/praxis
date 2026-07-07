//! # praxis Core Runtime
//!
//! The heart of the system: actor model, state machine, orchestrator,
//! loop controller, drift detection, and context management.
//!
//! The runtime is split across three modules:
//! - [`config`]: forge.toml parsing and runtime config types
//! - [`runtime`]: `CoreRuntime` struct, constructors, builders, accessors
//! - [`pipeline`]: `run_goal`/`resume_goal` execution loop and helpers

pub mod actor;
pub mod agents;
pub mod api;
pub mod bus;
pub mod completion;
pub mod config;
pub mod delegation;
pub mod drift;
pub mod r#loop;
pub mod machine;
pub mod orchestrator;
pub mod pipeline;
pub mod runtime;
pub mod skills;
pub mod workflow;

#[cfg(test)]
mod integration_tests;

// Re-exports for convenience
pub use actor::*;
pub use bus::EventBus;
pub use completion::{
    CompletionCriterion, OutcomeResult, OutcomeVerifier, default_coding_criterion,
};
pub use config::{ForgeConfig, McpServerConfig, ProviderConfig, load_forge_config};
pub use drift::*;
pub use pipeline::GoalResult;
#[cfg(test)]
pub(crate) use pipeline::{consolidate_feedback, extract_review_results, parse_delegate_requests};
pub use r#loop::*;
pub use machine::*;
pub use orchestrator::roles::ResolvedRole as AgentRoleResolved;
pub use orchestrator::{GoalConfig, ResolvedRole, RoleConfig, RoleOverride};
pub use orchestrator::{Task, TaskResult, TaskStatus};
pub use runtime::CoreRuntime;
pub use workflow::*;

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

pub type Result<T, E = CoreError> = std::result::Result<T, E>;

// ─── Injection ─────────────────────────────────────────────────

/// A message injected mid-loop into a running agent session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InjectedMessage {
    pub target_agent: String, // "coder", "all", etc.
    pub message_type: String, // "instruction", "context", "correction", "halt"
    pub content: String,
    pub created_at: String,
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
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), rx1.recv())
            .await
            .expect("timeout")
            .expect("recv error");
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), rx2.recv())
            .await
            .expect("timeout")
            .expect("recv error");
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
        let supervisor = actor::Supervisor::spawn()
            .await
            .expect("Failed to spawn Supervisor");

        let handle = actor::spawn_echo(&supervisor, "agent-1")
            .await
            .expect("spawn failed");
        assert_eq!(handle.name, "agent-1");

        let handle2 = actor::spawn_echo(&supervisor, "agent-2")
            .await
            .expect("spawn failed");
        assert_eq!(handle2.name, "agent-2");

        let response = actor::supervisor_echo_to(&supervisor, "agent-1", "test msg")
            .await
            .expect("echo failed");
        assert!(response.contains("test msg"));

        let children = actor::list_children(&supervisor)
            .await
            .expect("list failed");
        assert_eq!(children.len(), 2);

        let _ = actor::shutdown_all(&supervisor).await;
    }

    #[tokio::test]
    async fn test_core_runtime() {
        let runtime = CoreRuntime::new().await.expect("Failed to create runtime");

        let handle = runtime
            .spawn_echo_agent("test-agent")
            .await
            .expect("spawn failed");
        assert_eq!(handle.name, "test-agent");

        let response = runtime
            .echo_to("test-agent", "hello runtime")
            .await
            .expect("echo failed");
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

        assert!(
            !result.agent_results.is_empty(),
            "should have executed agents"
        );
        assert!(
            result.passed,
            "goal should pass with mock agents (all gates pass)"
        );

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
            "t1",
            "reviewer",
            "reviewer",
            "Review: PASS\nNo issues found",
            100,
        )];
        let review = extract_review_results(&results);
        assert_eq!(review.len(), 1);
        assert!(review[0].passed, "should pass when content says PASS");
    }

    #[test]
    fn test_extract_review_results_fail() {
        let results = vec![orchestrator::TaskResult::success(
            "t1",
            "reviewer",
            "reviewer",
            "Review: FAIL\nCritical issue found",
            100,
        )];
        let review = extract_review_results(&results);
        assert_eq!(review.len(), 1);
        assert!(!review[0].passed, "should fail when content says FAIL");
        assert!(
            !review[0].comments.is_empty(),
            "should have critical comments"
        );
    }

    #[test]
    fn test_consolidate_feedback() {
        let results = vec![
            orchestrator::TaskResult::success("t1", "coder", "coder", "code here", 100),
            orchestrator::TaskResult::success(
                "t2",
                "reviewer",
                "reviewer",
                "Fix the error handling",
                100,
            ),
        ];
        let feedback = consolidate_feedback(&results);
        assert!(
            feedback.contains("Fix the error handling"),
            "should include reviewer feedback"
        );
    }

    #[tokio::test]
    async fn test_checkpoint_saved_and_loaded() {
        let store =
            praxis_persistence::SqliteEventStore::in_memory().expect("Failed to create store");

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
        let mut runtime = CoreRuntime::new().await.expect("Failed to create runtime");

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
        let store =
            praxis_persistence::SqliteEventStore::in_memory().expect("Failed to create store");

        let mut runtime = CoreRuntime::new()
            .await
            .expect("Failed to create runtime")
            .with_event_store(store);

        let fake_session_id = uuid::Uuid::new_v4();
        let result = runtime
            .resume_goal(fake_session_id, None, None)
            .await
            .expect("resume_goal failed");

        assert!(
            result.is_none(),
            "should return None when no checkpoint exists"
        );

        let _ = runtime.shutdown().await;
    }

    #[tokio::test]
    async fn test_resume_goal_from_checkpoint() {
        let store =
            praxis_persistence::SqliteEventStore::in_memory().expect("Failed to create store");

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

    #[test]
    fn test_parse_delegate_requests_single() {
        let output = "Here is my analysis.\nDELEGATE:researcher:investigate async patterns in Rust 2024\nDone.";
        let requests = parse_delegate_requests(output);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, "researcher");
        assert_eq!(requests[0].1, "investigate async patterns in Rust 2024");
    }

    #[test]
    fn test_parse_delegate_requests_multiple() {
        let output =
            "DELEGATE:researcher:find async patterns\nDELEGATE:explorer:grep for AgentFactory";
        let requests = parse_delegate_requests(output);
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].0, "researcher");
        assert_eq!(requests[1].0, "explorer");
    }

    #[test]
    fn test_parse_delegate_requests_none() {
        let output = "Just a regular response with no delegation.";
        let requests = parse_delegate_requests(output);
        assert!(requests.is_empty());
    }

    #[test]
    fn test_parse_delegate_requests_empty_task_ignored() {
        let output = "DELEGATE:researcher:\nDELEGATE::task with no agent";
        let requests = parse_delegate_requests(output);
        assert!(requests.is_empty());
    }

    #[test]
    fn test_parse_delegate_requests_whitespace_trimmed() {
        let output = "DELEGATE:  researcher  :  investigate patterns  ";
        let requests = parse_delegate_requests(output);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, "researcher");
        assert_eq!(requests[0].1, "investigate patterns");
    }
}
