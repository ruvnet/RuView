//! Real-time Optimization for LLM Inference
//!
//! Features:
//! - Dynamic batch sizing based on latency targets
//! - KV cache pressure management
//! - Token budget allocation
//! - Speculative decoding integration

use crate::error::{Result, RuvLLMError};
use crate::optimization::metrics::{InferenceMetrics, MetricsSnapshot};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Configuration for the realtime optimizer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeConfig {
    /// Target latency for TTFT (milliseconds)
    pub latency_target_ms: f32,
    /// Target throughput (tokens per second)
    pub throughput_target_tps: f32,
    /// Minimum batch size
    pub min_batch_size: usize,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// KV cache pressure threshold (0.0 - 1.0)
    pub kv_cache_pressure_threshold: f32,
    /// Enable speculative decoding
    pub enable_speculative: bool,
    /// Speculative decoding configuration
    pub speculative: SpeculativeConfig,
    /// Batch sizing strategy
    pub batch_strategy: BatchSizeStrategy,
    /// KV cache pressure policy
    pub kv_policy: KvCachePressurePolicy,
    /// Maximum memory budget (bytes)
    pub max_memory_bytes: usize,
    /// Optimization interval (how often to recompute decisions)
    pub optimization_interval_ms: u64,
}

impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            latency_target_ms: 100.0,
            throughput_target_tps: 50.0,
            min_batch_size: 1,
            max_batch_size: 64,
            kv_cache_pressure_threshold: 0.8,
            enable_speculative: true, // Enabled by default for 2-3x decode speedup
            speculative: SpeculativeConfig::default(),
            batch_strategy: BatchSizeStrategy::Adaptive,
            kv_policy: KvCachePressurePolicy::Evict,
            max_memory_bytes: 8 * 1024 * 1024 * 1024, // 8GB default
            optimization_interval_ms: 100,
        }
    }
}

/// Batch size selection strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchSizeStrategy {
    /// Fixed batch size
    Fixed,
    /// Adaptive based on latency
    Adaptive,
    /// Aggressive (maximize throughput)
    Aggressive,
    /// Conservative (minimize latency)
    Conservative,
    /// Hybrid (balance throughput and latency)
    Hybrid,
}

impl Default for BatchSizeStrategy {
    fn default() -> Self {
        Self::Adaptive
    }
}

/// Policy for handling KV cache pressure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KvCachePressurePolicy {
    /// Evict oldest entries
    Evict,
    /// Quantize more aggressively
    Quantize,
    /// Reject new requests
    Reject,
    /// Spill to disk
    Spill,
    /// Hybrid approach
    Hybrid,
}

impl Default for KvCachePressurePolicy {
    fn default() -> Self {
        Self::Evict
    }
}

/// Configuration for speculative decoding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeConfig {
    /// Draft model identifier
    pub draft_model: Option<String>,
    /// Number of speculative tokens
    pub num_speculative_tokens: usize,
    /// Acceptance threshold
    pub acceptance_threshold: f32,
    /// Enable tree-based speculation
    pub tree_speculation: bool,
    /// Maximum tree depth
    pub max_tree_depth: usize,
}

impl Default for SpeculativeConfig {
    fn default() -> Self {
        Self {
            draft_model: None,
            num_speculative_tokens: 4,
            acceptance_threshold: 0.8,
            tree_speculation: false,
            max_tree_depth: 3,
        }
    }
}

/// Token budget allocation for a request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudgetAllocation {
    /// Request identifier
    pub request_id: String,
    /// Maximum tokens to generate
    pub max_tokens: usize,
    /// Priority level (higher = more resources)
    pub priority: f32,
    /// Deadline (optional)
    pub deadline: Option<Duration>,
    /// Allocated batch slot
    pub batch_slot: Option<usize>,
    /// Estimated completion time
    pub estimated_completion_ms: f32,
}

/// Request representation for optimization
#[derive(Debug, Clone)]
pub struct Request {
    /// Request identifier
    pub id: String,
    /// Input token count
    pub input_tokens: usize,
    /// Maximum output tokens
    pub max_output_tokens: usize,
    /// Priority (0.0 - 1.0)
    pub priority: f32,
    /// Arrival time
    pub arrival_time: Instant,
    /// Deadline (optional)
    pub deadline: Option<Duration>,
}

