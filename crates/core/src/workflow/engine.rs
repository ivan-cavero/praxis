//! Workflow engine — custom phase sequences with conditional branching.
//!
//! When a goal references a named workflow (via `goal.workflow = "name"`), the
//! [`WorkflowEngine`] drives the phase loop instead of the hardcoded
//! `get_next_phase` / `get_agents_for_phase` functions. A workflow is an
//! ordered list of phases; after each phase, optional [`WorkflowBranch`]
//! rules can redirect to a different phase based on whether the phase's gate
//! passed or failed. When no workflow is defined, the engine falls back to
//! the default linear sequence, preserving existing behavior.
//!
//! The [`crate::workflow::GoalEngine`] resolves which workflow a goal uses,
//! falling back to `None` (default sequence) when the goal doesn't name a
//! workflow or the named workflow doesn't exist in the config.

use praxis_shared::config::{BranchCondition, WorkflowDefinition};
use praxis_shared::types::Phase;

// ─── WorkflowEngine ────────────────────────────────────────────

/// Drives the phase loop for a named workflow.
///
/// Created per goal run from a [`WorkflowDefinition`]. Call [`next_phase`]
/// after each phase completes to determine the next phase, and
/// [`agents_for_phase`] to get the agent role names for the current phase.
pub struct WorkflowEngine<'a> {
    definition: &'a WorkflowDefinition,
    /// Index into `definition.phases` for the current phase.
    current_index: usize,
}

/// The result of a gate evaluation, used for conditional branching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateOutcome {
    /// The gate passed (or no gate was configured).
    Passed,
    /// The gate failed.
    Failed,
}

impl<'a> WorkflowEngine<'a> {
    /// Create a new engine for a workflow definition.
    ///
    /// The engine starts at the first phase in the definition.
    pub fn new(definition: &'a WorkflowDefinition) -> Self {
        Self {
            definition,
            current_index: 0,
        }
    }

    /// Get the current phase.
    ///
    /// Returns `None` if the workflow has no phases (empty definition).
    pub fn current_phase(&self) -> Option<Phase> {
        self.definition
            .phases
            .get(self.current_index)
            .and_then(|p| parse_phase(&p.name))
    }

    /// Get the agent role names for the current phase.
    pub fn agents_for_phase(&self) -> &[String] {
        self.definition
            .phases
            .get(self.current_index)
            .map(|p| p.agents.as_slice())
            .unwrap_or(&[])
    }

    /// Whether the current phase should run agents in parallel.
    pub fn is_parallel(&self) -> bool {
        self.definition
            .phases
            .get(self.current_index)
            .is_some_and(|p| p.parallel)
    }

    /// The gate name for the current phase, if any.
    pub fn gate_for_phase(&self) -> Option<&str> {
        self.definition
            .phases
            .get(self.current_index)
            .and_then(|p| p.gate.as_deref())
    }

    /// Advance to the next phase based on the gate outcome.
    ///
    /// Evaluates branch rules on the current phase: the first matching branch
    /// determines the target. If no branch matches (or there are none), the
    /// next phase in the list is used. Returns the new current phase, or
    /// `Phase::Completed` if the workflow has ended.
    pub fn next_phase(&mut self, gate_outcome: GateOutcome) -> Phase {
        let Some(current) = self.definition.phases.get(self.current_index) else {
            return Phase::Completed;
        };

        // Evaluate branch rules: first match wins.
        for branch in &current.branches {
            let matches = match branch.on {
                BranchCondition::GatePassed => gate_outcome == GateOutcome::Passed,
                BranchCondition::GateFailed => gate_outcome == GateOutcome::Failed,
                BranchCondition::Always => true,
            };
            if matches {
                if let Some(target) = self.find_phase_index(&branch.to) {
                    self.current_index = target;
                    return self.current_phase().unwrap_or(Phase::Completed);
                }
                // Branch target not found — fall through to linear advance.
                tracing::warn!(
                    "Workflow branch target '{}' not found in workflow '{}'",
                    branch.to,
                    self.definition.name
                );
            }
        }

        // No branch matched — advance to the next phase in the list.
        self.current_index += 1;
        self.current_phase().unwrap_or(Phase::Completed)
    }

    /// Find the index of a phase by name in the workflow definition.
    fn find_phase_index(&self, name: &str) -> Option<usize> {
        self.definition
            .phases
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
    }
}

