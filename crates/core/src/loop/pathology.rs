//! Loop pathology detection — detects stuck, oscillating, and destructive agents.
//!
//! An agent stuck in a loop can be destructive: killing processes, creating
//! processes, repeating the same failed action forever. This module detects
//! those patterns and alerts the loop controller to take action.
//!
//! Detection mechanisms:
//! - Repetition: same output/action N times in a row
//! - Oscillation: A→B→A→B phase cycling
//! - No progress: N iterations without state change
//! - Destructive behavior: process kill/create patterns, file deletion loops
//! - Token waste: token usage growing without progress
//! - Cross-model verification: when pathology suspected, ask another model

use std::collections::VecDeque;

// ─── Pathology Alert ──────────────────────────────────────────

/// A pathology alert emitted when a problematic loop pattern is detected.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PathologyAlert {
    pub kind: PathologyKind,
    pub severity: PathologySeverity,
    pub details: String,
    pub recommended_action: PathologyAction,
    pub iteration: u32,
    pub timestamp: String,
}

/// What kind of pathology was detected.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PathologyKind {
    /// Agent repeating the same output N times in a row.
    Repetition,
    /// Oscillating between two states (A→B→A→B).
    Oscillation,
    /// No progress in N iterations (output changes but no state advancement).
    NoProgress,
    /// Destructive behavior: process kill/create patterns, file deletion loops.
    DestructiveBehavior,
    /// Token usage growing without proportional progress.
    TokenWaste,
}

/// How severe the pathology is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PathologySeverity {
    /// Minor concern, log and continue.
    Warning,
    /// Significant concern, take corrective action.
    Critical,
    /// Dangerous behavior, stop immediately.
    Fatal,
}

/// What the loop controller should do about the pathology.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PathologyAction {
    /// Just log it, keep going.
    Log,
    /// Force a context reset on the agent.
    ForceContextReset,
    /// Pause the agent and notify.
    PauseAgent,
    /// Escalate to a more capable model.
    EscalateModel,
    /// Kill the session immediately.
    KillSession,
}

// ─── Loop Pathology Detector ──────────────────────────────────

/// Detects pathological loop patterns.
///
/// Tracks output hashes, phase transitions, progress markers, and destructive
/// action patterns. Call `record_iteration()` after each loop iteration.
pub struct LoopPathologyDetector {
    /// Rolling window of output hashes (per iteration).
    output_hashes: VecDeque<u64>,
    /// Rolling window of phase transitions.
    phase_history: VecDeque<String>,
    /// Iterations where output changed (progress markers).
    progress_iterations: Vec<u32>,
    /// Total iterations recorded.
    total_iterations: u32,
    /// Iterations since last meaningful progress.
    iterations_since_progress: u32,
    /// Rolling window of token counts per iteration (for token waste detection).
    token_history: VecDeque<(u32, u32)>, // (iteration, token_count)
    /// Maximum window size for hash/phase history.
    window_size: usize,
    /// Maximum allowed repetitions before alert.
    max_repetitions: u32,
    /// Maximum allowed iterations without progress before alert.
    max_no_progress: u32,
    /// Minimum iterations of rising token usage before token waste alert.
    max_token_waste_iterations: u32,
    /// All alerts emitted.
    alerts: Vec<PathologyAlert>,
    /// Destructive action patterns to watch for.
    destructive_patterns: Vec<&'static str>,
}

impl LoopPathologyDetector {
    pub fn new() -> Self {
        Self {
            output_hashes: VecDeque::with_capacity(10),
            phase_history: VecDeque::with_capacity(10),
            progress_iterations: Vec::new(),
            total_iterations: 0,
            iterations_since_progress: 0,
            token_history: VecDeque::with_capacity(20),
            window_size: 10,
            max_repetitions: 3,
            max_no_progress: 5,
            max_token_waste_iterations: 5,
            alerts: Vec::new(),
            destructive_patterns: vec![
                "kill -9",
                "kill -15",
                "taskkill",
                "rm -rf",
                "del /f",
                "format ",
                "shutdown",
                "reboot",
                "drop table",
                "drop database",
                "truncate",
                "chmod 777",
                "curl | bash",
                "wget | sh",
            ],
        }
    }

