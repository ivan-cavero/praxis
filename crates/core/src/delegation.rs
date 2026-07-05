//! Subagent delegation — spawn a child agent, await result, parent continues.
//!
//! Implements the Claude Code / CrewAI "Task" pattern: the parent agent
//! spawns a child with a narrowed budget, the child runs in isolation
//! (its own context window), and returns only a summary `TaskResult`.
//!
//! Budget propagation follows SentinelAgent P1 (authority narrowing)
//! and P5 (depth bounding). See `praxis_shared::budget`.

use praxis_shared::budget::Budget;
use praxis_shared::protocol::MessageKind;

use crate::agents::AgentRegistry;
use crate::actor::roles::AgentFactory;
use crate::orchestrator::task::{Task, TaskResult};
use crate::bus::EventBus;

use std::sync::Arc;
use praxis_agent_traits::provider::LLMProvider;

/// A request to delegate a sub-task to a subagent.
#[derive(Debug, Clone)]
pub struct DelegateRequest {
    /// The agent type to spawn (e.g. "researcher", "explorer").
    /// Resolved via the AgentRegistry.
    pub agent_type: String,
    /// The sub-task to execute.
    pub task: Task,
    /// The parent agent's name (for events).
    pub parent_name: String,
}

/// Result of a delegation — the child's TaskResult plus budget rollup.
#[derive(Debug, Clone)]
pub struct DelegateResult {
    /// The child agent's task result (summary only, not full history).
    pub result: TaskResult,
    /// The child's budget (for parent rollup).
    pub child_budget: Budget,
}

