//! Completion criteria — outcome-based loop termination.
//!
//! The core principle of loop engineering: the loop stops when the OUTCOME is
//! verified, not when an ACTION is executed.
//!
//! Bad criterion:  "Stop when you call the book tool."
//! Good criterion: "Stop when the appointment is confirmed OR no slots available."
//!
//! A completion criterion verifies whether the goal was actually achieved.

use crate::orchestrator::TaskResult;
use async_trait::async_trait;
use std::sync::Arc;

// ─── Outcome Result ───────────────────────────────────────────

/// The result of evaluating a completion criterion.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum OutcomeResult {
    /// The goal is verified complete. Evidence proves it.
    Achieved {
        evidence: String,
        timestamp: String,
    },
    /// The goal is not yet achieved. Keep iterating.
    NotAchieved {
        reason: String,
    },
    /// No more options. Give up gracefully.
    Exhausted {
        reason: String,
    },
}

impl OutcomeResult {
    pub fn is_achieved(&self) -> bool {
        matches!(self, OutcomeResult::Achieved { .. })
    }

    pub fn is_exhausted(&self) -> bool {
        matches!(self, OutcomeResult::Exhausted { .. })
    }

    pub fn should_continue(&self) -> bool {
        matches!(self, OutcomeResult::NotAchieved { .. })
    }
}

// ─── Outcome Verifier Trait ───────────────────────────────────

/// Verifies whether a goal has been achieved based on agent results.
///
/// Implementations check the OUTCOME (did it work?), not the ACTION (was it
/// called?). For example, a coding verifier checks `cargo test` passes, not
/// that the coder agent ran.
#[async_trait]
pub trait OutcomeVerifier: Send + Sync {
    /// Verify whether the goal is achieved given the agent results.
    async fn verify(&self, goal: &str, results: &[TaskResult]) -> OutcomeResult;

    /// Human-readable name for this verifier.
    fn name(&self) -> &str;
}

// ─── Completion Criterion ─────────────────────────────────────

/// A completion criterion wraps an outcome verifier with configuration.
pub struct CompletionCriterion {
    verifier: Arc<dyn OutcomeVerifier>,
    /// Maximum iterations without progress before marking as Exhausted.
    pub max_stagnant_iterations: u32,
    /// Current count of iterations without progress.
    stagnant_count: u32,
}

impl CompletionCriterion {
    pub fn new(verifier: Arc<dyn OutcomeVerifier>) -> Self {
        Self {
            verifier,
            max_stagnant_iterations: 5,
            stagnant_count: 0,
        }
    }

    pub fn with_stagnant_limit(verifier: Arc<dyn OutcomeVerifier>, max: u32) -> Self {
        Self {
            verifier,
            max_stagnant_iterations: max,
            stagnant_count: 0,
        }
    }

    /// Evaluate the criterion. Tracks stagnation: if the verifier returns
    /// `NotAchieved` too many times in a row, returns `Exhausted`.
    pub async fn evaluate(&mut self, goal: &str, results: &[TaskResult]) -> OutcomeResult {
        let result = self.verifier.verify(goal, results).await;

        match &result {
            OutcomeResult::Achieved { .. } => {
                self.stagnant_count = 0;
            }
            OutcomeResult::NotAchieved { .. } => {
                self.stagnant_count += 1;
                if self.stagnant_count >= self.max_stagnant_iterations {
                    return OutcomeResult::Exhausted {
                        reason: format!(
                            "No progress for {} consecutive iterations",
                            self.stagnant_count
                        ),
                    };
                }
            }
            OutcomeResult::Exhausted { .. } => {
                // Verifier itself said give up — respect it.
            }
        }

        result
    }

    pub fn verifier_name(&self) -> &str {
        self.verifier.name()
    }

    pub fn stagnant_count(&self) -> u32 {
        self.stagnant_count
    }

    pub fn reset(&mut self) {
        self.stagnant_count = 0;
    }
}

// ─── Built-in Verifiers ───────────────────────────────────────

