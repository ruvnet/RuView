//! Expert Hot-Set Cache and MoE Batch Scheduler
//!
//! This module implements memory bandwidth optimizations for MoE inference:
//!
//! - **ExpertCache**: Tracks which experts are "hot" (recently/frequently accessed)
//!   and manages eviction to keep working-set size bounded. With top-K=2 active
//!   experts per token but 8 total experts per layer, naive traversal thrashes
//!   L2/L3 cache. The hot-set cache keeps the 4 most relevant experts warm.
//!
//! - **MoeBatchScheduler**: Reorders expert execution across a token batch so that
//!   all tokens routed to the same expert are processed contiguously. This converts
//!   random expert access into sequential scans, maximizing cache-line reuse.
//!
//! - **Prefetcher trait**: Abstraction for platform-specific memory prefetch
//!   intrinsics (x86 `_mm_prefetch`, aarch64 `__pld`). Currently ships with a
//!   no-op implementation; architecture-specific backends can be added without
//!   changing call sites.
//!
//! ## Memory Layout Context
//!
//! Each expert's ternary weights occupy roughly `ceil(rows * cols / 4)` packed
//! bytes plus `ceil(rows * cols / block_size) * 4` scale bytes. For a 30B MoE
//! model with `intermediate_size=11008` and `hidden_size=4096`:
//!
//! ```text
//! gate_proj: 11008 * 4096 * 2 bits / 8 = ~11.3 MB packed
//! up_proj:   11008 * 4096 * 2 bits / 8 = ~11.3 MB packed
//! down_proj: 4096 * 11008 * 2 bits / 8 = ~11.3 MB packed
//! Total per expert: ~33.9 MB packed + scales
//! ```
//!
//! With 8 experts that is ~271 MB per layer. Keeping only 4 hot halves the
//! cache pressure while covering the top-2 active plus 2 likely next picks.

use std::collections::HashMap;

// ============================================================================
// Configuration
// ============================================================================

/// Eviction policy for the expert hot-set cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    /// Least Recently Used: evict the expert with the oldest access timestamp.
    Lru,
    /// Least Frequently Used: evict the expert with the lowest total access count.
    Lfu,
    /// Adaptive: use LFU when frequency distribution is skewed (top expert has
    /// 3x the accesses of the least-used), otherwise fall back to LRU. This
    /// handles both steady-state routing (where certain experts dominate) and
    /// transient shifts (where recency matters more).
    Adaptive,
}

/// Configuration for the expert hot-set cache.
#[derive(Debug, Clone)]
pub struct ExpertCacheConfig {
    /// Maximum number of experts kept in the hot set.
    ///
    /// Default is 4: with top-K=2 active per token, keeping 4 warm provides
    /// temporal locality for the next 1-2 tokens without over-provisioning.
    pub max_hot_experts: usize,

    /// Router weight threshold for speculative prefetch.
    ///
    /// If an expert's softmax weight exceeds this threshold but the expert is
    /// not in the current top-K selection, it is a prefetch candidate. This
    /// catches experts that are "almost selected" and likely to be needed soon.
    ///
    /// Default is 0.1 (10% softmax probability).
    pub prefetch_threshold: f32,

    /// Eviction policy when the hot set is full and a new expert must be admitted.
    pub eviction_policy: EvictionPolicy,
}

impl Default for ExpertCacheConfig {
    fn default() -> Self {
        Self {
            max_hot_experts: 4,
            prefetch_threshold: 0.1,
            eviction_policy: EvictionPolicy::Lru,
        }
    }
}

// ============================================================================
// Statistics
// ============================================================================

/// Runtime statistics for the expert cache.
///
/// Tracks hits, misses, evictions, and prefetch effectiveness to enable
/// tuning of `max_hot_experts` and `prefetch_threshold` parameters.
#[derive(Debug, Clone, Default)]
pub struct ExpertCacheStats {
    /// Number of accesses where the expert was already in the hot set.
    pub hits: usize,
    /// Number of accesses where the expert was not in the hot set.
    pub misses: usize,
    /// Number of experts evicted from the hot set.
    pub evictions: usize,
    /// Number of accesses that hit an expert that was speculatively prefetched.
    pub prefetch_hits: usize,
}

impl ExpertCacheStats {
    /// Compute the cache hit rate as a fraction in [0.0, 1.0].
    ///
    /// Returns 0.0 if no accesses have been recorded.
    pub fn hit_rate(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f32 / total as f32
    }
}

// ============================================================================
// ExpertCache
// ============================================================================

