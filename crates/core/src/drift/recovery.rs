//! Recovery actions: EMC, context reset, model switching, handoff.
//!
//! Based on drift severity, the system can automatically:
//! - Force episodic memory consolidation
//! - Reset agent context
//! - Switch to a more capable model
//! - Transfer to a fresh session

use crate::drift::asi::{ASIStatus, ASICalculator};
use crate::drift::metrics::MetricsCollector;

/// Recovery action triggered by drift detection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecoveryAction {
    pub kind: RecoveryKind,
    pub reason: String,
    pub severity: ASIStatus,
    pub timestamp: String,
    pub agent_id: Option<String>,
}

/// Type of recovery action.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RecoveryKind {
    /// Just log the issue, no automated action.
    LogOnly,
    /// Force episodic memory consolidation (compress context).
    ForceConsolidation,
    /// Clear context and restart from baseline + consolidated memory.
    ContextReset,
    /// Switch to a more capable (expensive) model.
    ModelUpgrade,
    /// Stop execution, notify user.
    PauseAgent,
    /// Terminate session, save diagnostic.
    KillSession,
    /// Transfer to a fresh session (handoff).
    SessionHandoff,
}

/// Orchestrates recovery actions based on ASI status.
pub struct RecoveryOrchestrator {
    /// History of recovery actions taken.
    history: Vec<RecoveryAction>,
    /// Maximum actions before escalating to KillSession.
    max_consecutive_resets: u32,
    /// Counter for consecutive context resets.
    consecutive_resets: u32,
}

impl RecoveryOrchestrator {
    /// Create a new recovery orchestrator.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            max_consecutive_resets: 3,
            consecutive_resets: 0,
        }
    }

    /// Evaluate the current ASI status and decide on recovery action.
    pub fn evaluate(&mut self, status: ASIStatus, agent_id: Option<&str>) -> Option<RecoveryAction> {
        let action = match status {
            ASIStatus::Healthy => None,

            ASIStatus::Attention => {
                Some(RecoveryAction {
                    kind: RecoveryKind::LogOnly,
                    reason: "ASI in attention zone, monitoring".to_string(),
                    severity: status.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    agent_id: agent_id.map(|s| s.to_string()),
                })
            }

            ASIStatus::Drift => {
                self.consecutive_resets += 1;
                Some(RecoveryAction {
                    kind: RecoveryKind::ForceConsolidation,
                    reason: format!(
                        "ASI drift detected (consecutive resets: {})",
                        self.consecutive_resets
                    ),
                    severity: status.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    agent_id: agent_id.map(|s| s.to_string()),
                })
            }

            ASIStatus::Critical => {
                if self.consecutive_resets >= self.max_consecutive_resets {
                    Some(RecoveryAction {
                        kind: RecoveryKind::SessionHandoff,
                        reason: format!(
                            "Max consecutive resets reached ({}), handing off",
                            self.consecutive_resets
                        ),
                        severity: status.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        agent_id: agent_id.map(|s| s.to_string()),
                    })
                } else {
                    self.consecutive_resets += 1;
                    Some(RecoveryAction {
                        kind: RecoveryKind::ModelUpgrade,
                        reason: "Critical drift, upgrading model".to_string(),
                        severity: status.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        agent_id: agent_id.map(|s| s.to_string()),
                    })
                }
            }

            ASIStatus::Severe => {
                Some(RecoveryAction {
                    kind: RecoveryKind::KillSession,
                    reason: "Severe drift detected, terminating session".to_string(),
                    severity: status.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    agent_id: agent_id.map(|s| s.to_string()),
                })
            }
        };

        if let Some(ref action) = action {
            self.history.push(action.clone());
        }

        action
    }

    /// Reset the consecutive reset counter (call after successful recovery).
    pub fn reset_counter(&mut self) {
        self.consecutive_resets = 0;
    }

    /// Get history of recovery actions.
    pub fn history(&self) -> &[RecoveryAction] {
        &self.history
    }

    /// Get the last recovery action.
    pub fn last_action(&self) -> Option<&RecoveryAction> {
        self.history.last()
    }
}

impl Default for RecoveryOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

// ─── DriftGuard ───────────────────────────────────────────────

/// Combines MetricsCollector + ASICalculator + RecoveryOrchestrator
/// into a single guard that monitors and recovers from drift.
pub struct DriftGuard {
    pub metrics: MetricsCollector,
    pub asi_calculator: ASICalculator,
    pub recovery: RecoveryOrchestrator,
    /// Threshold for triggering recovery (ASI score).
    pub recovery_threshold: f32,
}

impl DriftGuard {
    /// Create a new drift guard.
    pub fn new() -> Self {
        Self {
            metrics: MetricsCollector::new(),
            asi_calculator: ASICalculator::new(),
            recovery: RecoveryOrchestrator::new(),
            recovery_threshold: 60.0,
        }
    }

