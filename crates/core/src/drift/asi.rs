//! Agent Stability Index (ASI) calculator.
//!
//! Combines dimension scores into a single 0–100 score.
//! Higher = more stable. Below 60 = drift detected. Below 40 = critical.

use crate::drift::metrics::{DimensionScore, DimensionStatus, MetricsCollector};

/// Weighted ASI calculator.
pub struct ASICalculator {
    /// Weights for each dimension (must sum to 1.0).
    weights: Vec<f32>,
}

impl ASICalculator {
    /// Create with default weights.
    pub fn new() -> Self {
        Self {
            weights: vec![0.10, 0.10, 0.10, 0.15, 0.15, 0.10, 0.15, 0.15],
        }
    }

    /// Create with custom weights.
    pub fn with_weights(weights: Vec<f32>) -> Self {
        assert_eq!(weights.len(), 8, "Must provide exactly 8 weights");
        let sum: f32 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.01, "Weights must sum to 1.0");
        Self { weights }
    }

    /// Calculate ASI score from dimension scores.
    ///
    /// Returns (score, breakdown).
    /// Score: 0–100, where 100 = perfectly stable.
    pub fn calculate(&self, scores: &[DimensionScore]) -> (f32, Vec<DimensionBreakdown>) {
        if scores.is_empty() {
            return (100.0, Vec::new());
        }

        let mut breakdowns = Vec::new();
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for (i, score) in scores.iter().enumerate() {
            let weight = self.weights.get(i).copied().unwrap_or(0.1);

            // Convert z-score to 0-100 scale
            // z=0 → 100 (healthy), z=1 → 80, z=2 → 60, z=3 → 40
            let normalized = (100.0 - score.value * 20.0).max(0.0).min(100.0);

            weighted_sum += normalized * weight;
            total_weight += weight;

            breakdowns.push(DimensionBreakdown {
                dimension: score.name.clone(),
                raw_score: score.value,
                normalized_score: normalized,
                weight,
                weighted_score: normalized * weight,
                z_score: score.z_score,
                status: score.status.clone(),
            });
        }

        let final_score = if total_weight > 0.0 {
            (weighted_sum / total_weight).clamp(0.0, 100.0)
        } else {
            100.0
        };

        (final_score, breakdowns)
    }

    /// Calculate ASI directly from a MetricsCollector.
    pub fn from_collector(&self, collector: &MetricsCollector) -> (f32, Vec<DimensionBreakdown>) {
        let scores = collector.dimension_scores();
        self.calculate(&scores)
    }

    /// Get the health status for a given ASI score.
    pub fn status(score: f32) -> ASIStatus {
        if score >= 80.0 {
            ASIStatus::Healthy
        } else if score >= 60.0 {
            ASIStatus::Attention
        } else if score >= 40.0 {
            ASIStatus::Drift
        } else if score >= 20.0 {
            ASIStatus::Critical
        } else {
            ASIStatus::Severe
        }
    }
}

impl Default for ASICalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Breakdown of a single dimension's contribution to ASI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DimensionBreakdown {
    pub dimension: String,
    pub raw_score: f32,
    pub normalized_score: f32,
    pub weight: f32,
    pub weighted_score: f32,
    pub z_score: f32,
    pub status: DimensionStatus,
}

/// ASI health status.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ASIStatus {
    Healthy,   // ≥80
    Attention, // 60–79
    Drift,     // 40–59
    Critical,  // 20–39
    Severe,    // <20
}