/// Hot-set cache for MoE expert weights.
///
/// Maintains a bounded set of "hot" expert IDs whose weight tensors should be
/// kept in CPU cache (L2/L3). The cache does not own the weight data itself;
/// it tracks which expert IDs are hot so that the inference loop can skip
/// unnecessary memory traffic for cold experts.
///
/// # Usage
///
/// ```rust,ignore
/// use ruvllm::bitnet::expert_cache::{ExpertCache, ExpertCacheConfig};
///
/// let config = ExpertCacheConfig::default();
/// let mut cache = ExpertCache::new(8, config);
///
/// // Record that experts 2 and 5 were selected by the router
/// let hit_2 = cache.access(2); // false (cold miss on first access)
/// let hit_5 = cache.access(5); // false
///
/// // Next token: expert 2 selected again
/// let hit_2 = cache.access(2); // true (hot hit)
/// ```
pub struct ExpertCache {
    /// Total number of experts in the model (per layer).
    num_experts: usize,
    /// (expert_id, last_access_timestamp) for each expert currently in the hot set.
    hot_set: Vec<(usize, u64)>,
    /// Per-expert total access count, indexed by expert_id. Used for LFU eviction.
    frequency: Vec<usize>,
    /// Set of expert IDs that were admitted via speculative prefetch (not yet
    /// accessed by the router). Used to track prefetch hit effectiveness.
    prefetched: Vec<bool>,
    /// Cache configuration.
    config: ExpertCacheConfig,
    /// Runtime statistics.
    stats: ExpertCacheStats,
    /// Monotonically increasing counter used as a logical timestamp for LRU ordering.
    access_counter: u64,
}

impl ExpertCache {
    /// Create a new expert cache.
    ///
    /// # Arguments
    ///
    /// * `num_experts` - Total number of experts per layer in the model.
    /// * `config` - Cache configuration (hot-set size, thresholds, policy).
    pub fn new(num_experts: usize, config: ExpertCacheConfig) -> Self {
        Self {
            num_experts,
            hot_set: Vec::with_capacity(config.max_hot_experts),
            frequency: vec![0; num_experts],
            prefetched: vec![false; num_experts],
            config,
            stats: ExpertCacheStats::default(),
            access_counter: 0,
        }
    }

    /// Record an access to the given expert.
    ///
    /// If the expert is already in the hot set this is a cache hit: its
    /// timestamp is refreshed and its frequency count is incremented.
    ///
    /// If the expert is cold (not in the hot set) this is a cache miss: the
    /// expert is admitted (potentially evicting another), and the miss is
    /// recorded in stats.
    ///
    /// # Returns
    ///
    /// `true` if the expert was already hot (cache hit), `false` otherwise.
    pub fn access(&mut self, expert_id: usize) -> bool {
        self.access_counter += 1;
        let timestamp = self.access_counter;

        // Always bump frequency
        if expert_id < self.num_experts {
            self.frequency[expert_id] += 1;
        }

        // Check if expert is already in the hot set
        if let Some(pos) = self.hot_set.iter().position(|&(id, _)| id == expert_id) {
            // Hit: refresh timestamp
            self.hot_set[pos].1 = timestamp;
            self.stats.hits += 1;

            // Track prefetch effectiveness
            if expert_id < self.prefetched.len() && self.prefetched[expert_id] {
                self.stats.prefetch_hits += 1;
                self.prefetched[expert_id] = false;
            }

            return true;
        }

        // Miss: admit the expert
        self.stats.misses += 1;
        self.admit(expert_id);
        false
    }

    /// Check whether a not-yet-selected expert should be speculatively prefetched.
    ///
    /// Returns `true` if:
    /// 1. The expert is not already in the hot set, AND
    /// 2. Its router weight exceeds the configured `prefetch_threshold`.
    ///
    /// The caller is responsible for actually performing the prefetch (e.g.,
    /// issuing prefetch instructions or touching the memory).
    pub fn should_prefetch(&self, expert_id: usize, router_weight: f32) -> bool {
        if router_weight <= self.config.prefetch_threshold {
            return false;
        }
        !self.is_hot(expert_id)
    }

    /// Suggest which expert to evict from the hot set.
    ///
    /// Returns `None` if the hot set is not full. Otherwise returns the
    /// expert_id that should be evicted according to the configured policy.
    pub fn suggest_eviction(&self) -> Option<usize> {
        if self.hot_set.len() < self.config.max_hot_experts {
            return None;
        }

        match self.config.eviction_policy {
            EvictionPolicy::Lru => self.suggest_lru_eviction(),
            EvictionPolicy::Lfu => self.suggest_lfu_eviction(),
            EvictionPolicy::Adaptive => self.suggest_adaptive_eviction(),
        }
    }