/// Optimization decision output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationDecision {
    /// Recommended batch size
    pub batch_size: usize,
    /// Whether to evict KV cache
    pub should_evict_kv_cache: bool,
    /// Number of entries to evict
    pub evict_count: usize,
    /// Whether to enable speculative decoding
    pub enable_speculative: bool,
    /// Token budgets for requests
    pub token_budgets: Vec<TokenBudgetAllocation>,
    /// Quantization recommendation
    pub quantization_recommendation: Option<String>,
    /// Estimated latency for current batch
    pub estimated_latency_ms: f32,
    /// Estimated throughput for current batch
    pub estimated_tps: f32,
    /// Confidence in this decision (0.0 - 1.0)
    pub confidence: f32,
    /// Reason for this decision
    pub reason: String,
}

impl Default for OptimizationDecision {
    fn default() -> Self {
        Self {
            batch_size: 1,
            should_evict_kv_cache: false,
            evict_count: 0,
            enable_speculative: false,
            token_budgets: Vec::new(),
            quantization_recommendation: None,
            estimated_latency_ms: 0.0,
            estimated_tps: 0.0,
            confidence: 0.5,
            reason: "Default decision".to_string(),
        }
    }
}

/// Real-time optimizer for LLM inference
pub struct RealtimeOptimizer {
    /// Configuration
    config: RwLock<RealtimeConfig>,
    /// Current batch size
    current_batch_size: AtomicUsize,
    /// Current KV cache pressure (0.0 - 1.0)
    kv_cache_pressure: RwLock<f32>,
    /// Recent latency measurements
    recent_latencies: RwLock<VecDeque<f32>>,
    /// Recent throughput measurements
    recent_throughputs: RwLock<VecDeque<f32>>,
    /// Whether speculative decoding is active
    speculative_active: AtomicBool,
    /// Draft model identifier (if loaded)
    draft_model: RwLock<Option<String>>,
    /// Last optimization time
    last_optimization: RwLock<Instant>,
    /// Pending requests
    pending_requests: RwLock<Vec<Request>>,
    /// Current memory usage
    current_memory_bytes: AtomicUsize,
}

impl RealtimeOptimizer {
    /// Create a new realtime optimizer
    pub fn new(config: RealtimeConfig) -> Self {
        let initial_batch_size = match config.batch_strategy {
            BatchSizeStrategy::Fixed => config.max_batch_size,
            BatchSizeStrategy::Aggressive => config.max_batch_size,
            BatchSizeStrategy::Conservative => config.min_batch_size,
            _ => (config.min_batch_size + config.max_batch_size) / 2,
        };

        Self {
            config: RwLock::new(config),
            current_batch_size: AtomicUsize::new(initial_batch_size),
            kv_cache_pressure: RwLock::new(0.0),
            recent_latencies: RwLock::new(VecDeque::with_capacity(100)),
            recent_throughputs: RwLock::new(VecDeque::with_capacity(100)),
            speculative_active: AtomicBool::new(false),
            draft_model: RwLock::new(None),
            last_optimization: RwLock::new(Instant::now()),
            pending_requests: RwLock::new(Vec::new()),
            current_memory_bytes: AtomicUsize::new(0),
        }
    }

    /// Optimize batch size based on recent latency measurements
    pub fn optimize_batch_size(&self, recent_latencies: &[f32]) -> usize {
        let config = self.config.read();

        // Update internal latency tracking
        {
            let mut latencies = self.recent_latencies.write();
            for &l in recent_latencies {
                if latencies.len() >= 100 {
                    latencies.pop_front();
                }
                latencies.push_back(l);
            }
        }

        let current_batch = self.current_batch_size.load(Ordering::Relaxed);

        let new_batch_size = match config.batch_strategy {
            BatchSizeStrategy::Fixed => current_batch,

            BatchSizeStrategy::Adaptive => self.adaptive_batch_size(&config, recent_latencies),

            BatchSizeStrategy::Aggressive => {
                // Maximize batch size while staying under latency target
                let avg_latency = self.average_latency();
                if avg_latency < config.latency_target_ms * 0.7 {
                    (current_batch + 4).min(config.max_batch_size)
                } else if avg_latency > config.latency_target_ms {
                    (current_batch.saturating_sub(2)).max(config.min_batch_size)
                } else {
                    current_batch
                }
            }

            BatchSizeStrategy::Conservative => {
                // Minimize latency, slowly increase batch size
                let avg_latency = self.average_latency();
                if avg_latency < config.latency_target_ms * 0.5 {
                    (current_batch + 1).min(config.max_batch_size)
                } else if avg_latency > config.latency_target_ms * 0.8 {
                    (current_batch.saturating_sub(1)).max(config.min_batch_size)
                } else {
                    current_batch
                }
            }

            BatchSizeStrategy::Hybrid => {
                // Balance throughput and latency using a utility function
                self.hybrid_batch_size(&config)
            }
        };

        self.current_batch_size
            .store(new_batch_size, Ordering::Relaxed);
        new_batch_size
    }