/// Delegate a sub-task to a subagent.
///
/// Steps:
/// 1. Resolve `agent_type` from the registry → `AgentDefinition`.
/// 2. Check the parent can spawn this type (caller validates `can_spawn`).
/// 3. Derive child budget via `parent_budget.for_child(child_inherent)`.
/// 4. Create the child agent via `AgentFactory`.
/// 5. Execute the task.
/// 6. Publish `DelegationStarted` / `DelegationCompleted` events.
/// 7. Return the result + child budget for rollup.
pub async fn delegate_to_subagent(
    request: &DelegateRequest,
    parent_budget: &Budget,
    registry: &AgentRegistry,
    provider: Option<Arc<dyn LLMProvider>>,
    bus: Option<&EventBus>,
) -> Result<DelegateResult, String> {
    // 1. Resolve agent definition from registry
    let scoped = registry
        .resolve(&request.agent_type)
        .ok_or_else(|| format!("Agent '{}' not found in registry", request.agent_type))?;

    let definition = &scoped.definition;

    // 2. Check parent can spawn this type
    if !definition.can_spawn_type(&request.agent_type) {
        // The parent must allow this type — but actually, the parent's
        // definition is what matters, not the child's. The caller should
        // check the parent's can_spawn. Here we just verify the child exists.
    }

    // 3. Derive child budget
    let child_inherent = Budget {
        max_tokens: None, // child inherent: unlimited unless .md specifies
        max_cost_usd: None,
        max_turns: definition.max_turns(),
        max_depth: definition.max_depth(),
        used_tokens: 0,
        used_cost: 0.0,
        used_turns: 0,
    };
    let child_budget = parent_budget.for_child(&child_inherent);

    // Check budget allows delegation
    if !parent_budget.can_delegate() {
        return Err(format!(
            "Parent '{}' cannot delegate: budget exhausted or max depth reached",
            request.parent_name
        ));
    }

    // 4. Publish DelegationStarted event
    let task_preview: String = request.task.description.chars().take(100).collect();
    if let Some(bus) = bus {
        bus.publish(
            MessageKind::DelegationStarted {
                parent: request.parent_name.clone(),
                child: request.agent_type.clone(),
                task_preview,
                budget_tokens: child_budget.max_tokens,
                depth: child_budget.max_depth,
            },
            "delegation",
        );
    }

    // 5. Create child agent and execute
    let resolved_role = definition.to_resolved_role();
    let start = std::time::Instant::now();

    let child_result = match (&provider, bus) {
        (Some(p), Some(b)) => {
            let agent = AgentFactory::create_with_provider_and_bus(
                &resolved_role,
                p.clone(),
                b.clone(),
            );
            agent.execute(&request.task).await
        }
        (Some(p), None) => {
            let agent = AgentFactory::create_with_provider(&resolved_role, p.clone());
            agent.execute(&request.task).await
        }
        (None, _) => {
            let agent = AgentFactory::create(&resolved_role);
            agent.execute(&request.task).await
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    // 6. Publish DelegationCompleted event
    let result_preview: String = child_result.content.chars().take(200).collect();
    let tokens_used = child_result.token_usage.total as u64;
    if let Some(bus) = bus {
        bus.publish(
            MessageKind::DelegationCompleted {
                parent: request.parent_name.clone(),
                child: request.agent_type.clone(),
                status: format!("{:?}", child_result.status),
                duration_ms,
                tokens_used,
                result_preview,
            },
            "delegation",
        );
    }

    // 7. Build child budget with usage for rollup
    let mut final_child_budget = child_budget;
    final_child_budget.used_tokens = tokens_used;
    final_child_budget.used_turns = 1;

    Ok(DelegateResult {
        result: child_result,
        child_budget: final_child_budget,
    })
}

/// Check if a parent agent can delegate to a child agent type.
/// Validates: parent exists in registry, parent can_spawn includes child type,
/// and parent's budget allows delegation.
pub fn can_delegate(
    parent_type: &str,
    child_type: &str,
    parent_budget: &Budget,
    registry: &AgentRegistry,
) -> bool {
    let Some(parent) = registry.resolve(parent_type) else {
        return false;
    };
    if !parent.definition.can_spawn_type(child_type) {
        return false;
    }
    parent_budget.can_delegate()
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::task::TaskStatus;

    #[test]
    fn test_can_delegate_architect_to_researcher() {
        let registry = AgentRegistry::builtin_only();
        let budget = Budget::unlimited();
        assert!(can_delegate("architect", "researcher", &budget, &registry));
    }

    #[test]
    fn test_cannot_delegate_coder_to_anything() {
        let registry = AgentRegistry::builtin_only();
        let budget = Budget::unlimited();
        // coder is a leaf (max_depth=0, can_spawn=[])
        assert!(!can_delegate("coder", "researcher", &budget, &registry));
    }

    #[test]
    fn test_cannot_delegate_to_unauthorized_type() {
        let registry = AgentRegistry::builtin_only();
        let budget = Budget::unlimited();
        // architect can spawn [researcher, coder], not [explorer]
        assert!(!can_delegate("architect", "explorer", &budget, &registry));
    }

    #[test]
    fn test_cannot_delegate_with_exhausted_budget() {
        let registry = AgentRegistry::builtin_only();
        let budget = Budget {
            max_tokens: Some(100),
            max_cost_usd: None,
            max_turns: 10,
            max_depth: 2,
            used_tokens: 100,
            used_cost: 0.0,
            used_turns: 0,
        };
        assert!(!can_delegate("architect", "researcher", &budget, &registry));
    }

    #[test]
    fn test_cannot_delegate_with_zero_depth() {
        let registry = AgentRegistry::builtin_only();
        let budget = Budget {
            max_tokens: None,
            max_cost_usd: None,
            max_turns: 10,
            max_depth: 0,
            used_tokens: 0,
            used_cost: 0.0,
            used_turns: 0,
        };
        assert!(!can_delegate("architect", "researcher", &budget, &registry));
    }

    #[tokio::test]
    async fn test_delegate_to_researcher_mock() {
        let registry = AgentRegistry::builtin_only();
        let parent_budget = Budget::unlimited();
        let task = Task::new("researcher", "gpt-5", "investigate async patterns");
        let request = DelegateRequest {
            agent_type: "researcher".to_string(),
            task,
            parent_name: "architect".to_string(),
        };

        let result = delegate_to_subagent(&request, &parent_budget, &registry, None, None)
            .await
            .unwrap();

        assert_eq!(result.result.role, "researcher");
        assert_eq!(result.result.status, TaskStatus::Completed);
        assert!(!result.result.content.is_empty());
    }

    #[tokio::test]
    async fn test_delegate_unknown_agent_fails() {
        let registry = AgentRegistry::builtin_only();
        let parent_budget = Budget::unlimited();
        let task = Task::new("unknown", "gpt-5", "do something");
        let request = DelegateRequest {
            agent_type: "nonexistent".to_string(),
            task,
            parent_name: "architect".to_string(),
        };

        let result = delegate_to_subagent(&request, &parent_budget, &registry, None, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delegate_with_exhausted_budget_fails() {
        let registry = AgentRegistry::builtin_only();
        let parent_budget = Budget {
            max_tokens: Some(100),
            max_cost_usd: None,
            max_turns: 10,
            max_depth: 2,
            used_tokens: 100,
            used_cost: 0.0,
            used_turns: 0,
        };
        let task = Task::new("researcher", "gpt-5", "investigate");
        let request = DelegateRequest {
            agent_type: "researcher".to_string(),
            task,
            parent_name: "architect".to_string(),
        };

        let result = delegate_to_subagent(&request, &parent_budget, &registry, None, None).await;
        assert!(result.is_err());
    }
}
