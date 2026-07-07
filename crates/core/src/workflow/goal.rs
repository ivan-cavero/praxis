//! Goal configuration and resolution.
//!
//! The [`GoalEngine`] resolves which workflow a goal should use. Goals
//! optionally name a workflow via `goal.workflow = "name"`. If the name
//! matches a workflow defined in `[[workflows]]`, that workflow drives the
//! phase loop. Otherwise the default linear sequence is used.
//!
//! See [`crate::workflow::workflow::WorkflowEngine`] for the phase loop driver.

use praxis_shared::config::WorkflowDefinition;

/// Resolves which workflow a goal should use.
///
/// Goals optionally name a workflow via `goal.workflow`. If the name matches
/// a workflow in the config, that workflow drives the phase loop. Otherwise
/// the default linear sequence is used (returned as `None`).
pub struct GoalEngine;

impl Default for GoalEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GoalEngine {
    pub fn new() -> Self {
        Self
    }

    /// Resolve the workflow definition for a goal.
    ///
    /// Returns `Some(&WorkflowDefinition)` if the goal names a workflow that
    /// exists in `workflows`, or `None` if the goal has no workflow or the
    /// named workflow is not found (falls back to default sequence).
    pub fn resolve<'a>(
        &self,
        goal_workflow: Option<&str>,
        workflows: &'a [WorkflowDefinition],
    ) -> Option<&'a WorkflowDefinition> {
        let name = goal_workflow?;
        workflows.iter().find(|w| w.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use praxis_shared::config::{WorkflowDefinition, WorkflowPhase};

    fn make_workflow(name: &str) -> WorkflowDefinition {
        WorkflowDefinition {
            name: name.to_string(),
            phases: vec![WorkflowPhase {
                name: "Planning".to_string(),
                agents: vec!["architect".to_string()],
                gate: None,
                branches: Vec::new(),
                parallel: false,
            }],
        }
    }

    #[test]
    fn test_resolve_named_workflow() {
        let engine = GoalEngine::new();
        let workflows = vec![make_workflow("standard")];
        let resolved = engine.resolve(Some("standard"), &workflows);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().name, "standard");
    }

    #[test]
    fn test_resolve_missing_workflow_returns_none() {
        let engine = GoalEngine::new();
        let workflows = vec![make_workflow("standard")];
        assert!(engine.resolve(Some("nonexistent"), &workflows).is_none());
    }

    #[test]
    fn test_resolve_no_workflow_returns_none() {
        let engine = GoalEngine::new();
        let workflows = vec![make_workflow("standard")];
        assert!(engine.resolve(None, &workflows).is_none());
    }

    #[test]
    fn test_resolve_empty_workflows_returns_none() {
        let engine = GoalEngine::new();
        let workflows: Vec<WorkflowDefinition> = Vec::new();
        assert!(engine.resolve(Some("standard"), &workflows).is_none());
    }
}