    /// Adaptive batch sizing based on PID-like control
    fn adaptive_batch_size(&self, config: &RealtimeConfig, recent_latencies: &[f32]) -> usize {
        let current_batch = self.current_batch_size.load(Ordering::Relaxed);

        if recent_latencies.is_empty() {
            return current_batch;
        }

        let avg_latency: f32 = recent_latencies.iter().sum::<f32>() / recent_latencies.len() as f32;
        let target = config.latency_target_ms;

        // Error term (positive = too slow, negative = too fast)
        let error = avg_latency - target;
        let error_ratio = error / target;

        // PID-like adjustment
        let adjustment = if error_ratio.abs() < 0.1 {
            // Within 10% of target, no change
            0
        } else if error_ratio > 0.0 {
            // Too slow, reduce batch size
            let reduction = (error_ratio * 4.0).ceil() as i32;
            -reduction.min(4)
        } else {
            // Too fast, increase batch size
            let increase = (-error_ratio * 2.0).ceil() as i32;
            increase.min(2)
        };

        let new_batch = (current_batch as i32 + adjustment)
            .max(config.min_batch_size as i32)
            .min(config.max_batch_size as i32) as usize;

        new_batch
    }

    /// Hybrid batch sizing using utility maximization
    fn hybrid_batch_size(&self, config: &RealtimeConfig) -> usize {
        let current_batch = self.current_batch_size.load(Ordering::Relaxed);
        let avg_latency = self.average_latency();
        let avg_throughput = self.average_throughput();

        // Utility = alpha * throughput_normalized - beta * latency_normalized
        let alpha = 0.6; // Weight for throughput
        let beta = 0.4; // Weight for latency

        let latency_normalized = (avg_latency / config.latency_target_ms).min(2.0);
        let throughput_normalized = (avg_throughput / config.throughput_target_tps).min(2.0);

        let current_utility = alpha * throughput_normalized - beta * latency_normalized;

        // Try neighboring batch sizes and pick the one with best predicted utility
        let candidates = [
            current_batch.saturating_sub(2),
            current_batch.saturating_sub(1),
            current_batch,
            current_batch + 1,
            current_batch + 2,
        ];

        let mut best_batch = current_batch;
        let mut best_utility = current_utility;

        for &candidate in &candidates {
            if candidate < config.min_batch_size || candidate > config.max_batch_size {
                continue;
            }

            // Predict utility for this batch size
            let batch_ratio = candidate as f32 / current_batch as f32;
            let predicted_latency = avg_latency * batch_ratio.sqrt(); // Latency grows sub-linearly
            let predicted_throughput = avg_throughput * batch_ratio; // Throughput grows linearly

            let pred_latency_norm = (predicted_latency / config.latency_target_ms).min(2.0);
            let pred_throughput_norm =
                (predicted_throughput / config.throughput_target_tps).min(2.0);

            let predicted_utility = alpha * pred_throughput_norm - beta * pred_latency_norm;

            if predicted_utility > best_utility {
                best_utility = predicted_utility;
                best_batch = candidate;
            }
        }

        best_batch
    }

    /// Check if KV cache eviction is needed
    pub fn should_evict_kv_cache(&self) -> bool {
        let config = self.config.read();
        let pressure = *self.kv_cache_pressure.read();
        pressure >= config.kv_cache_pressure_threshold
    }

    /// Update KV cache pressure
    pub fn update_kv_cache_pressure(&self, pressure: f32) {
        *self.kv_cache_pressure.write() = pressure.clamp(0.0, 1.0);
    }

    /// Get KV cache pressure
    pub fn kv_cache_pressure(&self) -> f32 {
        *self.kv_cache_pressure.read()
    }