/// Parse a phase name string into a `Phase` enum variant.
///
/// Returns `None` if the name doesn't match any phase. Case-insensitive.
pub fn parse_phase(name: &str) -> Option<Phase> {
    let normalized = name.trim().to_lowercase();
    match normalized.as_str() {
        "idle" => Some(Phase::Idle),
        "planning" => Some(Phase::Planning),
        "researching" => Some(Phase::Researching),
        "designing" => Some(Phase::Designing),
        "implementing" => Some(Phase::Implementing),
        "reviewing" => Some(Phase::Reviewing),
        "fixing" => Some(Phase::Fixing),
        "testing" => Some(Phase::Testing),
        "securityscan" | "security_scan" | "security scan" => Some(Phase::SecurityScan),
        "finalizing" => Some(Phase::Finalizing),
        "completed" => Some(Phase::Completed),
        "failed" => Some(Phase::Failed),
        "cancelled" => Some(Phase::Cancelled),
        _ => None,
    }
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use praxis_shared::config::{WorkflowBranch, WorkflowDefinition, WorkflowPhase};

    fn phase(name: &str, agents: &[&str]) -> WorkflowPhase {
        WorkflowPhase {
            name: name.to_string(),
            agents: agents.iter().map(|s| s.to_string()).collect(),
            gate: None,
            branches: Vec::new(),
            parallel: false,
        }
    }

    fn simple_workflow() -> WorkflowDefinition {
        WorkflowDefinition {
            name: "test".to_string(),
            phases: vec![
                phase("Planning", &["architect"]),
                phase("Implementing", &["coder"]),
                phase("Reviewing", &["reviewer"]),
                phase("Testing", &["tester"]),
            ],
        }
    }

    #[test]
    fn test_workflow_engine_linear_sequence() {
        let def = simple_workflow();
        let mut engine = WorkflowEngine::new(&def);

        assert_eq!(engine.current_phase(), Some(Phase::Planning));
        assert_eq!(engine.agents_for_phase(), &["architect"]);

        let next = engine.next_phase(GateOutcome::Passed);
        assert_eq!(next, Phase::Implementing);
        assert_eq!(engine.agents_for_phase(), &["coder"]);

        let next = engine.next_phase(GateOutcome::Passed);
        assert_eq!(next, Phase::Reviewing);

        let next = engine.next_phase(GateOutcome::Passed);
        assert_eq!(next, Phase::Testing);

        // After the last phase, should complete.
        let next = engine.next_phase(GateOutcome::Passed);
        assert_eq!(next, Phase::Completed);
    }

    #[test]
    fn test_workflow_engine_conditional_branch_on_fail() {
        let def = WorkflowDefinition {
            name: "branching".to_string(),
            phases: vec![
                WorkflowPhase {
                    name: "Implementing".to_string(),
                    agents: vec!["coder".to_string()],
                    gate: Some("review".to_string()),
                    branches: vec![WorkflowBranch {
                        on: BranchCondition::GateFailed,
                        to: "Fixing".to_string(),
                    }],
                    parallel: false,
                },
                WorkflowPhase {
                    name: "Reviewing".to_string(),
                    agents: vec!["reviewer".to_string()],
                    gate: None,
                    branches: Vec::new(),
                    parallel: false,
                },
                WorkflowPhase {
                    name: "Fixing".to_string(),
                    agents: vec!["coder".to_string()],
                    gate: None,
                    branches: Vec::new(),
                    parallel: false,
                },
            ],
        };

        let mut engine = WorkflowEngine::new(&def);
        assert_eq!(engine.current_phase(), Some(Phase::Implementing));

        // Gate failed → branch to Fixing.
        let next = engine.next_phase(GateOutcome::Failed);
        assert_eq!(next, Phase::Fixing);
        assert_eq!(engine.agents_for_phase(), &["coder"]);

        // From Fixing, no branches → linear advance to Reviewing (index 2, but
        // we're at index 2 already, so next is index 3 = out of bounds = Completed).
        // Actually: Fixing is at index 2. After Fixing, linear advance goes to
        // index 3 which is out of bounds → Completed.
        let next = engine.next_phase(GateOutcome::Passed);
        assert_eq!(next, Phase::Completed);
    }

    #[test]
    fn test_workflow_engine_branch_on_pass() {
        let def = WorkflowDefinition {
            name: "skip_review".to_string(),
            phases: vec![
                WorkflowPhase {
                    name: "Implementing".to_string(),
                    agents: vec!["coder".to_string()],
                    gate: Some("fast_check".to_string()),
                    branches: vec![
                        WorkflowBranch {
                            on: BranchCondition::GatePassed,
                            to: "Testing".to_string(),
                        },
                        WorkflowBranch {
                            on: BranchCondition::GateFailed,
                            to: "Reviewing".to_string(),
                        },
                    ],
                    parallel: false,
                },
                WorkflowPhase {
                    name: "Reviewing".to_string(),
                    agents: vec!["reviewer".to_string()],
                    gate: None,
                    branches: Vec::new(),
                    parallel: false,
                },
                WorkflowPhase {
                    name: "Testing".to_string(),
                    agents: vec!["tester".to_string()],
                    gate: None,
                    branches: Vec::new(),
                    parallel: false,
                },
            ],
        };

        let mut engine = WorkflowEngine::new(&def);

        // Gate passed → skip review, go to Testing.
        let next = engine.next_phase(GateOutcome::Passed);
        assert_eq!(next, Phase::Testing);
        assert_eq!(engine.agents_for_phase(), &["tester"]);

        // Gate failed → go to Reviewing.
        let mut engine2 = WorkflowEngine::new(&def);
        let next = engine2.next_phase(GateOutcome::Failed);
        assert_eq!(next, Phase::Reviewing);
        assert_eq!(engine2.agents_for_phase(), &["reviewer"]);
    }

    #[test]
    fn test_workflow_engine_parallel_flag() {
        let def = WorkflowDefinition {
            name: "parallel".to_string(),
            phases: vec![WorkflowPhase {
                name: "Reviewing".to_string(),
                agents: vec!["reviewer1".to_string(), "reviewer2".to_string()],
                gate: None,
                branches: Vec::new(),
                parallel: true,
            }],
        };

        let engine = WorkflowEngine::new(&def);
        assert!(engine.is_parallel());
        assert_eq!(engine.agents_for_phase().len(), 2);
    }

    #[test]
    fn test_workflow_engine_gate_for_phase() {
        let def = WorkflowDefinition {
            name: "gated".to_string(),
            phases: vec![WorkflowPhase {
                name: "Reviewing".to_string(),
                agents: vec!["reviewer".to_string()],
                gate: Some("code_review".to_string()),
                branches: Vec::new(),
                parallel: false,
            }],
        };

        let engine = WorkflowEngine::new(&def);
        assert_eq!(engine.gate_for_phase(), Some("code_review"));
    }

    #[test]
    fn test_workflow_engine_empty_definition() {
        let def = WorkflowDefinition {
            name: "empty".to_string(),
            phases: Vec::new(),
        };
        let mut engine = WorkflowEngine::new(&def);
        assert_eq!(engine.current_phase(), None);
        assert_eq!(engine.next_phase(GateOutcome::Passed), Phase::Completed);
    }

    #[test]
    fn test_parse_phase_case_insensitive() {
        assert_eq!(parse_phase("Planning"), Some(Phase::Planning));
        assert_eq!(parse_phase("planning"), Some(Phase::Planning));
        assert_eq!(parse_phase("PLANNING"), Some(Phase::Planning));
        assert_eq!(parse_phase("SecurityScan"), Some(Phase::SecurityScan));
        assert_eq!(parse_phase("security_scan"), Some(Phase::SecurityScan));
        assert_eq!(parse_phase("nonexistent"), None);
    }

    #[test]
    fn test_workflow_engine_always_branch() {
        let def = WorkflowDefinition {
            name: "always".to_string(),
            phases: vec![
                WorkflowPhase {
                    name: "Planning".to_string(),
                    agents: vec!["architect".to_string()],
                    gate: None,
                    branches: vec![WorkflowBranch {
                        on: BranchCondition::Always,
                        to: "Testing".to_string(),
                    }],
                    parallel: false,
                },
                WorkflowPhase {
                    name: "Implementing".to_string(),
                    agents: vec!["coder".to_string()],
                    gate: None,
                    branches: Vec::new(),
                    parallel: false,
                },
                WorkflowPhase {
                    name: "Testing".to_string(),
                    agents: vec!["tester".to_string()],
                    gate: None,
                    branches: Vec::new(),
                    parallel: false,
                },
            ],
        };

        let mut engine = WorkflowEngine::new(&def);
        // Always branch → skip Implementing, go to Testing.
        let next = engine.next_phase(GateOutcome::Passed);
        assert_eq!(next, Phase::Testing);
    }
}