/// Verifier for coding tasks: checks that all agents completed successfully
/// and the reviewer/security/tester agents reported PASS.
pub struct CodingOutcomeVerifier;

#[async_trait]
impl OutcomeVerifier for CodingOutcomeVerifier {
    fn name(&self) -> &str {
        "coding-outcome"
    }

    async fn verify(&self, _goal: &str, results: &[TaskResult]) -> OutcomeResult {
        if results.is_empty() {
            return OutcomeResult::NotAchieved {
                reason: "No agent results yet".to_string(),
            };
        }

        let all_completed = results
            .iter()
            .all(|r| r.status == crate::orchestrator::TaskStatus::Completed);

        if !all_completed {
            let failed: Vec<&str> = results
                .iter()
                .filter(|r| r.status != crate::orchestrator::TaskStatus::Completed)
                .map(|r| r.role.as_str())
                .collect();
            return OutcomeResult::NotAchieved {
                reason: format!("Agents not completed: {}", failed.join(", ")),
            };
        }

        let has_reviewer = results.iter().any(|r| r.role == "reviewer");
        let has_tester = results.iter().any(|r| r.role == "tester");
        let has_security = results.iter().any(|r| r.role == "security");

        if !has_reviewer {
            return OutcomeResult::NotAchieved {
                reason: "No reviewer result yet".to_string(),
            };
        }

        let review_passed = results
            .iter()
            .filter(|r| r.role == "reviewer")
            .all(|r| {
                let lower = r.content.to_lowercase();
                !lower.contains("fail")
            });

        if !review_passed {
            return OutcomeResult::NotAchieved {
                reason: "Reviewer did not approve".to_string(),
            };
        }

        let security_clean = if has_security {
            results
                .iter()
                .filter(|r| r.role == "security")
                .all(|r| {
                    let lower = r.content.to_lowercase();
                    !lower.contains("critical")
                        || lower.contains("0 critical")
                        || lower.contains("no critical")
                })
        } else {
            true
        };

        if !security_clean {
            return OutcomeResult::NotAchieved {
                reason: "Security scan found critical issues".to_string(),
            };
        }

        let tests_pass = if has_tester {
            results
                .iter()
                .filter(|r| r.role == "tester")
                .all(|r| {
                    let lower = r.content.to_lowercase();
                    !lower.contains("fail")
                })
        } else {
            true
        };

        if !tests_pass {
            return OutcomeResult::NotAchieved {
                reason: "Tests are failing".to_string(),
            };
        }

        let evidence_parts: Vec<String> = results
            .iter()
            .map(|r| format!("{}: {}", r.role, r.content.chars().take(80).collect::<String>()))
            .collect();

        OutcomeResult::Achieved {
            evidence: format!(
                "All gates passed. Reviewer approved, security clean, tests pass.\n{}",
                evidence_parts.join("\n")
            ),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Verifier that always returns NotAchieved — for goals that need manual
/// completion or external verification.
pub struct ManualCompletionVerifier;

#[async_trait]
impl OutcomeVerifier for ManualCompletionVerifier {
    fn name(&self) -> &str {
        "manual"
    }

    async fn verify(&self, _goal: &str, _results: &[TaskResult]) -> OutcomeResult {
        OutcomeResult::NotAchieved {
            reason: "Manual completion required".to_string(),
        }
    }
}

/// Factory for creating default completion criteria.
pub fn default_coding_criterion() -> CompletionCriterion {
    CompletionCriterion::new(Arc::new(CodingOutcomeVerifier))
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::task::{TaskResult, TaskStatus};

    fn completed(role: &str, content: &str) -> TaskResult {
        TaskResult::success("t1", role, role, content, 100)
    }

    fn failed(role: &str, reason: &str) -> TaskResult {
        TaskResult::failure("t1", role, role, reason)
    }

    #[tokio::test]
    async fn test_coding_verifier_achieved() {
        let verifier = CodingOutcomeVerifier;
        let results = vec![
            completed("coder", "fn main() {}"),
            completed("reviewer", "Review: PASS. Code looks good."),
            completed("security", "Security Scan: PASS. 0 critical."),
            completed("tester", "Tests: PASS. All tests passed."),
        ];

        let result = verifier.verify("build hello world", &results).await;
        assert!(result.is_achieved(), "should be achieved: {:?}", result);
    }

    #[tokio::test]
    async fn test_coding_verifier_review_failed() {
        let verifier = CodingOutcomeVerifier;
        let results = vec![
            completed("coder", "fn main() {}"),
            completed("reviewer", "Review: FAIL. Missing error handling."),
        ];

        let result = verifier.verify("build hello world", &results).await;
        assert!(result.should_continue(), "should continue: {:?}", result);
    }

    #[tokio::test]
    async fn test_coding_verifier_security_critical() {
        let verifier = CodingOutcomeVerifier;
        let results = vec![
            completed("coder", "password = 'hardcoded'"),
            completed("reviewer", "Review: PASS."),
            completed("security", "Security Scan: FAIL. Critical: hardcoded secret."),
        ];

        let result = verifier.verify("build api", &results).await;
        assert!(result.should_continue(), "should continue due to critical: {:?}", result);
    }

    #[tokio::test]
    async fn test_coding_verifier_no_results() {
        let verifier = CodingOutcomeVerifier;
        let result = verifier.verify("goal", &[]).await;
        assert!(result.should_continue(), "empty results should continue");
    }

    #[tokio::test]
    async fn test_coding_verifier_agent_not_completed() {
        let verifier = CodingOutcomeVerifier;
        let results = vec![failed("coder", "timeout")];
        let result = verifier.verify("goal", &results).await;
        assert!(result.should_continue(), "failed agent should continue");
    }

    #[tokio::test]
    async fn test_completion_criterion_stagnation() {
        let verifier: Arc<dyn OutcomeVerifier> = Arc::new(ManualCompletionVerifier);
        let mut criterion = CompletionCriterion::with_stagnant_limit(verifier, 3);

        // First 2 iterations: NotAchieved
        let r1 = criterion.evaluate("goal", &[]).await;
        assert!(r1.should_continue());
        assert_eq!(criterion.stagnant_count(), 1);

        let r2 = criterion.evaluate("goal", &[]).await;
        assert!(r2.should_continue());
        assert_eq!(criterion.stagnant_count(), 2);

        // Third iteration: should become Exhausted
        let r3 = criterion.evaluate("goal", &[]).await;
        assert!(r3.is_exhausted(), "should be exhausted after 3 stagnant iterations");
    }

    #[tokio::test]
    async fn test_completion_criterion_resets_on_achieved() {
        let verifier: Arc<dyn OutcomeVerifier> = Arc::new(CodingOutcomeVerifier);
        let mut criterion = CompletionCriterion::with_stagnant_limit(verifier, 3);

        // No results → NotAchieved
        criterion.evaluate("goal", &[]).await;
        assert_eq!(criterion.stagnant_count(), 1);

        // Good results → Achieved, counter resets
        let results = vec![
            completed("coder", "code"),
            completed("reviewer", "PASS"),
            completed("security", "0 critical"),
            completed("tester", "PASS"),
        ];
        let result = criterion.evaluate("goal", &results).await;
        assert!(result.is_achieved());
        assert_eq!(criterion.stagnant_count(), 0);
    }

    #[test]
    fn test_outcome_result_predicates() {
        let achieved = OutcomeResult::Achieved {
            evidence: "proof".to_string(),
            timestamp: "now".to_string(),
        };
        assert!(achieved.is_achieved());
        assert!(!achieved.should_continue());
        assert!(!achieved.is_exhausted());

        let not_achieved = OutcomeResult::NotAchieved {
            reason: "not yet".to_string(),
        };
        assert!(!not_achieved.is_achieved());
        assert!(not_achieved.should_continue());

        let exhausted = OutcomeResult::Exhausted {
            reason: "no options".to_string(),
        };
        assert!(exhausted.is_exhausted());
        assert!(!exhausted.should_continue());
    }
}