    /// Allocate token budgets for a set of requests
    pub fn allocate_token_budget(&self, requests: &[Request]) -> Vec<TokenBudgetAllocation> {
        let config = self.config.read();
        let batch_size = self.current_batch_size.load(Ordering::Relaxed);
        let memory_budget = config.max_memory_bytes;

        // Sort requests by priority and deadline
        let mut sorted_requests: Vec<(usize, &Request)> = requests.iter().enumerate().collect();
        sorted_requests.sort_by(|(_, a), (_, b)| {
            // Higher priority first
            let priority_cmp = b
                .priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal);
            if priority_cmp != std::cmp::Ordering::Equal {
                return priority_cmp;
            }
            // Earlier deadline first
            match (&a.deadline, &b.deadline) {
                (Some(da), Some(db)) => da.cmp(db),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });

        let mut allocations = Vec::with_capacity(requests.len());
        let mut total_memory = 0usize;
        let mut assigned_slots = 0usize;

        for (original_idx, request) in sorted_requests {
            // Estimate memory for this request
            let estimated_memory = self.estimate_request_memory(request);

            let (max_tokens, batch_slot) = if assigned_slots < batch_size
                && total_memory + estimated_memory <= memory_budget
            {
                total_memory += estimated_memory;
                let slot = assigned_slots;
                assigned_slots += 1;
                (request.max_output_tokens, Some(slot))
            } else {
                // Request is queued, reduced token budget
                let reduced = (request.max_output_tokens / 2).max(1);
                (reduced, None)
            };

            let estimated_completion = self.estimate_completion_time(request, batch_slot);

            allocations.push((
                original_idx,
                TokenBudgetAllocation {
                    request_id: request.id.clone(),
                    max_tokens,
                    priority: request.priority,
                    deadline: request.deadline,
                    batch_slot,
                    estimated_completion_ms: estimated_completion,
                },
            ));
        }

        // Sort back to original order
        allocations.sort_by_key(|(idx, _)| *idx);
        allocations.into_iter().map(|(_, alloc)| alloc).collect()
    }

    /// Estimate memory requirement for a request
    fn estimate_request_memory(&self, request: &Request) -> usize {
        // Rough estimate: 2 bytes per token for KV cache (FP16)
        // Plus overhead for attention computation
        let kv_memory = (request.input_tokens + request.max_output_tokens) * 2 * 128; // head_dim
        let attention_overhead = request.input_tokens * 32; // Attention scores
        kv_memory + attention_overhead
    }

    /// Estimate completion time for a request
    fn estimate_completion_time(&self, request: &Request, batch_slot: Option<usize>) -> f32 {
        let avg_tps = self.average_throughput().max(1.0);
        let base_time = request.max_output_tokens as f32 / avg_tps * 1000.0;

        // Add queue time if not in current batch
        if batch_slot.is_none() {
            let queue_size = self.pending_requests.read().len();
            base_time + (queue_size as f32 * self.average_latency())
        } else {
            base_time
        }
    }

    /// Enable speculative decoding
    pub fn enable_speculative_decoding(&self, draft_model: &str) {
        *self.draft_model.write() = Some(draft_model.to_string());
        self.speculative_active.store(true, Ordering::Relaxed);
    }

    /// Disable speculative decoding
    pub fn disable_speculative_decoding(&self) {
        self.speculative_active.store(false, Ordering::Relaxed);
    }

    /// Update speculation statistics for learning/monitoring
    ///
    /// This records the acceptance rate of speculative decoding rounds
    /// to help tune the lookahead parameter adaptively.
    ///
    /// # Arguments
    /// * `accepted_count` - Number of draft tokens that were accepted
    /// * `total_drafted` - Total number of draft tokens generated
    pub fn update_speculation_stats(&self, accepted_count: usize, total_drafted: usize) {
        if total_drafted == 0 {
            return;
        }

        // Calculate acceptance rate
        let acceptance_rate = accepted_count as f32 / total_drafted as f32;

        // Use acceptance rate to adjust future speculative decoding behavior
        // High acceptance (>0.8) suggests we can increase lookahead
        // Low acceptance (<0.5) suggests we should reduce lookahead or disable
        let mut config = self.config.write();

        if acceptance_rate > 0.9 && config.speculative.num_speculative_tokens < 8 {
            // Excellent acceptance, try more tokens
            config.speculative.num_speculative_tokens += 1;
        } else if acceptance_rate < 0.3 && config.speculative.num_speculative_tokens > 2 {
            // Poor acceptance, reduce speculation
            config.speculative.num_speculative_tokens -= 1;
        }

        // Update acceptance threshold based on recent performance
        // This implements a simple exponential moving average
        let alpha = 0.1; // Learning rate
        config.speculative.acceptance_threshold =
            config.speculative.acceptance_threshold * (1.0 - alpha) + acceptance_rate * alpha;
    }