    /// Record an iteration's output and check for pathology.
    ///
    /// `token_count` is optional — pass `Some(n)` to enable token waste detection.
    /// Returns an alert if a pathology was detected, `None` if healthy.
    pub fn record_iteration(
        &mut self,
        iteration: u32,
        output: &str,
        phase: &str,
        token_count: Option<u32>,
    ) -> Option<PathologyAlert> {
        self.total_iterations = iteration + 1;

        // Check for destructive behavior FIRST — highest priority
        if let Some(alert) = self.check_destructive(output, iteration) {
            self.alerts.push(alert.clone());
            return Some(alert);
        }

        let hash = self.hash_output(output);

        // Check for repetition (same output as last iteration)
        if let Some(alert) = self.check_repetition(hash, iteration) {
            self.alerts.push(alert.clone());
            return Some(alert);
        }

        // Track progress: if hash differs from previous, it's progress
        let is_progress = self
            .output_hashes
            .back()
            .map(|&prev| prev != hash)
            .unwrap_or(true);

        if is_progress {
            self.progress_iterations.push(iteration);
            self.iterations_since_progress = 0;
        } else {
            self.iterations_since_progress += 1;
        }

        // Update windows
        self.output_hashes.push_back(hash);
        if self.output_hashes.len() > self.window_size {
            self.output_hashes.pop_front();
        }

        self.phase_history.push_back(phase.to_string());
        if self.phase_history.len() > self.window_size {
            self.phase_history.pop_front();
        }

        // Track token usage if provided
        if let Some(count) = token_count {
            self.token_history.push_back((iteration, count));
            while self.token_history.len() > 20 {
                self.token_history.pop_front();
            }
        }

        // Check for oscillation (A→B→A→B pattern in outputs)
        if let Some(alert) = self.check_oscillation(iteration) {
            self.alerts.push(alert.clone());
            return Some(alert);
        }

        // Check for no progress
        if let Some(alert) = self.check_no_progress(iteration) {
            self.alerts.push(alert.clone());
            return Some(alert);
        }

        // Check for token waste (rising token usage without progress)
        if let Some(alert) = self.check_token_waste(iteration) {
            self.alerts.push(alert.clone());
            return Some(alert);
        }

        None
    }

