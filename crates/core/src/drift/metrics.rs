//! Metric collectors for Agent Stability Index (ASI).
//!
//! Tracks per-iteration metrics and calculates drift from baseline.
//! Uses statistical methods (z-score, rolling averages) for robust detection.

use std::collections::VecDeque;
use std::time::Instant;

// ─── Metric Sample ────────────────────────────────────────────

/// A single metric sample from one iteration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricSample {
    pub iteration: u32,
    pub timestamp: String,
    pub latency_ms: u64,
    pub output_tokens: u32,
    pub input_tokens: u32,
    pub tool_calls: u32,
    pub tool_errors: u32,
    pub output_length_chars: usize,
    pub gate_passed: bool,
    pub context_pressure: f32,
}

// ─── Metrics Collector ────────────────────────────────────────

/// Collects and analyzes agent behavior metrics.
pub struct MetricsCollector {
    /// Rolling window of recent samples.
    window: VecDeque<MetricSample>,
    /// Maximum window size.
    max_window: usize,
    /// Baseline statistics (from first N iterations).
    baseline: Option<BaselineStats>,
    /// Number of iterations to establish baseline.
    baseline_size: usize,
    /// Current iteration counter.
    current_iteration: u32,
}

/// Aggregated baseline statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BaselineStats {
    pub mean_latency: f64,
    pub std_latency: f64,
    pub mean_output_tokens: f64,
    pub std_output_tokens: f64,
    pub mean_output_length: f64,
    pub std_output_length: f64,
    pub mean_error_rate: f64,
    pub std_error_rate: f64,
    pub mean_gate_pass_rate: f64,
    pub mean_tool_usage: f64,
    pub std_tool_usage: f64,
    pub sample_count: usize,
}

/// Per-dimension drift score (0.0 = healthy, 1.0 = severe).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DimensionScore {
    pub name: String,
    pub value: f32,
    pub weight: f32,
    pub z_score: f32,
    pub status: DimensionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DimensionStatus {
    Healthy,
    Warning,
    Critical,
}

impl MetricsCollector {
    /// Create a new collector with default settings.
    pub fn new() -> Self {
        Self {
            window: VecDeque::with_capacity(50),
            max_window: 50,
            baseline: None,
            baseline_size: 10,
            current_iteration: 0,
        }
    }