    /// Record a metric sample and evaluate for drift.
    pub fn record_and_evaluate(&mut self, sample: crate::drift::metrics::MetricSample, agent_id: Option<&str>) -> Option<DriftReport> {
        self.metrics.record(sample);

        if !self.metrics.has_baseline() {
            return None;
        }

        let (asi_score, breakdown) = self.asi_calculator.from_collector(&self.metrics);
        let status = ASICalculator::status(asi_score);

        // Trigger recovery if below threshold
        let recovery_action = if asi_score < self.recovery_threshold {
            self.recovery.evaluate(status.clone(), agent_id)
        } else {
            self.recovery.reset_counter();
            None
        };

        Some(DriftReport {
            asi_score,
            status,
            breakdown,
            recovery_action,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Force a drift evaluation (e.g., on phase transition).
    pub fn force_evaluate(&self, _agent_id: Option<&str>) -> Option<DriftReport> {
        if !self.metrics.has_baseline() {
            return None;
        }

        let (asi_score, breakdown) = self.asi_calculator.from_collector(&self.metrics);
        let status = ASICalculator::status(asi_score);

        Some(DriftReport {
            asi_score,
            status,
            breakdown,
            recovery_action: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }
}

impl Default for DriftGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// A report of the current drift status.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DriftReport {
    pub asi_score: f32,
    pub status: ASIStatus,
    pub breakdown: Vec<crate::drift::asi::DimensionBreakdown>,
    pub recovery_action: Option<RecoveryAction>,
    pub timestamp: String,
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::metrics::MetricSample;

    #[test]
    fn test_recovery_log_only() {
        let mut recovery = RecoveryOrchestrator::new();
        let action = recovery.evaluate(ASIStatus::Attention, Some("coder"));
        assert!(action.is_some());
        assert_eq!(action.unwrap().kind, RecoveryKind::LogOnly);
    }

    #[test]
    fn test_recovery_consolidation() {
        let mut recovery = RecoveryOrchestrator::new();
        let action = recovery.evaluate(ASIStatus::Drift, Some("coder"));
        assert!(action.is_some());
        assert_eq!(action.unwrap().kind, RecoveryKind::ForceConsolidation);
    }

    #[test]
    fn test_recovery_model_upgrade() {
        let mut recovery = RecoveryOrchestrator::new();
        let action = recovery.evaluate(ASIStatus::Critical, Some("coder"));
        assert!(action.is_some());
        assert_eq!(action.unwrap().kind, RecoveryKind::ModelUpgrade);
    }

    #[test]
    fn test_recovery_handoff_after_max_resets() {
        let mut recovery = RecoveryOrchestrator::new();
        // Simulate 3 consecutive drifts → critical
        recovery.evaluate(ASIStatus::Drift, None); // consecutive = 1
        recovery.evaluate(ASIStatus::Drift, None); // consecutive = 2
        recovery.evaluate(ASIStatus::Drift, None); // consecutive = 3
        let action = recovery.evaluate(ASIStatus::Critical, None);
        assert_eq!(action.unwrap().kind, RecoveryKind::SessionHandoff);
    }

    #[test]
    fn test_recovery_kill_session() {
        let mut recovery = RecoveryOrchestrator::new();
        let action = recovery.evaluate(ASIStatus::Severe, Some("coder"));
        assert_eq!(action.unwrap().kind, RecoveryKind::KillSession);
    }

    #[test]
    fn test_drift_guard_healthy() {
        let mut guard = DriftGuard::new();
        // Record 15 samples to establish baseline + evaluate
        for i in 0..15 {
            let sample = MetricSample {
                iteration: i,
                timestamp: chrono::Utc::now().to_rfc3339(),
                latency_ms: 100,
                output_tokens: 50,
                input_tokens: 100,
                tool_calls: 2,
                tool_errors: 0,
                output_length_chars: 200,
                gate_passed: true,
                context_pressure: 0.3,
            };
            let report = guard.record_and_evaluate(sample, Some("coder"));
            if i >= 9 {
                // After baseline established
                let report = report.unwrap();
                assert!(report.asi_score > 60.0, "Expected healthy ASI, got {}", report.asi_score);
                assert!(report.recovery_action.is_none());
            }
        }
    }

    #[test]
    fn test_drift_guard_detects_drift() {
        let mut guard = DriftGuard::new();
        guard.recovery_threshold = 80.0; // Higher threshold for this test

        // Establish baseline with varied good metrics
        for i in 0..10 {
            guard.record_and_evaluate(MetricSample {
                iteration: i,
                timestamp: chrono::Utc::now().to_rfc3339(),
                latency_ms: 100 + i as u64 * 10,
                output_tokens: 50 + i as u32 * 5,
                input_tokens: 100,
                tool_calls: 2,
                tool_errors: 0,
                output_length_chars: 200 + i as usize * 20,
                gate_passed: true,
                context_pressure: 0.3,
            }, None);
        }

        // Now record severely degraded metrics
        let mut recovery_triggered = false;
        for i in 0..10 {
            let report = guard.record_and_evaluate(MetricSample {
                iteration: i + 10,
                timestamp: chrono::Utc::now().to_rfc3339(),
                latency_ms: 5000,
                output_tokens: 500,
                input_tokens: 100,
                tool_calls: 2,
                tool_errors: 2,
                output_length_chars: 10000,
                gate_passed: false,
                context_pressure: 0.95,
            }, Some("coder"));

            if let Some(report) = report {
                if report.recovery_action.is_some() {
                    recovery_triggered = true;
                    break;
                }
            }
        }

        // Check if recovery was triggered at some point
        assert!(recovery_triggered || guard.recovery.history().len() > 0,
            "Expected drift detection with recovery threshold 80");
    }
}