    /// Check if the output contains destructive patterns.
    fn check_destructive(&self, output: &str, iteration: u32) -> Option<PathologyAlert> {
        let output_lower = output.to_lowercase();
        let found = self
            .destructive_patterns
            .iter()
            .find(|pattern| output_lower.contains(*pattern));

        found.map(|pattern| PathologyAlert {
            kind: PathologyKind::DestructiveBehavior,
            severity: PathologySeverity::Fatal,
            details: format!(
                "Destructive pattern detected in output: '{}'. This can cause irreversible damage.",
                pattern
            ),
            recommended_action: PathologyAction::KillSession,
            iteration,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Check for repetition: same output hash N times in a row.
    fn check_repetition(&mut self, hash: u64, iteration: u32) -> Option<PathologyAlert> {
        let consecutive = self
            .output_hashes
            .iter()
            .rev()
            .take_while(|&&h| h == hash)
            .count() as u32;

        // +1 because the current hash hasn't been pushed yet
        if consecutive + 1 >= self.max_repetitions {
            return Some(PathologyAlert {
                kind: PathologyKind::Repetition,
                severity: PathologySeverity::Critical,
                details: format!(
                    "Same output repeated {} times in a row. Agent is stuck.",
                    consecutive + 1
                ),
                recommended_action: PathologyAction::ForceContextReset,
                iteration,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }

        None
    }

    /// Check for oscillation: repeating cycle of any period in output hashes.
    ///
    /// Detects A→B→A→B (period 2), A→B→C→A→B→C (period 3), and longer cycles
    /// up to MAX_PERIOD. Checks smaller periods first (more pathological).
    /// Skips period 1 (that's repetition, handled by `check_repetition`).
    fn check_oscillation(&self, iteration: u32) -> Option<PathologyAlert> {
        let hashes: Vec<u64> = self.output_hashes.iter().copied().collect();
        let len = hashes.len();

        // Need at least 4 hashes for the shortest cycle (period 2 × 2 cycles)
        if len < 4 {
            return None;
        }

        const MAX_PERIOD: usize = 5;
        let max_period = (len / 2).min(MAX_PERIOD);

        for period in 2..=max_period {
            let start = len - 2 * period;
            let first_half = &hashes[start..start + period];
            let second_half = &hashes[start + period..len];

            // Two identical halves = repeating cycle of length `period`
            if first_half == second_half {
                // Exclude all-same (that's repetition, not oscillation)
                let all_same = first_half.iter().all(|&h| h == first_half[0]);
                if !all_same {
                    return Some(PathologyAlert {
                        kind: PathologyKind::Oscillation,
                        severity: PathologySeverity::Critical,
                        details: format!(
                            "Oscillation detected: output cycles through {} distinct states. \
                             Agent is cycling without progress.",
                            period
                        ),
                        recommended_action: PathologyAction::EscalateModel,
                        iteration,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    });
                }
            }
        }

        None
    }

    /// Check for no progress: N iterations without meaningful change.
    fn check_no_progress(&self, iteration: u32) -> Option<PathologyAlert> {
        if self.iterations_since_progress >= self.max_no_progress {
            return Some(PathologyAlert {
                kind: PathologyKind::NoProgress,
                severity: if self.iterations_since_progress >= self.max_no_progress * 2 {
                    PathologySeverity::Critical
                } else {
                    PathologySeverity::Warning
                },
                details: format!(
                    "No progress for {} consecutive iterations. Agent may be stuck.",
                    self.iterations_since_progress
                ),
                recommended_action: if self.iterations_since_progress >= self.max_no_progress * 2 {
                    PathologyAction::PauseAgent
                } else {
                    PathologyAction::Log
                },
                iteration,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }

        None
    }

    /// Check for token waste: token usage rising without progress.
    ///
    /// Looks at the last N token counts. If they are monotonically increasing
    /// and the agent isn't making progress, it's burning tokens with no effect.
    fn check_token_waste(&self, iteration: u32) -> Option<PathologyAlert> {
        if self.token_history.len() < self.max_token_waste_iterations as usize {
            return None;
        }

        // Check if last N token counts are monotonically increasing
        let recent: Vec<u32> = self
            .token_history
            .iter()
            .rev()
            .take(self.max_token_waste_iterations as usize)
            .map(|&(_, count)| count)
            .collect();

        if recent.len() < 2 {
            return None;
        }

        let is_increasing = recent.windows(2).all(|w| w[1] >= w[0]);
        let is_no_progress = self.iterations_since_progress > 0;

        if is_increasing && is_no_progress {
            let total_rise = recent.last().unwrap_or(&0) - recent.first().unwrap_or(&0);
            return Some(PathologyAlert {
                kind: PathologyKind::TokenWaste,
                severity: PathologySeverity::Warning,
                details: format!(
                    "Token usage rising for {} iterations ({}→{}, +{} tokens) without progress.",
                    self.max_token_waste_iterations,
                    recent.first().unwrap_or(&0),
                    recent.last().unwrap_or(&0),
                    total_rise,
                ),
                recommended_action: PathologyAction::Log,
                iteration,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }

        None
    }

    /// Cross-model verification: ask another LLM whether the agent is making progress.
    ///
    /// Call this after a pathology alert is detected to get a second opinion.
    /// Returns the model's response text, or an error message if the call fails.
    pub async fn verify_with_model(
        &self,
        provider: &dyn praxis_agent_traits::provider::LLMProvider,
        goal: &str,
        last_output: &str,
        alert: &PathologyAlert,
    ) -> String {
        let prompt = format!(
            "You are a quality assurance monitor for an autonomous coding agent.\n\n\
             Goal: {}\n\n\
             The agent produced this output:\n\
             ---\n{}\n---\n\n\
             Pathology detected: {} ({:?})\n\n\
             Question: Is the agent making meaningful progress toward the goal? \
             Answer with just YES or NO, followed by a brief reason.",
            goal,
            last_output.chars().take(1000).collect::<String>(),
            alert.details,
            alert.severity,
        );

        let config = praxis_agent_traits::provider::ChatConfig {
            temperature: 0.0,
            max_tokens: 100,
            ..Default::default()
        };

        match provider
            .chat(
                &[praxis_agent_traits::provider::ChatMessage {
                    role: praxis_agent_traits::provider::ChatRole::User,
                    content: prompt,
                    tool_calls: None,
                    tool_call_id: None,
                }],
                &config,
            )
            .await
        {
            Ok(response) => {
                let content = response.content.trim().to_string();
                tracing::info!("Cross-model verification: {}", content);
                content
            }
            Err(e) => {
                tracing::warn!("Cross-model verification failed: {}", e);
                format!("Verification call failed: {}", e)
            }
        }
    }

    /// Simple hash function for output comparison.
    fn hash_output(&self, content: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Get all alerts.
    pub fn alerts(&self) -> &[PathologyAlert] {
        &self.alerts
    }

    /// Get the last alert.
    pub fn last_alert(&self) -> Option<&PathologyAlert> {
        self.alerts.last()
    }

    /// Total iterations recorded.
    pub fn total_iterations(&self) -> u32 {
        self.total_iterations
    }

    /// Iterations since last meaningful progress.
    pub fn iterations_since_progress(&self) -> u32 {
        self.iterations_since_progress
    }

    /// Clear all history.
    pub fn reset(&mut self) {
        self.output_hashes.clear();
        self.phase_history.clear();
        self.progress_iterations.clear();
        self.total_iterations = 0;
        self.iterations_since_progress = 0;
        self.token_history.clear();
        self.alerts.clear();
    }
}

impl Default for LoopPathologyDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_pathology_healthy_loop() {
        let mut detector = LoopPathologyDetector::new();

        for i in 0..10 {
            let output = format!("Unique output number {}", i);
            let alert = detector.record_iteration(i, &output, "Implementing", None);
            assert!(alert.is_none(), "should not alert on unique outputs: {:?}", alert);
        }

        assert!(detector.alerts().is_empty());
    }

    #[test]
    fn test_repetition_detection() {
        let mut detector = LoopPathologyDetector::new();
        detector.max_repetitions = 3;

        let output = "Same output every time";
        detector.record_iteration(0, output, "Implementing", None);
        detector.record_iteration(1, output, "Implementing", None);
        let alert = detector.record_iteration(2, output, "Implementing", None);

        assert!(alert.is_some());
        let alert = alert.unwrap();
        assert_eq!(alert.kind, PathologyKind::Repetition);
        assert_eq!(alert.severity, PathologySeverity::Critical);
        assert_eq!(alert.recommended_action, PathologyAction::ForceContextReset);
    }

    #[test]
    fn test_oscillation_detection() {
        let mut detector = LoopPathologyDetector::new();
        detector.max_repetitions = 100; // Disable repetition to isolate oscillation

        let output_a = "Output state A with unique content";
        let output_b = "Output state B with different content";

        detector.record_iteration(0, output_a, "Implementing", None);
        detector.record_iteration(1, output_b, "Reviewing", None);
        detector.record_iteration(2, output_a, "Implementing", None);
        let alert = detector.record_iteration(3, output_b, "Reviewing", None);

        assert!(alert.is_some());
        let alert = alert.unwrap();
        assert_eq!(alert.kind, PathologyKind::Oscillation);
        assert_eq!(alert.recommended_action, PathologyAction::EscalateModel);
    }

    #[test]
    fn test_oscillation_period_3() {
        let mut detector = LoopPathologyDetector::new();
        detector.max_repetitions = 100; // Disable repetition to isolate oscillation

        let output_a = "Output state A with unique content";
        let output_b = "Output state B with different content";
        let output_c = "Output state C with other content";

        // A→B→C→A→B→C (period 3, needs 6 hashes for 2 full cycles)
        detector.record_iteration(0, output_a, "Implementing", None);
        detector.record_iteration(1, output_b, "Reviewing", None);
        detector.record_iteration(2, output_c, "Testing", None);
        detector.record_iteration(3, output_a, "Implementing", None);
        detector.record_iteration(4, output_b, "Reviewing", None);
        let alert = detector.record_iteration(5, output_c, "Testing", None);

        assert!(alert.is_some());
        let alert = alert.unwrap();
        assert_eq!(alert.kind, PathologyKind::Oscillation);
        assert!(alert.details.contains("3 distinct states"));
    }

    #[test]
    fn test_oscillation_period_4() {
        let mut detector = LoopPathologyDetector::new();
        detector.max_repetitions = 100;

        let a = "State A unique";
        let b = "State B unique";
        let c = "State C unique";
        let d = "State D unique";

        // A→B→C→D→A→B→C→D (period 4, needs 8 hashes)
        detector.record_iteration(0, a, "P1", None);
        detector.record_iteration(1, b, "P2", None);
        detector.record_iteration(2, c, "P3", None);
        detector.record_iteration(3, d, "P4", None);
        detector.record_iteration(4, a, "P1", None);
        detector.record_iteration(5, b, "P2", None);
        detector.record_iteration(6, c, "P3", None);
        let alert = detector.record_iteration(7, d, "P4", None);

        assert!(alert.is_some());
        let alert = alert.unwrap();
        assert_eq!(alert.kind, PathologyKind::Oscillation);
        assert!(alert.details.contains("4 distinct states"));
    }

    #[test]
    fn test_no_oscillation_unique_outputs() {
        let mut detector = LoopPathologyDetector::new();
        detector.max_repetitions = 100;

        // All unique — no oscillation
        for i in 0..10 {
            let output = format!("Unique output number {}", i);
            let alert = detector.record_iteration(i, &output, "Implementing", None);
            assert!(alert.is_none(), "should not alert on unique outputs: {:?}", alert);
        }
    }

    #[test]
    fn test_no_progress_detection() {
        let mut detector = LoopPathologyDetector::new();
        detector.max_no_progress = 3;
        detector.max_repetitions = 100; // Disable repetition

        // Each output is different but we're not making real progress
        // (the detector tracks hash changes as progress, so we need same hash)
        let output = "Same output no progress";
        detector.record_iteration(0, output, "Implementing", None);
        detector.record_iteration(1, output, "Implementing", None);
        detector.record_iteration(2, output, "Implementing", None);

        let alert = detector.record_iteration(3, output, "Implementing", None);
        // Actually with same output, repetition fires first.
        // Let's test no-progress with different outputs but same phase
        // The no-progress check is about iterations_since_progress which
        // tracks hash changes. Same hash = no progress.
        assert!(alert.is_some());
    }

    #[test]
    fn test_destructive_behavior_kill() {
        let mut detector = LoopPathologyDetector::new();

        let output = "Running command: kill -9 12345";
        let alert = detector.record_iteration(0, output, "Implementing", None);

        assert!(alert.is_some());
        let alert = alert.unwrap();
        assert_eq!(alert.kind, PathologyKind::DestructiveBehavior);
        assert_eq!(alert.severity, PathologySeverity::Fatal);
        assert_eq!(alert.recommended_action, PathologyAction::KillSession);
    }

    #[test]
    fn test_destructive_behavior_rm_rf() {
        let mut detector = LoopPathologyDetector::new();

        let output = "Cleaning up: rm -rf /";
        let alert = detector.record_iteration(0, output, "Implementing", None);

        assert!(alert.is_some());
        assert_eq!(alert.unwrap().kind, PathologyKind::DestructiveBehavior);
    }

    #[test]
    fn test_destructive_behavior_drop_table() {
        let mut detector = LoopPathologyDetector::new();

        let output = "DROP TABLE users;";
        let alert = detector.record_iteration(0, output, "Implementing", None);

        assert!(alert.is_some());
        assert_eq!(alert.unwrap().kind, PathologyKind::DestructiveBehavior);
    }

    #[test]
    fn test_destructive_takes_priority_over_repetition() {
        let mut detector = LoopPathologyDetector::new();
        detector.max_repetitions = 2;

        let output = "kill -9 12345";
        detector.record_iteration(0, output, "Implementing", None);
        let alert = detector.record_iteration(1, output, "Implementing", None);

        // Should be destructive, not repetition
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().kind, PathologyKind::DestructiveBehavior);
    }

    #[test]
    fn test_reset() {
        let mut detector = LoopPathologyDetector::new();
        detector.record_iteration(0, "output one", "Implementing", None);
        detector.record_iteration(1, "output one", "Implementing", None);
        detector.record_iteration(2, "output one", "Implementing", None);

        detector.reset();
        assert!(detector.alerts().is_empty());
        assert_eq!(detector.total_iterations(), 0);
    }

    #[test]
    fn test_progress_tracking() {
        let mut detector = LoopPathologyDetector::new();
        detector.max_repetitions = 100;

        detector.record_iteration(0, "output A", "Implementing", None);
        assert_eq!(detector.iterations_since_progress(), 0);

        detector.record_iteration(1, "output A", "Implementing", None);
        assert_eq!(detector.iterations_since_progress(), 1);

        detector.record_iteration(2, "output B", "Implementing", None);
        assert_eq!(detector.iterations_since_progress(), 0);
    }
}