    /// Check if speculative decoding is active
    pub fn is_speculative_active(&self) -> bool {
        self.speculative_active.load(Ordering::Relaxed)
    }

    /// Get the draft model identifier
    pub fn draft_model(&self) -> Option<String> {
        self.draft_model.read().clone()
    }

    /// Record a latency measurement
    pub fn record_latency(&self, latency_ms: f32) {
        let mut latencies = self.recent_latencies.write();
        if latencies.len() >= 100 {
            latencies.pop_front();
        }
        latencies.push_back(latency_ms);
    }

    /// Record a throughput measurement
    pub fn record_throughput(&self, tps: f32) {
        let mut throughputs = self.recent_throughputs.write();
        if throughputs.len() >= 100 {
            throughputs.pop_front();
        }
        throughputs.push_back(tps);
    }

    /// Get average latency
    pub fn average_latency(&self) -> f32 {
        let latencies = self.recent_latencies.read();
        if latencies.is_empty() {
            return 50.0; // Default estimate
        }
        latencies.iter().sum::<f32>() / latencies.len() as f32
    }

    /// Get average throughput
    pub fn average_throughput(&self) -> f32 {
        let throughputs = self.recent_throughputs.read();
        if throughputs.is_empty() {
            return 50.0; // Default estimate
        }
        throughputs.iter().sum::<f32>() / throughputs.len() as f32
    }

    /// Update memory usage
    pub fn update_memory_usage(&self, bytes: usize) {
        self.current_memory_bytes.store(bytes, Ordering::Relaxed);
    }

    /// Get memory pressure (0.0 - 1.0)
    pub fn memory_pressure(&self) -> f32 {
        let config = self.config.read();
        let current = self.current_memory_bytes.load(Ordering::Relaxed);
        current as f32 / config.max_memory_bytes as f32
    }

    /// Make a comprehensive optimization decision
    pub fn optimize(&self, metrics: &InferenceMetrics) -> OptimizationDecision {
        let config = self.config.read();
        let snapshot = metrics.snapshot();

        // Check if we need to optimize
        let last_opt = *self.last_optimization.read();
        if last_opt.elapsed().as_millis() < config.optimization_interval_ms as u128 {
            return OptimizationDecision {
                batch_size: self.current_batch_size.load(Ordering::Relaxed),
                confidence: 0.3,
                reason: "Skipping optimization (too recent)".to_string(),
                ..Default::default()
            };
        }
        *self.last_optimization.write() = Instant::now();

        // Determine batch size
        let latencies: Vec<f32> = self.recent_latencies.read().iter().copied().collect();
        let batch_size = self.optimize_batch_size(&latencies);

        // Determine KV cache action
        let kv_pressure = *self.kv_cache_pressure.read();
        let (should_evict, evict_count) = if kv_pressure >= config.kv_cache_pressure_threshold {
            let excess_pressure = kv_pressure - config.kv_cache_pressure_threshold;
            let evict_ratio =
                (excess_pressure / (1.0 - config.kv_cache_pressure_threshold)).min(0.5);
            (true, (evict_ratio * 1000.0) as usize) // Evict proportionally
        } else {
            (false, 0)
        };

        // Determine speculative decoding
        let enable_speculative = config.enable_speculative
            && snapshot.ttft_avg_ms < config.latency_target_ms * 0.5
            && self.draft_model.read().is_some();

        // Token budget allocation for pending requests
        let pending = self.pending_requests.read().clone();
        let token_budgets = self.allocate_token_budget(&pending);

        // Quantization recommendation
        let quantization_recommendation = if self.memory_pressure() > 0.8 {
            Some("Q4".to_string())
        } else if self.memory_pressure() > 0.6 {
            Some("Q8".to_string())
        } else {
            None
        };

        // Estimate outcomes
        let batch_ratio =
            batch_size as f32 / self.current_batch_size.load(Ordering::Relaxed).max(1) as f32;
        let estimated_latency = snapshot.ttft_avg_ms * batch_ratio.sqrt();
        let estimated_tps = snapshot.tps_avg * batch_ratio;

        // Calculate confidence based on data quality
        let sample_count = latencies.len();
        let confidence = if sample_count < 10 {
            0.3
        } else if sample_count < 50 {
            0.6
        } else {
            0.9
        };

        // Generate reason
        let reason = format!(
            "Batch: {} (latency={:.1}ms, target={:.1}ms), KV pressure: {:.1}%, Memory: {:.1}%",
            batch_size,
            snapshot.ttft_avg_ms,
            config.latency_target_ms,
            kv_pressure * 100.0,
            self.memory_pressure() * 100.0
        );

        OptimizationDecision {
            batch_size,
            should_evict_kv_cache: should_evict,
            evict_count,
            enable_speculative,
            token_budgets,
            quantization_recommendation,
            estimated_latency_ms: estimated_latency,
            estimated_tps,
            confidence,
            reason,
        }
    }

