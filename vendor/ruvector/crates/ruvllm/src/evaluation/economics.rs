//! Systems Economics Metrics - Layer 3
//!
//! Measures whether the system is worth running at scale:
//! - Latency distribution (p50, p95, p99)
//! - Cost per accepted patch
//! - Stability under load

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Duration;

/// Latency breakdown for different phases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyDistribution {
    /// Prefill latency samples (first token)
    pub prefill: LatencyStats,
    /// Decode latency samples (per token)
    pub decode: LatencyStats,
    /// Routing decision latency
    pub routing: LatencyStats,
    /// Adapter swap latency
    pub adapter_swap: LatencyStats,
    /// MicroLoRA adaptation latency
    pub micro_lora_adapt: LatencyStats,
    /// EWC++ consolidation latency
    pub consolidation: LatencyStats,
    /// Total end-to-end latency
    pub end_to_end: LatencyStats,
}

impl LatencyDistribution {
    /// Create new empty distribution
    pub fn new() -> Self {
        Self {
            prefill: LatencyStats::new(),
            decode: LatencyStats::new(),
            routing: LatencyStats::new(),
            adapter_swap: LatencyStats::new(),
            micro_lora_adapt: LatencyStats::new(),
            consolidation: LatencyStats::new(),
            end_to_end: LatencyStats::new(),
        }
    }

    /// Get summary of all latencies
    pub fn summary(&self) -> String {
        format!(
            "E2E: p50={:.1}ms p95={:.1}ms p99={:.1}ms | Prefill: {:.1}ms | Decode: {:.3}ms/tok | Route: {:.2}ms",
            self.end_to_end.p50() * 1000.0,
            self.end_to_end.p95() * 1000.0,
            self.end_to_end.p99() * 1000.0,
            self.prefill.p50() * 1000.0,
            self.decode.p50() * 1000.0,
            self.routing.p50() * 1000.0,
        )
    }
}

impl Default for LatencyDistribution {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for a latency metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    /// All samples (in seconds)
    samples: Vec<f64>,
}

impl LatencyStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    /// Add a sample (duration)
    pub fn add(&mut self, duration: Duration) {
        self.samples.push(duration.as_secs_f64());
    }

    /// Add a sample in seconds
    pub fn add_secs(&mut self, secs: f64) {
        self.samples.push(secs);
    }

    /// Get percentile value (creates sorted copy)
    fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    /// Get p50 (median)
    pub fn p50(&self) -> f64 {
        self.percentile(50.0)
    }

    /// Get p95
    pub fn p95(&self) -> f64 {
        self.percentile(95.0)
    }

    /// Get p99
    pub fn p99(&self) -> f64 {
        self.percentile(99.0)
    }

    /// Get mean
    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    /// Get count
    pub fn count(&self) -> usize {
        self.samples.len()
    }

    /// Get min
    pub fn min(&self) -> f64 {
        self.samples.iter().copied().fold(f64::INFINITY, f64::min)
    }

    /// Get max
    pub fn max(&self) -> f64 {
        self.samples
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    }
}

impl Default for LatencyStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Cost tracking for a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTracker {
    /// Input tokens consumed
    pub input_tokens: u64,
    /// Output tokens generated
    pub output_tokens: u64,
    /// Tool calls made
    pub tool_calls: u64,
    /// Retries attempted
    pub retries: u64,
    /// Cost per 1M input tokens (USD)
    pub input_cost_per_million: f64,
    /// Cost per 1M output tokens (USD)
    pub output_cost_per_million: f64,
    /// Fixed cost per tool call (USD)
    pub tool_call_cost: f64,
}

impl CostTracker {
    /// Create new tracker with Claude pricing defaults
    pub fn with_claude_pricing() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            tool_calls: 0,
            retries: 0,
            // Claude 3.5 Sonnet pricing
            input_cost_per_million: 3.0,
            output_cost_per_million: 15.0,
            tool_call_cost: 0.0,
        }
    }

    /// Create tracker for Haiku (cheaper)
    pub fn with_haiku_pricing() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            tool_calls: 0,
            retries: 0,
            input_cost_per_million: 0.25,
            output_cost_per_million: 1.25,
            tool_call_cost: 0.0,
        }
    }

    /// Create tracker for Opus (most expensive)
    pub fn with_opus_pricing() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            tool_calls: 0,
            retries: 0,
            input_cost_per_million: 15.0,
            output_cost_per_million: 75.0,
            tool_call_cost: 0.0,
        }
    }

    /// Calculate total cost in USD
    pub fn total_cost(&self) -> f64 {
        let input_cost = (self.input_tokens as f64 / 1_000_000.0) * self.input_cost_per_million;
        let output_cost = (self.output_tokens as f64 / 1_000_000.0) * self.output_cost_per_million;
        let tool_cost = self.tool_calls as f64 * self.tool_call_cost;
        input_cost + output_cost + tool_cost
    }

    /// Calculate effective cost per token
    pub fn cost_per_token(&self) -> f64 {
        let total_tokens = self.input_tokens + self.output_tokens;
        if total_tokens == 0 {
            return 0.0;
        }
        self.total_cost() / total_tokens as f64
    }

    /// Add another tracker's usage
    pub fn add(&mut self, other: &CostTracker) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.tool_calls += other.tool_calls;
        self.retries += other.retries;
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::with_claude_pricing()
    }
}