    /// Evict a specific expert from the hot set.
    ///
    /// No-op if the expert is not currently hot.
    pub fn evict(&mut self, expert_id: usize) {
        if let Some(pos) = self.hot_set.iter().position(|&(id, _)| id == expert_id) {
            self.hot_set.swap_remove(pos);
            self.stats.evictions += 1;
        }
    }

    /// Admit an expert into the hot set.
    ///
    /// If the hot set is already at capacity, evicts one expert first according
    /// to the configured eviction policy. If the expert is already hot, this
    /// is a no-op.
    pub fn admit(&mut self, expert_id: usize) {
        // Already hot: nothing to do
        if self.is_hot(expert_id) {
            return;
        }

        // Evict if at capacity
        if self.hot_set.len() >= self.config.max_hot_experts {
            if let Some(victim) = self.suggest_eviction() {
                self.evict(victim);
            }
        }

        let timestamp = self.access_counter;
        self.hot_set.push((expert_id, timestamp));
    }

    /// Admit an expert via speculative prefetch.
    ///
    /// Like `admit`, but marks the expert as prefetched so that a subsequent
    /// `access` hit can be attributed to the prefetch in stats.
    pub fn prefetch_admit(&mut self, expert_id: usize) {
        if expert_id < self.prefetched.len() {
            self.prefetched[expert_id] = true;
        }
        self.admit(expert_id);
    }

    /// Check whether the given expert is currently in the hot set.
    pub fn is_hot(&self, expert_id: usize) -> bool {
        self.hot_set.iter().any(|&(id, _)| id == expert_id)
    }

    /// Return a reference to the current cache statistics.
    pub fn stats(&self) -> &ExpertCacheStats {
        &self.stats
    }

    /// Reset all statistics counters to zero.
    pub fn reset_stats(&mut self) {
        self.stats = ExpertCacheStats::default();
    }

    /// Return the current number of experts in the hot set.
    pub fn hot_count(&self) -> usize {
        self.hot_set.len()
    }

    /// Return the configured maximum hot-set size.
    pub fn max_hot(&self) -> usize {
        self.config.max_hot_experts
    }

    // --- Private helpers ---

    /// LRU eviction: pick the expert with the smallest (oldest) timestamp.
    fn suggest_lru_eviction(&self) -> Option<usize> {
        self.hot_set
            .iter()
            .min_by_key(|&&(_, ts)| ts)
            .map(|&(id, _)| id)
    }

    /// LFU eviction: pick the hot expert with the lowest total access frequency.
    fn suggest_lfu_eviction(&self) -> Option<usize> {
        self.hot_set
            .iter()
            .min_by_key(|&&(id, _)| self.frequency.get(id).copied().unwrap_or(0))
            .map(|&(id, _)| id)
    }

    /// Adaptive eviction: use LFU when frequency distribution is skewed,
    /// otherwise fall back to LRU.
    fn suggest_adaptive_eviction(&self) -> Option<usize> {
        if self.hot_set.is_empty() {
            return None;
        }

        let freqs: Vec<usize> = self
            .hot_set
            .iter()
            .map(|&(id, _)| self.frequency.get(id).copied().unwrap_or(0))
            .collect();

        let max_freq = freqs.iter().copied().max().unwrap_or(0);
        let min_freq = freqs.iter().copied().min().unwrap_or(0);

        // If the most-accessed expert has >= 3x the accesses of the least-accessed,
        // the distribution is skewed enough that frequency is a better signal.
        if min_freq > 0 && max_freq >= 3 * min_freq {
            self.suggest_lfu_eviction()
        } else {
            self.suggest_lru_eviction()
        }
    }
}

// ============================================================================
// MoE Batch Scheduler
// ============================================================================

/// A batch of tokens routed to the same expert, produced by `MoeBatchScheduler`.
#[derive(Debug, Clone)]
pub struct ExpertBatch {
    /// The expert ID that all tokens in this batch are routed to.
    pub expert_id: usize,
    /// Indices into the original token batch identifying which tokens are included.
    pub token_indices: Vec<usize>,
    /// Per-token router weights for this expert (same order as `token_indices`).
    pub weights: Vec<f32>,
}