impl ASIStatus {
    /// Recommended action for this status.
    pub fn recommended_action(&self) -> &'static str {
        match self {
            ASIStatus::Healthy => "No action needed",
            ASIStatus::Attention => "Monitor closely, log warning",
            ASIStatus::Drift => "Force consolidation, reset context",
            ASIStatus::Critical => "Pause agent, notify user, upgrade model",
            ASIStatus::Severe => "Kill session, save diagnostic, don't auto-resume",
        }
    }

    /// Emoji indicator.
    pub fn emoji(&self) -> &'static str {
        match self {
            ASIStatus::Healthy => "🟢",
            ASIStatus::Attention => "🟡",
            ASIStatus::Drift => "🟠",
            ASIStatus::Critical => "🔴",
            ASIStatus::Severe => "⚫",
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::metrics::{DimensionScore, DimensionStatus};

    fn healthy_scores() -> Vec<DimensionScore> {
        vec![
            DimensionScore {
                name: "a".into(),
                value: 0.1,
                weight: 0.1,
                z_score: 0.1,
                status: DimensionStatus::Healthy,
            },
            DimensionScore {
                name: "b".into(),
                value: 0.2,
                weight: 0.1,
                z_score: 0.2,
                status: DimensionStatus::Healthy,
            },
            DimensionScore {
                name: "c".into(),
                value: 0.1,
                weight: 0.1,
                z_score: 0.1,
                status: DimensionStatus::Healthy,
            },
            DimensionScore {
                name: "d".into(),
                value: 0.15,
                weight: 0.15,
                z_score: 0.15,
                status: DimensionStatus::Healthy,
            },
            DimensionScore {
                name: "e".into(),
                value: 0.1,
                weight: 0.15,
                z_score: 0.1,
                status: DimensionStatus::Healthy,
            },
            DimensionScore {
                name: "f".into(),
                value: 0.2,
                weight: 0.1,
                z_score: 0.2,
                status: DimensionStatus::Healthy,
            },
            DimensionScore {
                name: "g".into(),
                value: 0.3,
                weight: 0.15,
                z_score: 0.0,
                status: DimensionStatus::Healthy,
            },
            DimensionScore {
                name: "h".into(),
                value: 0.5,
                weight: 0.15,
                z_score: 0.0,
                status: DimensionStatus::Healthy,
            },
        ]
    }

    fn critical_scores() -> Vec<DimensionScore> {
        vec![
            DimensionScore {
                name: "a".into(),
                value: 5.0,
                weight: 0.1,
                z_score: 5.0,
                status: DimensionStatus::Critical,
            },
            DimensionScore {
                name: "b".into(),
                value: 4.5,
                weight: 0.1,
                z_score: 4.5,
                status: DimensionStatus::Critical,
            },
            DimensionScore {
                name: "c".into(),
                value: 4.0,
                weight: 0.1,
                z_score: 4.0,
                status: DimensionStatus::Critical,
            },
            DimensionScore {
                name: "d".into(),
                value: 5.5,
                weight: 0.15,
                z_score: 5.5,
                status: DimensionStatus::Critical,
            },
            DimensionScore {
                name: "e".into(),
                value: 4.8,
                weight: 0.15,
                z_score: 4.8,
                status: DimensionStatus::Critical,
            },
            DimensionScore {
                name: "f".into(),
                value: 4.2,
                weight: 0.1,
                z_score: 4.2,
                status: DimensionStatus::Critical,
            },
            DimensionScore {
                name: "g".into(),
                value: 0.98,
                weight: 0.15,
                z_score: 0.0,
                status: DimensionStatus::Critical,
            },
            DimensionScore {
                name: "h".into(),
                value: 0.05,
                weight: 0.15,
                z_score: 0.0,
                status: DimensionStatus::Critical,
            },
        ]
    }

    #[test]
    fn test_asi_healthy() {
        let calc = ASICalculator::new();
        let (score, breakdown) = calc.calculate(&healthy_scores());
        assert!(score > 80.0, "Expected healthy ASI > 80, got {}", score);
        assert_eq!(breakdown.len(), 8);
    }

    #[test]
    fn test_asi_critical() {
        let calc = ASICalculator::new();
        let (score, _) = calc.calculate(&critical_scores());
        assert!(score < 50.0, "Expected critical ASI < 50, got {}", score);
    }

    #[test]
    fn test_asi_status() {
        assert_eq!(ASICalculator::status(90.0), ASIStatus::Healthy);
        assert_eq!(ASICalculator::status(70.0), ASIStatus::Attention);
        assert_eq!(ASICalculator::status(50.0), ASIStatus::Drift);
        assert_eq!(ASICalculator::status(30.0), ASIStatus::Critical);
        assert_eq!(ASICalculator::status(10.0), ASIStatus::Severe);
    }

    #[test]
    fn test_empty_scores() {
        let calc = ASICalculator::new();
        let (score, _) = calc.calculate(&[]);
        assert_eq!(score, 100.0);
    }

    #[test]
    fn test_status_actions() {
        assert_eq!(ASIStatus::Healthy.recommended_action(), "No action needed");
        assert_eq!(
            ASIStatus::Critical.recommended_action(),
            "Pause agent, notify user, upgrade model"
        );
    }
}