    /// Add a pending request
    pub fn add_request(&self, request: Request) {
        self.pending_requests.write().push(request);
    }

    /// Remove a completed request
    pub fn remove_request(&self, request_id: &str) {
        self.pending_requests.write().retain(|r| r.id != request_id);
    }

    /// Get pending request count
    pub fn pending_request_count(&self) -> usize {
        self.pending_requests.read().len()
    }

    /// Get current batch size
    pub fn current_batch_size(&self) -> usize {
        self.current_batch_size.load(Ordering::Relaxed)
    }

    /// Update configuration
    pub fn update_config(&self, config: RealtimeConfig) {
        *self.config.write() = config;
    }

    /// Get current configuration
    pub fn config(&self) -> RealtimeConfig {
        self.config.read().clone()
    }
}

impl RealtimeOptimizer {
    /// Check if speculative decoding should be used for these generation parameters
    ///
    /// Returns true when:
    /// - Temperature is low (< 0.5) - deterministic generation benefits most
    /// - Greedy decoding (top_k = 1)
    /// - Speculative decoding is enabled in config
    pub fn should_use_speculative(&self, params: &crate::backends::GenerateParams) -> bool {
        let config = self.config.read();
        if !config.enable_speculative {
            return false;
        }

        // Speculative decoding is most effective for:
        // 1. Low temperature (more deterministic)
        // 2. Greedy decoding
        // 3. When not using high top-p sampling
        params.temperature < 0.5 || params.top_k == 1
    }

    /// Get recommended speculative decoding configuration based on current metrics
    pub fn get_speculative_config(&self) -> SpeculativeConfig {
        let config = self.config.read();
        let avg_latency = self.average_latency();
        let memory_pressure = self.memory_pressure();

        // Adjust speculative config based on system state
        let mut spec_config = config.speculative.clone();

        // Reduce lookahead under memory pressure
        if memory_pressure > 0.8 {
            spec_config.num_speculative_tokens = (spec_config.num_speculative_tokens / 2).max(2);
        }

        // Increase acceptance threshold when latency is high
        if avg_latency > config.latency_target_ms {
            spec_config.acceptance_threshold = (spec_config.acceptance_threshold + 0.1).min(0.95);
        }

        spec_config
    }
}