/// Reorders expert execution across a token batch to maximize cache reuse.
///
/// Without batching, each token processes its top-K experts independently:
/// ```text
/// Token 0: Expert 2, Expert 5
/// Token 1: Expert 5, Expert 3
/// Token 2: Expert 2, Expert 7
/// ```
///
/// This causes expert weights to be loaded, evicted, and reloaded. The batch
/// scheduler groups tokens by expert:
/// ```text
/// Expert 2: Token 0 (w=0.6), Token 2 (w=0.7)
/// Expert 3: Token 1 (w=0.3)
/// Expert 5: Token 0 (w=0.4), Token 1 (w=0.7)
/// Expert 7: Token 2 (w=0.3)
/// ```
///
/// Now each expert's weights are loaded once and applied to all relevant tokens
/// before moving on.
pub struct MoeBatchScheduler;

impl MoeBatchScheduler {
    /// Schedule a batch of routing decisions into expert-grouped batches.
    ///
    /// # Arguments
    ///
    /// * `routing_decisions` - For each token in the batch, a tuple of
    ///   `(token_index, Vec<(expert_id, router_weight)>)` describing which
    ///   experts were selected and their normalized weights.
    ///
    /// # Returns
    ///
    /// A vector of `ExpertBatch` structs, one per unique expert referenced in
    /// the routing decisions, sorted by expert_id for deterministic ordering.
    pub fn schedule(routing_decisions: &[(usize, Vec<(usize, f32)>)]) -> Vec<ExpertBatch> {
        // Collect all (expert_id -> Vec<(token_idx, weight)>)
        let mut expert_map: HashMap<usize, Vec<(usize, f32)>> = HashMap::new();

        for &(token_idx, ref experts) in routing_decisions {
            for &(expert_id, weight) in experts {
                expert_map
                    .entry(expert_id)
                    .or_default()
                    .push((token_idx, weight));
            }
        }

        // Build sorted batches
        let mut batches: Vec<ExpertBatch> = expert_map
            .into_iter()
            .map(|(expert_id, entries)| {
                let (token_indices, weights): (Vec<usize>, Vec<f32>) = entries.into_iter().unzip();
                ExpertBatch {
                    expert_id,
                    token_indices,
                    weights,
                }
            })
            .collect();

        // Sort by expert_id for deterministic execution order
        batches.sort_by_key(|b| b.expert_id);
        batches
    }
}

// ============================================================================
// Prefetcher Trait
// ============================================================================

/// Abstraction for platform-specific memory prefetch instructions.
///
/// Implementations can issue hardware prefetch hints (e.g., x86 `_mm_prefetch`
/// with `_MM_HINT_T0`, aarch64 `__pld`) to pull expert weight data into cache
/// ahead of the GEMV kernel touching it.
///
/// The trait is object-safe to allow runtime dispatch between platform backends.
pub trait Prefetcher: Send + Sync {
    /// Issue a prefetch hint for a region of memory.
    ///
    /// # Arguments
    ///
    /// * `data` - The backing byte slice (e.g., `TernaryTensor::packed_data`).
    /// * `offset` - Byte offset into `data` where the prefetch region starts.
    /// * `len` - Number of bytes to prefetch. Implementations may round up to
    ///   cache-line granularity.
    ///
    /// # Safety
    ///
    /// This is a hint only. Implementations must not cause faults if `offset + len`
    /// exceeds `data.len()`.
    fn prefetch(&self, data: &[u8], offset: usize, len: usize);
}

/// No-op prefetcher used when platform-specific intrinsics are not available.
///
/// All calls are silent no-ops. This is the default prefetcher for portable builds.
pub struct NullPrefetcher;

impl Prefetcher for NullPrefetcher {
    #[inline(always)]
    fn prefetch(&self, _data: &[u8], _offset: usize, _len: usize) {
        // Intentionally empty. On x86_64, this would be:
        //   unsafe { std::arch::x86_64::_mm_prefetch(ptr, _MM_HINT_T0); }
        // On aarch64:
        //   unsafe { std::arch::aarch64::__pld(ptr); }
    }
}

// ============================================================================
// Memory Layout Helpers
// ============================================================================

/// Cache line size in bytes (standard for x86_64 and most aarch64 cores).
const CACHE_LINE_BYTES: usize = 64;

/// Round a pointer-sized address up to the nearest 64-byte cache-line boundary.
///
/// This is useful for ensuring that expert weight buffers start on cache-line
/// boundaries to avoid false sharing and partial-line fetches.
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::bitnet::expert_cache::align_to_cache_line;
///
/// assert_eq!(align_to_cache_line(0), 0);
/// assert_eq!(align_to_cache_line(1), 64);
/// assert_eq!(align_to_cache_line(64), 64);
/// assert_eq!(align_to_cache_line(65), 128);
/// ```
#[inline]
pub fn align_to_cache_line(ptr: usize) -> usize {
    (ptr + CACHE_LINE_BYTES - 1) & !(CACHE_LINE_BYTES - 1)
}