/// Stability metrics under load
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityMetrics {
    /// Total requests processed
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Out of memory events
    pub oom_events: u64,
    /// Timeout events
    pub timeout_events: u64,
    /// Queue time samples (seconds)
    pub queue_times: LatencyStats,
    /// Throughput samples (requests/second)
    pub throughput_samples: VecDeque<f64>,
    /// Maximum concurrent requests observed
    pub max_concurrent: u64,
    /// Current concurrent requests
    pub current_concurrent: u64,
}

impl StabilityMetrics {
    /// Create new metrics
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            oom_events: 0,
            timeout_events: 0,
            queue_times: LatencyStats::new(),
            throughput_samples: VecDeque::with_capacity(100),
            max_concurrent: 0,
            current_concurrent: 0,
        }
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 1.0;
        }
        self.successful_requests as f64 / self.total_requests as f64
    }

    /// Calculate failure rate
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Calculate OOM rate
    pub fn oom_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.oom_events as f64 / self.total_requests as f64
    }

    /// Calculate average throughput
    pub fn avg_throughput(&self) -> f64 {
        if self.throughput_samples.is_empty() {
            return 0.0;
        }
        self.throughput_samples.iter().sum::<f64>() / self.throughput_samples.len() as f64
    }

    /// Record a throughput sample
    pub fn record_throughput(&mut self, requests_per_second: f64) {
        if self.throughput_samples.len() >= 100 {
            self.throughput_samples.pop_front();
        }
        self.throughput_samples.push_back(requests_per_second);
    }

    /// Check if system is stable
    pub fn is_stable(&self) -> bool {
        self.success_rate() > 0.95 && self.oom_rate() < 0.01 && self.queue_times.p95() < 5.0
    }
}

impl Default for StabilityMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregated economics metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicsMetrics {
    /// Latency distribution across phases
    pub latency: LatencyDistribution,
    /// Total cost tracking
    pub cost: CostTracker,
    /// Stability metrics
    pub stability: StabilityMetrics,
    /// Number of successful tasks
    pub successful_tasks: u64,
    /// Cost per accepted patch (USD)
    pub cost_per_accepted_patch: f64,
}

impl EconomicsMetrics {
    /// Create new metrics
    pub fn new() -> Self {
        Self {
            latency: LatencyDistribution::new(),
            cost: CostTracker::with_claude_pricing(),
            stability: StabilityMetrics::new(),
            successful_tasks: 0,
            cost_per_accepted_patch: 0.0,
        }
    }

    /// Recalculate cost per accepted patch
    pub fn recalculate(&mut self) {
        if self.successful_tasks > 0 {
            self.cost_per_accepted_patch = self.cost.total_cost() / self.successful_tasks as f64;
        }
    }

    /// Check if economics are acceptable
    /// target_cost: max acceptable cost per patch in USD
    pub fn is_economical(&self, target_cost: f64) -> bool {
        self.cost_per_accepted_patch <= target_cost && self.stability.is_stable()
    }

    /// Get summary string
    pub fn summary(&self) -> String {
        format!(
            "Cost/patch: ${:.4} | Total: ${:.2} | Success: {}/{} | {}",
            self.cost_per_accepted_patch,
            self.cost.total_cost(),
            self.successful_tasks,
            self.stability.total_requests,
            self.latency.summary(),
        )
    }
}

impl Default for EconomicsMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_percentiles() {
        let mut stats = LatencyStats::new();
        for i in 1..=100 {
            stats.add_secs(i as f64 / 1000.0); // 1ms to 100ms
        }

        assert!((stats.p50() - 0.050).abs() < 0.002);
        assert!((stats.p95() - 0.095).abs() < 0.002);
        assert!((stats.p99() - 0.099).abs() < 0.002);
    }

    #[test]
    fn test_cost_calculation() {
        let mut tracker = CostTracker::with_claude_pricing();
        tracker.input_tokens = 1_000_000; // 1M tokens
        tracker.output_tokens = 100_000; // 100K tokens

        // 1M input @ $3/M = $3.00
        // 100K output @ $15/M = $1.50
        // Total = $4.50
        let cost = tracker.total_cost();
        assert!((cost - 4.50).abs() < 0.01);
    }

    #[test]
    fn test_cost_per_accepted_patch() {
        let mut metrics = EconomicsMetrics::new();
        metrics.cost.input_tokens = 10_000_000; // 10M tokens
        metrics.cost.output_tokens = 1_000_000; // 1M tokens
        metrics.successful_tasks = 100;
        metrics.recalculate();

        // Total cost = 10M * $3/M + 1M * $15/M = $30 + $15 = $45
        // Cost per patch = $45 / 100 = $0.45
        assert!((metrics.cost_per_accepted_patch - 0.45).abs() < 0.01);
    }

    #[test]
    fn test_stability_rates() {
        let mut stability = StabilityMetrics::new();
        stability.total_requests = 100;
        stability.successful_requests = 95;
        stability.failed_requests = 5;
        stability.oom_events = 1;

        assert!((stability.success_rate() - 0.95).abs() < 0.001);
        assert!((stability.oom_rate() - 0.01).abs() < 0.001);
    }

    #[test]
    fn test_haiku_vs_opus_pricing() {
        let haiku = CostTracker::with_haiku_pricing();
        let opus = CostTracker::with_opus_pricing();

        // Opus should be ~60x more expensive for input
        assert!(opus.input_cost_per_million / haiku.input_cost_per_million > 50.0);
    }
}