impl Default for RealtimeOptimizer {
    fn default() -> Self {
        Self::new(RealtimeConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_realtime_config_default() {
        let config = RealtimeConfig::default();
        assert!((config.latency_target_ms - 100.0).abs() < 0.01);
        assert!((config.throughput_target_tps - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_optimizer_creation() {
        let config = RealtimeConfig {
            min_batch_size: 1,
            max_batch_size: 32,
            batch_strategy: BatchSizeStrategy::Adaptive,
            ..Default::default()
        };

        let optimizer = RealtimeOptimizer::new(config);
        assert!(optimizer.current_batch_size() >= 1);
        assert!(optimizer.current_batch_size() <= 32);
    }

    #[test]
    fn test_batch_size_optimization() {
        let config = RealtimeConfig {
            latency_target_ms: 100.0,
            min_batch_size: 1,
            max_batch_size: 16,
            batch_strategy: BatchSizeStrategy::Adaptive,
            ..Default::default()
        };

        let optimizer = RealtimeOptimizer::new(config);

        // High latency should reduce batch size
        let high_latencies = vec![150.0, 160.0, 140.0];
        let batch = optimizer.optimize_batch_size(&high_latencies);
        assert!(batch <= 8, "High latency should reduce batch size");

        // Low latency should increase batch size
        let low_latencies = vec![30.0, 35.0, 25.0];
        let batch = optimizer.optimize_batch_size(&low_latencies);
        assert!(batch >= 4, "Low latency should allow larger batch size");
    }

    #[test]
    fn test_kv_cache_pressure() {
        let config = RealtimeConfig {
            kv_cache_pressure_threshold: 0.8,
            ..Default::default()
        };

        let optimizer = RealtimeOptimizer::new(config);

        optimizer.update_kv_cache_pressure(0.5);
        assert!(!optimizer.should_evict_kv_cache());

        optimizer.update_kv_cache_pressure(0.9);
        assert!(optimizer.should_evict_kv_cache());
    }

    #[test]
    fn test_token_budget_allocation() {
        let optimizer = RealtimeOptimizer::new(RealtimeConfig::default());

        let requests = vec![
            Request {
                id: "req1".to_string(),
                input_tokens: 100,
                max_output_tokens: 200,
                priority: 0.9,
                arrival_time: Instant::now(),
                deadline: None,
            },
            Request {
                id: "req2".to_string(),
                input_tokens: 50,
                max_output_tokens: 100,
                priority: 0.5,
                arrival_time: Instant::now(),
                deadline: Some(Duration::from_secs(1)),
            },
        ];

        let allocations = optimizer.allocate_token_budget(&requests);
        assert_eq!(allocations.len(), 2);

        // Higher priority request should get more resources
        let high_priority = allocations.iter().find(|a| a.request_id == "req1").unwrap();
        assert!(high_priority.batch_slot.is_some() || high_priority.max_tokens >= 100);
    }

    #[test]
    fn test_speculative_decoding() {
        let optimizer = RealtimeOptimizer::new(RealtimeConfig {
            enable_speculative: true,
            ..Default::default()
        });

        assert!(!optimizer.is_speculative_active());

        optimizer.enable_speculative_decoding("draft-model-1");
        assert!(optimizer.is_speculative_active());
        assert_eq!(optimizer.draft_model(), Some("draft-model-1".to_string()));

        optimizer.disable_speculative_decoding();
        assert!(!optimizer.is_speculative_active());
    }

    #[test]
    fn test_optimization_decision() {
        let optimizer = RealtimeOptimizer::new(RealtimeConfig::default());
        let metrics = InferenceMetrics::new();

        // Record some metrics
        for i in 1..=10 {
            metrics.record_ttft(i as f32 * 10.0);
            optimizer.record_latency(i as f32 * 10.0);
            optimizer.record_throughput(50.0 + i as f32);
        }

        let decision = optimizer.optimize(&metrics);
        assert!(decision.batch_size >= 1);
        assert!(decision.confidence > 0.0);
    }

    #[test]
    fn test_memory_pressure() {
        let config = RealtimeConfig {
            max_memory_bytes: 1024 * 1024 * 1024, // 1GB
            ..Default::default()
        };

        let optimizer = RealtimeOptimizer::new(config);

        optimizer.update_memory_usage(512 * 1024 * 1024); // 512MB
        assert!((optimizer.memory_pressure() - 0.5).abs() < 0.01);

        optimizer.update_memory_usage(800 * 1024 * 1024); // 800MB
        assert!((optimizer.memory_pressure() - 0.78).abs() < 0.02);
    }

    #[test]
    fn test_batch_strategies() {
        let strategies = vec![
            BatchSizeStrategy::Fixed,
            BatchSizeStrategy::Adaptive,
            BatchSizeStrategy::Aggressive,
            BatchSizeStrategy::Conservative,
            BatchSizeStrategy::Hybrid,
        ];

        for strategy in strategies {
            let config = RealtimeConfig {
                batch_strategy: strategy,
                min_batch_size: 1,
                max_batch_size: 16,
                ..Default::default()
            };

            let optimizer = RealtimeOptimizer::new(config);
            let latencies = vec![50.0, 55.0, 45.0];
            let batch = optimizer.optimize_batch_size(&latencies);

            assert!(
                batch >= 1 && batch <= 16,
                "Strategy {:?} produced invalid batch size",
                strategy
            );
        }
    }
}