/// Compute the memory footprint of a single expert's packed ternary data.
///
/// An expert projection (e.g., gate_proj) with shape `(rows, cols)` and the
/// given `block_size` occupies:
/// - Packed data: `ceil(rows * cols / 4)` bytes (2 bits per weight, 4 per byte)
/// - Scales: `ceil(rows * cols / block_size) * 4` bytes (one FP32 per block)
///
/// The returned value is the sum, **not** cache-line aligned.
///
/// # Arguments
///
/// * `rows` - Number of output features (e.g., intermediate_size).
/// * `cols` - Number of input features (e.g., hidden_size).
/// * `block_size` - Elements per quantization block (typically 256).
#[inline]
pub fn expert_memory_footprint(rows: usize, cols: usize, block_size: usize) -> usize {
    let total_elements = rows * cols;
    let packed_bytes = (total_elements + 3) / 4;
    let num_blocks = (total_elements + block_size - 1) / block_size;
    let scale_bytes = num_blocks * 4; // FP32 = 4 bytes
    packed_bytes + scale_bytes
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // Helper: create a default cache with given expert count and max hot
    // ---------------------------------------------------------------

    fn make_cache(num_experts: usize, max_hot: usize, policy: EvictionPolicy) -> ExpertCache {
        let config = ExpertCacheConfig {
            max_hot_experts: max_hot,
            prefetch_threshold: 0.1,
            eviction_policy: policy,
        };
        ExpertCache::new(num_experts, config)
    }

    // ---------------------------------------------------------------
    // 1. LRU eviction order is correct
    // ---------------------------------------------------------------

    #[test]
    fn test_lru_eviction_order() {
        let mut cache = make_cache(8, 3, EvictionPolicy::Lru);

        // Fill the hot set: 0, 1, 2
        cache.access(0);
        cache.access(1);
        cache.access(2);

        // All three should be hot
        assert!(cache.is_hot(0));
        assert!(cache.is_hot(1));
        assert!(cache.is_hot(2));

        // Access expert 0 again to refresh its timestamp
        cache.access(0);

        // Now admit expert 3 -> should evict expert 1 (oldest unrefresfreshed)
        cache.access(3);

        assert!(
            cache.is_hot(0),
            "Expert 0 was refreshed, should still be hot"
        );
        assert!(!cache.is_hot(1), "Expert 1 should have been evicted (LRU)");
        assert!(
            cache.is_hot(2),
            "Expert 2 was accessed after 1, should survive"
        );
        assert!(cache.is_hot(3), "Expert 3 was just admitted");
    }

    // ---------------------------------------------------------------
    // 2. LFU eviction order is correct
    // ---------------------------------------------------------------

    #[test]
    fn test_lfu_eviction_order() {
        let mut cache = make_cache(8, 3, EvictionPolicy::Lfu);

        // Expert 0: accessed 3 times
        cache.access(0);
        cache.access(0);
        cache.access(0);

        // Expert 1: accessed 1 time
        cache.access(1);

        // Expert 2: accessed 2 times
        cache.access(2);
        cache.access(2);

        // Hot set: {0, 1, 2}, frequencies: 0->3, 1->1, 2->2
        assert!(cache.is_hot(0));
        assert!(cache.is_hot(1));
        assert!(cache.is_hot(2));

        // Admit expert 3 -> should evict expert 1 (frequency=1, lowest)
        cache.access(3);

        assert!(cache.is_hot(0), "Expert 0 (freq=3) should survive");
        assert!(
            !cache.is_hot(1),
            "Expert 1 (freq=1) should be evicted by LFU"
        );
        assert!(cache.is_hot(2), "Expert 2 (freq=2) should survive");
        assert!(cache.is_hot(3), "Expert 3 was just admitted");
    }

    // ---------------------------------------------------------------
    // 3. Hot set respects max_hot_experts limit
    // ---------------------------------------------------------------

    #[test]
    fn test_hot_set_respects_limit() {
        let mut cache = make_cache(16, 4, EvictionPolicy::Lru);

        // Access more experts than max_hot
        for i in 0..10 {
            cache.access(i);
        }

        // Should never exceed 4 hot experts
        assert!(
            cache.hot_count() <= 4,
            "Hot count {} exceeds max of 4",
            cache.hot_count()
        );
        assert_eq!(cache.hot_count(), 4);
    }

    // ---------------------------------------------------------------
    // 4. Access returns hit=true for hot experts
    // ---------------------------------------------------------------

    #[test]
    fn test_access_returns_hit_for_hot() {
        let mut cache = make_cache(8, 4, EvictionPolicy::Lru);

        // First access is always a miss
        assert!(!cache.access(3));

        // Second access should be a hit
        assert!(cache.access(3));
        assert!(cache.access(3));
    }

    // ---------------------------------------------------------------
    // 5. Access returns hit=false for cold experts
    // ---------------------------------------------------------------

    #[test]
    fn test_access_returns_miss_for_cold() {
        let mut cache = make_cache(8, 2, EvictionPolicy::Lru);

        // Fill: 0, 1
        cache.access(0);
        cache.access(1);

        // Access 2 -> evicts 0, returns false (miss)
        assert!(!cache.access(2));
        // Access 3 -> evicts 1, returns false (miss)
        assert!(!cache.access(3));

        // Now 0 and 1 are cold, accessing them is a miss
        assert!(!cache.access(0));
    }

    // ---------------------------------------------------------------
    // 6. Hit rate calculation is correct
    // ---------------------------------------------------------------

    #[test]
    fn test_hit_rate_calculation() {
        let mut cache = make_cache(8, 4, EvictionPolicy::Lru);

        // No accesses -> 0.0
        assert_eq!(cache.stats().hit_rate(), 0.0);

        // 1 miss (first access to expert 0)
        cache.access(0);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hit_rate(), 0.0);

        // 1 hit (second access to expert 0)
        cache.access(0);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);
        assert!((cache.stats().hit_rate() - 0.5).abs() < 1e-6);

        // 2 more hits
        cache.access(0);
        cache.access(0);
        // Total: 3 hits, 1 miss => 3/4 = 0.75
        assert!((cache.stats().hit_rate() - 0.75).abs() < 1e-6);
    }

    // ---------------------------------------------------------------
    // 7. Prefetch threshold works
    // ---------------------------------------------------------------

    #[test]
    fn test_prefetch_threshold() {
        let config = ExpertCacheConfig {
            max_hot_experts: 4,
            prefetch_threshold: 0.15,
            eviction_policy: EvictionPolicy::Lru,
        };
        let mut cache = ExpertCache::new(8, config);

        // Expert 0 is not hot -> should prefetch if weight > 0.15
        assert!(cache.should_prefetch(0, 0.2));
        assert!(cache.should_prefetch(0, 0.16));
        assert!(!cache.should_prefetch(0, 0.15)); // at threshold, not above
        assert!(!cache.should_prefetch(0, 0.1));
        assert!(!cache.should_prefetch(0, 0.0));

        // Make expert 0 hot -> should NOT prefetch (already hot)
        cache.access(0);
        assert!(!cache.should_prefetch(0, 0.5));
    }

    // ---------------------------------------------------------------
    // 8. Batch scheduler groups tokens by expert
    // ---------------------------------------------------------------

    #[test]
    fn test_batch_scheduler_groups_by_expert() {
        let routing = vec![
            (0, vec![(2, 0.6), (5, 0.4)]),
            (1, vec![(5, 0.7), (3, 0.3)]),
            (2, vec![(2, 0.7), (7, 0.3)]),
        ];

        let batches = MoeBatchScheduler::schedule(&routing);

        // Should have 4 unique experts: 2, 3, 5, 7
        assert_eq!(batches.len(), 4);

        // Batches should be sorted by expert_id
        let expert_ids: Vec<usize> = batches.iter().map(|b| b.expert_id).collect();
        assert_eq!(expert_ids, vec![2, 3, 5, 7]);

        // Expert 2: tokens 0 and 2
        let batch_2 = &batches[0];
        assert_eq!(batch_2.expert_id, 2);
        assert_eq!(batch_2.token_indices, vec![0, 2]);
        assert_eq!(batch_2.weights, vec![0.6, 0.7]);

        // Expert 3: token 1 only
        let batch_3 = &batches[1];
        assert_eq!(batch_3.expert_id, 3);
        assert_eq!(batch_3.token_indices, vec![1]);
        assert_eq!(batch_3.weights, vec![0.3]);

        // Expert 5: tokens 0 and 1
        let batch_5 = &batches[2];
        assert_eq!(batch_5.expert_id, 5);
        assert_eq!(batch_5.token_indices, vec![0, 1]);
        assert_eq!(batch_5.weights, vec![0.4, 0.7]);

        // Expert 7: token 2 only
        let batch_7 = &batches[3];
        assert_eq!(batch_7.expert_id, 7);
        assert_eq!(batch_7.token_indices, vec![2]);
        assert_eq!(batch_7.weights, vec![0.3]);
    }

    // ---------------------------------------------------------------
    // 9. Batch scheduler handles single-token case
    // ---------------------------------------------------------------

    #[test]
    fn test_batch_scheduler_single_token() {
        let routing = vec![(0, vec![(4, 0.65), (1, 0.35)])];

        let batches = MoeBatchScheduler::schedule(&routing);

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].expert_id, 1);
        assert_eq!(batches[0].token_indices, vec![0]);
        assert_eq!(batches[0].weights, vec![0.35]);

        assert_eq!(batches[1].expert_id, 4);
        assert_eq!(batches[1].token_indices, vec![0]);
        assert_eq!(batches[1].weights, vec![0.65]);
    }

    // ---------------------------------------------------------------
    // 10. Cache stats accumulate correctly
    // ---------------------------------------------------------------

    #[test]
    fn test_cache_stats_accumulate() {
        let mut cache = make_cache(8, 2, EvictionPolicy::Lru);

        // Misses: 0, 1
        cache.access(0); // miss
        cache.access(1); // miss
        assert_eq!(cache.stats().misses, 2);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().evictions, 0);

        // Hit: 0
        cache.access(0); // hit
        assert_eq!(cache.stats().hits, 1);

        // Miss + eviction: 2 evicts 1 (LRU)
        cache.access(2); // miss, evicts 1
        assert_eq!(cache.stats().misses, 3);
        assert_eq!(cache.stats().evictions, 1);

        // Hit: 0 (still hot)
        cache.access(0); // hit
        assert_eq!(cache.stats().hits, 2);

        // Reset
        cache.reset_stats();
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 0);
        assert_eq!(cache.stats().evictions, 0);
        assert_eq!(cache.stats().prefetch_hits, 0);
    }

    // ---------------------------------------------------------------
    // 11. Eviction happens when hot set is full
    // ---------------------------------------------------------------

    #[test]
    fn test_eviction_when_full() {
        let mut cache = make_cache(8, 3, EvictionPolicy::Lru);

        cache.access(0);
        cache.access(1);
        cache.access(2);
        assert_eq!(cache.hot_count(), 3);
        assert_eq!(cache.stats().evictions, 0);

        // Admitting a 4th expert must trigger an eviction
        cache.access(3);
        assert_eq!(cache.hot_count(), 3);
        assert_eq!(cache.stats().evictions, 1);
        assert!(!cache.is_hot(0), "Expert 0 (oldest) should be evicted");
        assert!(cache.is_hot(3));
    }

    // ---------------------------------------------------------------
    // 12. Memory footprint calculation is correct
    // ---------------------------------------------------------------

    #[test]
    fn test_memory_footprint_calculation() {
        // 256 x 256 tensor, block_size = 256
        // total = 65536 elements
        // packed = ceil(65536/4) = 16384 bytes
        // blocks = ceil(65536/256) = 256
        // scales = 256 * 4 = 1024 bytes
        // total = 16384 + 1024 = 17408
        let footprint = expert_memory_footprint(256, 256, 256);
        assert_eq!(footprint, 17408);

        // 1 x 4 tensor, block_size = 256
        // total = 4 elements
        // packed = ceil(4/4) = 1 byte
        // blocks = ceil(4/256) = 1
        // scales = 1 * 4 = 4 bytes
        // total = 5
        let footprint_small = expert_memory_footprint(1, 4, 256);
        assert_eq!(footprint_small, 5);

        // 11008 x 4096 tensor (realistic gate_proj), block_size = 256
        let rows = 11008usize;
        let cols = 4096usize;
        let total = rows * cols; // 45088768
        let packed = (total + 3) / 4; // 11272192
        let blocks = (total + 255) / 256; // 176128
        let scales_bytes = blocks * 4; // 704512
        let expected = packed + scales_bytes; // 11976704
        assert_eq!(expert_memory_footprint(rows, cols, 256), expected);
    }

    // ---------------------------------------------------------------
    // 13. align_to_cache_line works correctly
    // ---------------------------------------------------------------

    #[test]
    fn test_align_to_cache_line() {
        assert_eq!(align_to_cache_line(0), 0);
        assert_eq!(align_to_cache_line(1), 64);
        assert_eq!(align_to_cache_line(63), 64);
        assert_eq!(align_to_cache_line(64), 64);
        assert_eq!(align_to_cache_line(65), 128);
        assert_eq!(align_to_cache_line(128), 128);
        assert_eq!(align_to_cache_line(129), 192);
    }

    // ---------------------------------------------------------------
    // 14. NullPrefetcher does not panic
    // ---------------------------------------------------------------

    #[test]
    fn test_null_prefetcher_noop() {
        let prefetcher = NullPrefetcher;
        let data = vec![0u8; 1024];

        // Should not panic even with out-of-range offset
        prefetcher.prefetch(&data, 0, 64);
        prefetcher.prefetch(&data, 512, 256);
        prefetcher.prefetch(&data, 2000, 100); // offset > data.len(), still no-op
        prefetcher.prefetch(&[], 0, 0);
    }

    // ---------------------------------------------------------------
    // 15. Adaptive eviction switches between LRU and LFU
    // ---------------------------------------------------------------

    #[test]
    fn test_adaptive_eviction_policy() {
        let mut cache = make_cache(8, 3, EvictionPolicy::Adaptive);

        // Create skewed frequency distribution:
        // Expert 0: 9 accesses, Expert 1: 3 accesses, Expert 2: 1 access
        for _ in 0..9 {
            cache.access(0);
        }
        for _ in 0..3 {
            cache.access(1);
        }
        cache.access(2);

        // Frequencies: 0->9, 1->3, 2->1
        // max_freq(9) >= 3 * min_freq(1) -> skewed -> use LFU
        // LFU evicts expert 2 (frequency=1)
        cache.access(3);

        assert!(
            cache.is_hot(0),
            "Expert 0 (freq=9) should survive adaptive LFU"
        );
        assert!(
            cache.is_hot(1),
            "Expert 1 (freq=3) should survive adaptive LFU"
        );
        assert!(
            !cache.is_hot(2),
            "Expert 2 (freq=1) should be evicted by adaptive LFU"
        );
        assert!(cache.is_hot(3), "Expert 3 was just admitted");
    }

    // ---------------------------------------------------------------
    // 16. Prefetch admit tracks prefetch hits
    // ---------------------------------------------------------------

    #[test]
    fn test_prefetch_admit_tracks_hits() {
        let mut cache = make_cache(8, 4, EvictionPolicy::Lru);

        // Prefetch-admit expert 5
        cache.prefetch_admit(5);
        assert!(cache.is_hot(5));
        assert_eq!(cache.stats().prefetch_hits, 0);

        // Access expert 5 -> should count as a prefetch hit
        let hit = cache.access(5);
        assert!(hit, "Expert 5 is in hot set via prefetch");
        assert_eq!(cache.stats().prefetch_hits, 1);

        // Second access should not count as prefetch hit again
        cache.access(5);
        assert_eq!(cache.stats().prefetch_hits, 1);
    }

    // ---------------------------------------------------------------
    // 17. Batch scheduler handles empty input
    // ---------------------------------------------------------------

    #[test]
    fn test_batch_scheduler_empty() {
        let routing: Vec<(usize, Vec<(usize, f32)>)> = vec![];
        let batches = MoeBatchScheduler::schedule(&routing);
        assert!(batches.is_empty());
    }

    // ---------------------------------------------------------------
    // 18. ExpertCacheConfig default values
    // ---------------------------------------------------------------

    #[test]
    fn test_config_defaults() {
        let config = ExpertCacheConfig::default();
        assert_eq!(config.max_hot_experts, 4);
        assert!((config.prefetch_threshold - 0.1).abs() < 1e-6);
        assert_eq!(config.eviction_policy, EvictionPolicy::Lru);
    }

    // ---------------------------------------------------------------
    // 19. suggest_eviction returns None when not full
    // ---------------------------------------------------------------

    #[test]
    fn test_suggest_eviction_none_when_not_full() {
        let mut cache = make_cache(8, 4, EvictionPolicy::Lru);

        assert!(cache.suggest_eviction().is_none());

        cache.access(0);
        assert!(cache.suggest_eviction().is_none());

        cache.access(1);
        cache.access(2);
        assert!(cache.suggest_eviction().is_none());

        // Fill to capacity
        cache.access(3);
        assert!(cache.suggest_eviction().is_some());
    }

    // ---------------------------------------------------------------
    // 20. Admit is idempotent for already-hot experts
    // ---------------------------------------------------------------

    #[test]
    fn test_admit_idempotent() {
        let mut cache = make_cache(8, 4, EvictionPolicy::Lru);

        cache.admit(0);
        cache.admit(1);
        assert_eq!(cache.hot_count(), 2);

        // Re-admitting should not duplicate
        cache.admit(0);
        cache.admit(1);
        assert_eq!(cache.hot_count(), 2);
    }
}