    /// Create with custom baseline size.
    pub fn with_baseline_size(baseline_size: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(baseline_size * 5),
            max_window: baseline_size * 5,
            baseline: None,
            baseline_size,
            current_iteration: 0,
        }
    }

    /// Record a new metric sample.
    pub fn record(&mut self, sample: MetricSample) {
        self.current_iteration = sample.iteration;
        self.window.push_back(sample);

        // Trim window to max size
        while self.window.len() > self.max_window {
            self.window.pop_front();
        }

        // Try to establish baseline
        if self.baseline.is_none() && self.window.len() >= self.baseline_size {
            self.establish_baseline();
        }
    }

    /// Create a sample from timing data.
    pub fn start_sample(&self) -> SampleBuilder {
        SampleBuilder {
            iteration: self.current_iteration + 1,
            start: Instant::now(),
            output_tokens: 0,
            input_tokens: 0,
            tool_calls: 0,
            tool_errors: 0,
            output_length_chars: 0,
            gate_passed: true,
            context_pressure: 0.0,
        }
    }

    /// Establish baseline from the first N samples.
    fn establish_baseline(&mut self) {
        let samples: Vec<&MetricSample> = self.window.iter().take(self.baseline_size).collect();
        if samples.is_empty() {
            return;
        }

        let latencies: Vec<f64> = samples.iter().map(|s| s.latency_ms as f64).collect();
        let output_tokens: Vec<f64> = samples.iter().map(|s| s.output_tokens as f64).collect();
        let output_lengths: Vec<f64> = samples
            .iter()
            .map(|s| s.output_length_chars as f64)
            .collect();
        let error_rates: Vec<f64> = samples
            .iter()
            .map(|s| {
                if s.tool_calls > 0 {
                    s.tool_errors as f64 / s.tool_calls as f64
                } else {
                    0.0
                }
            })
            .collect();
        let gate_rates: Vec<f64> = samples
            .iter()
            .map(|s| if s.gate_passed { 1.0 } else { 0.0 })
            .collect();
        let tool_usages: Vec<f64> = samples.iter().map(|s| s.tool_calls as f64).collect();

        self.baseline = Some(BaselineStats {
            mean_latency: mean(&latencies),
            std_latency: std_dev(&latencies),
            mean_output_tokens: mean(&output_tokens),
            std_output_tokens: std_dev(&output_tokens),
            mean_output_length: mean(&output_lengths),
            std_output_length: std_dev(&output_lengths),
            mean_error_rate: mean(&error_rates),
            std_error_rate: std_dev(&error_rates),
            mean_gate_pass_rate: mean(&gate_rates),
            mean_tool_usage: mean(&tool_usages),
            std_tool_usage: std_dev(&tool_usages),
            sample_count: samples.len(),
        });
    }

    /// Get current baseline (if established).
    pub fn baseline(&self) -> Option<&BaselineStats> {
        self.baseline.as_ref()
    }

    /// Calculate dimension scores for the recent window.
    pub fn dimension_scores(&self) -> Vec<DimensionScore> {
        let baseline = match &self.baseline {
            Some(b) => b,
            None => return Vec::new(),
        };

        let recent: Vec<&MetricSample> = self.window.iter().rev().take(10).collect();
        if recent.is_empty() {
            return Vec::new();
        }

        // Compute per-dimension means once
        let mean_latency = mean(
            &recent
                .iter()
                .map(|s| s.latency_ms as f64)
                .collect::<Vec<_>>(),
        );
        let mean_output_tokens = mean(
            &recent
                .iter()
                .map(|s| s.output_tokens as f64)
                .collect::<Vec<_>>(),
        );
        let mean_output_length = mean(
            &recent
                .iter()
                .map(|s| s.output_length_chars as f64)
                .collect::<Vec<_>>(),
        );
        let mean_error_rate = mean(
            &recent
                .iter()
                .map(|s| {
                    if s.tool_calls > 0 {
                        s.tool_errors as f64 / s.tool_calls as f64
                    } else {
                        0.0
                    }
                })
                .collect::<Vec<_>>(),
        );
        let mean_gate_rate = mean(
            &recent
                .iter()
                .map(|s| if s.gate_passed { 1.0 } else { 0.0 })
                .collect::<Vec<_>>(),
        );
        let mean_tool_usage = mean(
            &recent
                .iter()
                .map(|s| s.tool_calls as f64)
                .collect::<Vec<_>>(),
        );
        let mean_pressure = mean(
            &recent
                .iter()
                .map(|s| s.context_pressure as f64)
                .collect::<Vec<_>>(),
        );

        // Compute z-scores once per dimension
        let z_latency = z_score(mean_latency, baseline.mean_latency, baseline.std_latency);
        let z_output_tokens = z_score(
            mean_output_tokens,
            baseline.mean_output_tokens,
            baseline.std_output_tokens,
        );
        let z_output_length = z_score(
            mean_output_length,
            baseline.mean_output_length,
            baseline.std_output_length,
        );
        let z_error_rate = z_score(
            mean_error_rate,
            baseline.mean_error_rate,
            baseline.std_error_rate,
        );
        let z_tool_usage = z_score(
            mean_tool_usage,
            baseline.mean_tool_usage,
            baseline.std_tool_usage,
        );

        // Compute std_dev for repetition CV
        let output_lengths: Vec<f64> = recent
            .iter()
            .map(|s| s.output_length_chars as f64)
            .collect();
        let cv = if mean(&output_lengths) > 0.0 {
            std_dev(&output_lengths) / mean(&output_lengths)
        } else {
            0.0
        };

        vec![
            DimensionScore {
                name: "latency".to_string(),
                value: z_latency.abs() as f32,
                weight: 0.10,
                z_score: z_latency as f32,
                status: classify_z_score(z_latency),
            },
            DimensionScore {
                name: "output_tokens".to_string(),
                value: z_output_tokens.abs() as f32,
                weight: 0.10,
                z_score: z_output_tokens as f32,
                status: classify_z_score(z_output_tokens),
            },
            DimensionScore {
                name: "output_length".to_string(),
                value: z_output_length.abs() as f32,
                weight: 0.10,
                z_score: z_output_length as f32,
                status: classify_z_score(z_output_length),
            },
            DimensionScore {
                name: "error_rate".to_string(),
                value: z_error_rate.abs() as f32,
                weight: 0.15,
                z_score: z_error_rate as f32,
                status: classify_z_score(z_error_rate),
            },
            DimensionScore {
                name: "gate_pass_rate".to_string(),
                value: (1.0 - mean_gate_rate) as f32,
                weight: 0.15,
                z_score: -z_score(mean_gate_rate, baseline.mean_gate_pass_rate, 0.5) as f32,
                status: classify_z_score(-z_score(
                    mean_gate_rate,
                    baseline.mean_gate_pass_rate,
                    0.5,
                )),
            },
            DimensionScore {
                name: "tool_usage".to_string(),
                value: z_tool_usage.abs() as f32,
                weight: 0.10,
                z_score: z_tool_usage as f32,
                status: classify_z_score(z_tool_usage),
            },
            DimensionScore {
                name: "context_pressure".to_string(),
                value: mean_pressure as f32,
                weight: 0.15,
                z_score: 0.0,
                status: if mean_pressure > 0.9 {
                    DimensionStatus::Critical
                } else if mean_pressure > 0.7 {
                    DimensionStatus::Warning
                } else {
                    DimensionStatus::Healthy
                },
            },
            DimensionScore {
                name: "repetition".to_string(),
                value: cv.min(1.0) as f32,
                weight: 0.15,
                z_score: 0.0,
                status: if cv < 0.1 {
                    DimensionStatus::Warning
                } else {
                    DimensionStatus::Healthy
                },
            },
        ]
    }

    /// Get the number of samples in the window.
    pub fn window_size(&self) -> usize {
        self.window.len()
    }

    /// Get current iteration.
    pub fn current_iteration(&self) -> u32 {
        self.current_iteration
    }

    /// Check if baseline is established.
    pub fn has_baseline(&self) -> bool {
        self.baseline.is_some()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Sample Builder ───────────────────────────────────────────

/// Builder for creating metric samples with timing.
pub struct SampleBuilder {
    iteration: u32,
    start: Instant,
    output_tokens: u32,
    input_tokens: u32,
    tool_calls: u32,
    tool_errors: u32,
    output_length_chars: usize,
    gate_passed: bool,
    context_pressure: f32,
}

impl SampleBuilder {
    pub fn tokens(mut self, input: u32, output: u32) -> Self {
        self.input_tokens = input;
        self.output_tokens = output;
        self
    }

    pub fn tool_result(mut self, success: bool) -> Self {
        self.tool_calls += 1;
        if !success {
            self.tool_errors += 1;
        }
        self
    }

    pub fn output_length(mut self, chars: usize) -> Self {
        self.output_length_chars = chars;
        self
    }

    pub fn gate_passed(mut self, passed: bool) -> Self {
        self.gate_passed = passed;
        self
    }

    pub fn context_pressure(mut self, pressure: f32) -> Self {
        self.context_pressure = pressure;
        self
    }

    pub fn build(self) -> MetricSample {
        MetricSample {
            iteration: self.iteration,
            timestamp: chrono::Utc::now().to_rfc3339(),
            latency_ms: self.start.elapsed().as_millis() as u64,
            output_tokens: self.output_tokens,
            input_tokens: self.input_tokens,
            tool_calls: self.tool_calls,
            tool_errors: self.tool_errors,
            output_length_chars: self.output_length_chars,
            gate_passed: self.gate_passed,
            context_pressure: self.context_pressure,
        }
    }
}

// ─── Statistical Helpers ──────────────────────────────────────

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

fn z_score(value: f64, mean: f64, std: f64) -> f64 {
    if std == 0.0 {
        return 0.0;
    }
    (value - mean) / std
}

fn classify_z_score(z: f64) -> DimensionStatus {
    let abs_z = z.abs();
    if abs_z > 2.5 {
        DimensionStatus::Critical
    } else if abs_z > 1.5 {
        DimensionStatus::Warning
    } else {
        DimensionStatus::Healthy
    }
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_new() {
        let collector = MetricsCollector::new();
        assert_eq!(collector.window_size(), 0);
        assert!(!collector.has_baseline());
    }

    #[test]
    fn test_baseline_establishment() {
        let mut collector = MetricsCollector::with_baseline_size(5);

        for i in 0..5 {
            let sample = MetricSample {
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
            };
            collector.record(sample);
        }

        assert!(collector.has_baseline());
        let baseline = collector.baseline().unwrap();
        assert_eq!(baseline.sample_count, 5);
        assert!(baseline.mean_latency > 0.0);
    }

    #[test]
    fn test_dimension_scores_healthy() {
        let mut collector = MetricsCollector::with_baseline_size(5);

        // Establish baseline with consistent data
        for i in 0..5 {
            collector.record(MetricSample {
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
            });
        }

        // Recent data matches baseline
        for i in 0..5 {
            collector.record(MetricSample {
                iteration: i + 5,
                timestamp: chrono::Utc::now().to_rfc3339(),
                latency_ms: 105,
                output_tokens: 52,
                input_tokens: 100,
                tool_calls: 2,
                tool_errors: 0,
                output_length_chars: 210,
                gate_passed: true,
                context_pressure: 0.3,
            });
        }

        let scores = collector.dimension_scores();
        assert!(!scores.is_empty());

        // All dimensions should be healthy or close
        for score in &scores {
            assert!(
                score.value < 2.0,
                "Dimension {} has high drift: {}",
                score.name,
                score.value
            );
        }
    }

    #[test]
    fn test_dimension_scores_critical() {
        let mut collector = MetricsCollector::with_baseline_size(10);

        // Establish baseline with varied but reasonable data
        for i in 0..10 {
            collector.record(MetricSample {
                iteration: i,
                timestamp: chrono::Utc::now().to_rfc3339(),
                latency_ms: 100 + i as u64 * 10, // 100-190ms
                output_tokens: 50 + i as u32 * 5,
                input_tokens: 100,
                tool_calls: 2,
                tool_errors: 0,
                output_length_chars: 200 + i as usize * 20,
                gate_passed: true,
                context_pressure: 0.3,
            });
        }

        // Recent data is dramatically different (5x latency, all errors, huge outputs)
        for i in 0..10 {
            collector.record(MetricSample {
                iteration: i + 10,
                timestamp: chrono::Utc::now().to_rfc3339(),
                latency_ms: 1000 + i as u64 * 50,
                output_tokens: 500 + i as u32 * 20,
                input_tokens: 100,
                tool_calls: 2,
                tool_errors: 2,
                output_length_chars: 5000 + i as usize * 100,
                gate_passed: false,
                context_pressure: 0.95,
            });
        }

        let scores = collector.dimension_scores();
        assert!(!scores.is_empty());

        // Latency should have very high drift (1000 vs 100 mean)
        let latency_score = scores.iter().find(|s| s.name == "latency").unwrap();
        assert!(
            latency_score.value > 1.5,
            "Expected high latency drift, got {}",
            latency_score.value
        );
    }

    #[test]
    fn test_sample_builder() {
        let collector = MetricsCollector::new();
        let sample = collector
            .start_sample()
            .tokens(100, 50)
            .tool_result(true)
            .tool_result(false)
            .output_length(300)
            .gate_passed(true)
            .context_pressure(0.4)
            .build();

        assert_eq!(sample.input_tokens, 100);
        assert_eq!(sample.output_tokens, 50);
        assert_eq!(sample.tool_calls, 2);
        assert_eq!(sample.tool_errors, 1);
        assert_eq!(sample.output_length_chars, 300);
        assert!(sample.gate_passed);
    }

    #[test]
    fn test_z_score_helpers() {
        assert!((mean(&[1.0, 2.0, 3.0]) - 2.0).abs() < 0.001);
        assert!((std_dev(&[1.0, 1.0, 1.0])).abs() < 0.001);
        assert!((z_score(3.0, 2.0, 1.0) - 1.0).abs() < 0.001);
        assert_eq!(classify_z_score(0.5), DimensionStatus::Healthy);
        assert_eq!(classify_z_score(2.0), DimensionStatus::Warning);
        assert_eq!(classify_z_score(3.0), DimensionStatus::Critical);
    }
}
