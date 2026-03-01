//! AgentDB adapter for pattern-aware tiering.
//!
//! Provides a bridge between the TieredStore and an external HNSW
//! vector index. When connected, tiering decisions can be influenced
//! by semantic similarity to frequently-accessed patterns.
//!
//! # Overview
//!
//! Block metadata is converted into a compact 4-dimensional embedding
//! via [`pattern_from_meta`], then stored in a [`PatternIndex`]. The
//! [`AdaptiveTiering`] struct combines the index with a
//! [`TierConfig`](crate::tiering::TierConfig) to produce tier
//! suggestions based on weighted neighbor voting.
//!
//! The default [`InMemoryPatternIndex`] uses brute-force linear scan
//! with cosine similarity, suitable for up to ~10K blocks. A real
//! deployment would swap in an HNSW-backed implementation.

use crate::store::{BlockKey, BlockMeta, Tier};
use crate::tiering::TierConfig;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// PatternVector
// ---------------------------------------------------------------------------

/// A block's access-pattern embedding for similarity search.
#[derive(Clone, Debug)]
pub struct PatternVector {
    /// The block this vector represents.
    pub key: BlockKey,
    /// Access-pattern embedding (typically 4 dimensions).
    pub embedding: Vec<f32>,
    /// Tiering score at the time of insertion.
    pub score: f32,
}

// ---------------------------------------------------------------------------
// PatternIndex trait
// ---------------------------------------------------------------------------

/// Trait for a vector index over access-pattern embeddings.
///
/// Implementations range from a simple brute-force scan
/// ([`InMemoryPatternIndex`]) to an HNSW-backed production index.
pub trait PatternIndex {
    /// Insert (or replace) a pattern vector.
    fn insert(&mut self, vec: &PatternVector);

    /// Return the `k` nearest neighbors to `query`, sorted by
    /// descending cosine similarity. Each result is `(key, similarity)`.
    fn search_nearest(&self, query: &[f32], k: usize) -> Vec<(BlockKey, f32)>;

    /// Remove the pattern for `key`, if present.
    fn remove(&mut self, key: BlockKey);

    /// Number of pattern vectors currently stored.
    fn len(&self) -> usize;

    /// Returns `true` if the index contains no vectors.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ---------------------------------------------------------------------------
// Cosine similarity
// ---------------------------------------------------------------------------

/// Compute the cosine similarity between two vectors.
///
/// Returns 0.0 if either vector has zero magnitude or they differ in length.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a_sq = 0.0f32;
    let mut norm_b_sq = 0.0f32;

    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a_sq += x * x;
        norm_b_sq += y * y;
    }

    let denom = norm_a_sq.sqrt() * norm_b_sq.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

// ---------------------------------------------------------------------------
// InMemoryPatternIndex
// ---------------------------------------------------------------------------

/// Brute-force in-memory implementation of [`PatternIndex`].
///
/// Uses a `Vec<PatternVector>` with linear-scan cosine similarity.
/// Adequate for small collections (<10K blocks); a real AgentDB
/// deployment would use HNSW for sub-linear search.
pub struct InMemoryPatternIndex {
    vectors: Vec<PatternVector>,
}

impl InMemoryPatternIndex {
    /// Create a new empty index.
    pub fn new() -> Self {
        Self {
            vectors: Vec::new(),
        }
    }
}

impl Default for InMemoryPatternIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternIndex for InMemoryPatternIndex {
    fn insert(&mut self, vec: &PatternVector) {
        // Remove any existing entry for the same key, then append.
        self.vectors.retain(|v| v.key != vec.key);
        self.vectors.push(vec.clone());
    }

    fn search_nearest(&self, query: &[f32], k: usize) -> Vec<(BlockKey, f32)> {
        if k == 0 || self.vectors.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<(BlockKey, f32)> = self
            .vectors
            .iter()
            .map(|v| (v.key, cosine_similarity(query, &v.embedding)))
            .collect();

        // Sort by descending similarity.
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }

    fn remove(&mut self, key: BlockKey) {
        self.vectors.retain(|v| v.key != key);
    }

    fn len(&self) -> usize {
        self.vectors.len()
    }
}

// ---------------------------------------------------------------------------
// pattern_from_meta
// ---------------------------------------------------------------------------

/// Convert block metadata into a 4-dimensional pattern vector.
///
/// The dimensions encode access-pattern features that are useful for
/// clustering blocks with similar tiering behaviour:
///
/// | Index | Feature          | Range   | Description                              |
/// |-------|------------------|---------|------------------------------------------|
/// | 0     | `ema_rate`       | [0, 1]  | Exponential moving average of access rate|
/// | 1     | `popcount/64`    | [0, 1]  | Fraction of recent ticks with access     |
/// | 2     | `recency_decay`  | (0, 1]  | `1 / (1 + tier_age)` -- inverse staleness|
/// | 3     | `access_count_log` | [0, 1] | `log2(1 + count) / 32` -- normalized log |
pub fn pattern_from_meta(meta: &BlockMeta) -> Vec<f32> {
    let ema = meta.ema_rate.clamp(0.0, 1.0);
    let pop = meta.window.count_ones() as f32 / 64.0;
    let recency = 1.0 / (1.0 + meta.tier_age as f32);
    let count_log = ((1.0 + meta.access_count as f32).log2() / 32.0).clamp(0.0, 1.0);

    vec![ema, pop, recency, count_log]
}

// ---------------------------------------------------------------------------
// AdaptiveTiering
// ---------------------------------------------------------------------------

/// Pattern-aware tiering advisor.
///
/// Combines a [`PatternIndex`] with a [`TierConfig`] to suggest tier
/// assignments based on the tiers of semantically similar blocks.
///
/// # Algorithm
///
/// Given a block's metadata and a set of nearest neighbors (from the
/// pattern index), each neighbor's known tier contributes a weighted
/// vote proportional to its cosine similarity. The tier with the
/// highest cumulative vote is suggested, unless it matches the block's
/// current tier (in which case `None` is returned).
pub struct AdaptiveTiering<I: PatternIndex> {
    /// The underlying pattern vector index.
    pub index: I,
    /// Tiering configuration (thresholds, hysteresis, etc.).
    pub config: TierConfig,
    /// Known tier for each block, updated via [`register_block`].
    block_tiers: HashMap<BlockKey, Tier>,
}

impl<I: PatternIndex> AdaptiveTiering<I> {
    /// Create a new `AdaptiveTiering` with the given index and config.
    pub fn new(index: I, config: TierConfig) -> Self {
        Self {
            index,
            config,
            block_tiers: HashMap::new(),
        }
    }

    /// Register (or update) the known tier for a block.
    ///
    /// This must be called whenever a block changes tier so that
    /// [`suggest_tier`](Self::suggest_tier) can use accurate neighbor
    /// tier information for voting.
    pub fn register_block(&mut self, key: BlockKey, tier: Tier) {
        self.block_tiers.insert(key, tier);
    }

    /// Remove a block from the tier registry and the pattern index.
    pub fn remove_block(&mut self, key: BlockKey) {
        self.block_tiers.remove(&key);
        self.index.remove(key);
    }

    /// Number of blocks registered in the tier map.
    pub fn registered_count(&self) -> usize {
        self.block_tiers.len()
    }

    /// Suggest a tier for `meta` based on its nearest neighbors.
    ///
    /// `neighbors` should be the output of
    /// [`PatternIndex::search_nearest`]: a list of `(BlockKey, similarity)`
    /// pairs. Each neighbor whose tier is known contributes a weighted
    /// vote. The tier with the highest total vote is returned, unless it
    /// matches the block's current tier.
    ///
    /// Returns `None` if:
    /// - `neighbors` is empty,
    /// - no neighbors have known tiers, or
    /// - the consensus tier matches the block's current tier.
    pub fn suggest_tier(&self, meta: &BlockMeta, neighbors: &[(BlockKey, f32)]) -> Option<Tier> {
        if neighbors.is_empty() {
            return None;
        }

        // Accumulate weighted votes per tier.
        // Index 0 = Tier0, 1 = Tier1, 2 = Tier2, 3 = Tier3.
        let mut votes = [0.0f32; 4];
        let mut total_weight = 0.0f32;

        for &(key, similarity) in neighbors {
            if let Some(&tier) = self.block_tiers.get(&key) {
                let weight = similarity.max(0.0);
                votes[tier as u8 as usize] += weight;
                total_weight += weight;
            }
        }

        if total_weight == 0.0 {
            return None;
        }

        // Find the tier with the highest vote. On ties, prefer the
        // hotter tier (lower index) since it was found first.
        let mut best_idx = 0usize;
        let mut best_vote = votes[0];
        for i in 1..4 {
            if votes[i] > best_vote {
                best_vote = votes[i];
                best_idx = i;
            }
        }

        let suggested = match best_idx {
            0 => Tier::Tier0,
            1 => Tier::Tier1,
            2 => Tier::Tier2,
            3 => Tier::Tier3,
            _ => unreachable!(),
        };

        if suggested == meta.tier {
            None
        } else {
            Some(suggested)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{DType, ReconstructPolicy};

    fn make_key(tid: u128, idx: u32) -> BlockKey {
        BlockKey {
            tensor_id: tid,
            block_index: idx,
        }
    }

    fn make_store_meta(
        key: BlockKey,
        tier: Tier,
        ema_rate: f32,
        window: u64,
        access_count: u32,
        tier_age: u32,
    ) -> BlockMeta {
        BlockMeta {
            key,
            dtype: DType::F32,
            tier,
            bits: 8,
            scale: 1.0,
            zero_point: 0,
            created_at: 0,
            last_access_at: 100,
            access_count,
            ema_rate,
            window,
            checksum: 0,
            reconstruct: ReconstructPolicy::None,
            tier_age,
            lineage_parent: None,
            block_bytes: 1024,
        }
    }

    // -- cosine_similarity -------------------------------------------------

    #[test]
    fn cosine_identical_vectors() {
        let v = vec![1.0, 2.0, 3.0, 4.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6, "sim={sim}");
    }

    #[test]
    fn cosine_orthogonal_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6, "sim={sim}");
    }

    #[test]
    fn cosine_opposite_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6, "sim={sim}");
    }

    #[test]
    fn cosine_zero_vector() {
        let a = vec![1.0, 2.0];
        let b = vec![0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_different_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_known_value() {
        // cos([1,1], [1,0]) = 1/sqrt(2) ~ 0.7071
        let a = vec![1.0, 1.0];
        let b = vec![1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        let expected = 1.0 / 2.0f32.sqrt();
        assert!(
            (sim - expected).abs() < 1e-6,
            "sim={sim}, expected={expected}"
        );
    }

    // -- InMemoryPatternIndex ----------------------------------------------

    #[test]
    fn index_insert_and_len() {
        let mut idx = InMemoryPatternIndex::new();
        assert!(idx.is_empty());

        idx.insert(&PatternVector {
            key: make_key(1, 0),
            embedding: vec![1.0, 0.0, 0.0, 0.0],
            score: 0.5,
        });
        assert_eq!(idx.len(), 1);
        assert!(!idx.is_empty());
    }

    #[test]
    fn index_insert_replaces_duplicate_key() {
        let mut idx = InMemoryPatternIndex::new();
        let key = make_key(1, 0);

        idx.insert(&PatternVector {
            key,
            embedding: vec![1.0, 0.0, 0.0, 0.0],
            score: 0.5,
        });
        idx.insert(&PatternVector {
            key,
            embedding: vec![0.0, 1.0, 0.0, 0.0],
            score: 0.8,
        });

        assert_eq!(idx.len(), 1);

        // The search should find the updated embedding.
        let results = idx.search_nearest(&[0.0, 1.0, 0.0, 0.0], 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, key);
        // Similarity should be ~1.0 since embeddings match.
        assert!((results[0].1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn index_remove() {
        let mut idx = InMemoryPatternIndex::new();
        let key = make_key(1, 0);

        idx.insert(&PatternVector {
            key,
            embedding: vec![1.0, 0.0, 0.0, 0.0],
            score: 0.5,
        });
        assert_eq!(idx.len(), 1);

        idx.remove(key);
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn index_remove_nonexistent() {
        let mut idx = InMemoryPatternIndex::new();
        idx.remove(make_key(99, 0)); // should not panic
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn index_search_nearest_ordering() {
        let mut idx = InMemoryPatternIndex::new();

        // Insert three vectors with known geometry.
        idx.insert(&PatternVector {
            key: make_key(1, 0),
            embedding: vec![1.0, 0.0, 0.0, 0.0],
            score: 0.0,
        });
        idx.insert(&PatternVector {
            key: make_key(2, 0),
            embedding: vec![0.7, 0.7, 0.0, 0.0],
            score: 0.0,
        });
        idx.insert(&PatternVector {
            key: make_key(3, 0),
            embedding: vec![0.0, 1.0, 0.0, 0.0],
            score: 0.0,
        });

        // Query close to [1, 0, 0, 0].
        let results = idx.search_nearest(&[1.0, 0.1, 0.0, 0.0], 3);
        assert_eq!(results.len(), 3);

        // Closest should be key 1 (nearly identical direction).
        assert_eq!(results[0].0, make_key(1, 0));
        // Second should be key 2 (partial overlap).
        assert_eq!(results[1].0, make_key(2, 0));
        // Third should be key 3 (mostly orthogonal).
        assert_eq!(results[2].0, make_key(3, 0));

        // Similarities should be descending.
        assert!(results[0].1 >= results[1].1);
        assert!(results[1].1 >= results[2].1);
    }

    #[test]
    fn index_search_nearest_k_larger_than_size() {
        let mut idx = InMemoryPatternIndex::new();
        idx.insert(&PatternVector {
            key: make_key(1, 0),
            embedding: vec![1.0, 0.0],
            score: 0.0,
        });

        let results = idx.search_nearest(&[1.0, 0.0], 10);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn index_search_nearest_k_zero() {
        let mut idx = InMemoryPatternIndex::new();
        idx.insert(&PatternVector {
            key: make_key(1, 0),
            embedding: vec![1.0],
            score: 0.0,
        });

        let results = idx.search_nearest(&[1.0], 0);
        assert!(results.is_empty());
    }

    #[test]
    fn index_search_nearest_empty() {
        let idx = InMemoryPatternIndex::new();
        let results = idx.search_nearest(&[1.0, 0.0], 5);
        assert!(results.is_empty());
    }

    // -- pattern_from_meta -------------------------------------------------

    #[test]
    fn pattern_from_meta_dimensions() {
        let meta = make_store_meta(make_key(1, 0), Tier::Tier1, 0.5, 0xFFFF, 100, 10);
        let pat = pattern_from_meta(&meta);
        assert_eq!(pat.len(), 4);
    }

    #[test]
    fn pattern_from_meta_ema_component() {
        let meta = make_store_meta(make_key(1, 0), Tier::Tier1, 0.8, 0, 0, 0);
        let pat = pattern_from_meta(&meta);
        assert!((pat[0] - 0.8).abs() < 1e-6, "ema={}", pat[0]);
    }

    #[test]
    fn pattern_from_meta_popcount_component() {
        // All 64 bits set.
        let meta = make_store_meta(make_key(1, 0), Tier::Tier1, 0.0, u64::MAX, 0, 0);
        let pat = pattern_from_meta(&meta);
        assert!((pat[1] - 1.0).abs() < 1e-6, "pop={}", pat[1]);

        // No bits set.
        let meta2 = make_store_meta(make_key(1, 0), Tier::Tier1, 0.0, 0, 0, 0);
        let pat2 = pattern_from_meta(&meta2);
        assert!((pat2[1]).abs() < 1e-6, "pop={}", pat2[1]);

        // 32 bits set.
        let meta3 = make_store_meta(make_key(1, 0), Tier::Tier1, 0.0, 0xFFFF_FFFF, 0, 0);
        let pat3 = pattern_from_meta(&meta3);
        assert!((pat3[1] - 0.5).abs() < 1e-6, "pop={}", pat3[1]);
    }

    #[test]
    fn pattern_from_meta_recency_component() {
        // tier_age = 0 => recency = 1.0 / (1.0 + 0) = 1.0
        let meta = make_store_meta(make_key(1, 0), Tier::Tier1, 0.0, 0, 0, 0);
        let pat = pattern_from_meta(&meta);
        assert!((pat[2] - 1.0).abs() < 1e-6, "recency={}", pat[2]);

        // tier_age = 9 => recency = 1.0 / 10.0 = 0.1
        let meta2 = make_store_meta(make_key(1, 0), Tier::Tier1, 0.0, 0, 0, 9);
        let pat2 = pattern_from_meta(&meta2);
        assert!((pat2[2] - 0.1).abs() < 1e-6, "recency={}", pat2[2]);
    }

    #[test]
    fn pattern_from_meta_access_count_log_component() {
        // access_count = 0 => log2(1) / 32 = 0
        let meta = make_store_meta(make_key(1, 0), Tier::Tier1, 0.0, 0, 0, 0);
        let pat = pattern_from_meta(&meta);
        assert!(pat[3].abs() < 1e-6, "count_log={}", pat[3]);

        // access_count = 1 => log2(2) / 32 = 1/32 ~ 0.03125
        let meta2 = make_store_meta(make_key(1, 0), Tier::Tier1, 0.0, 0, 1, 0);
        let pat2 = pattern_from_meta(&meta2);
        assert!((pat2[3] - 1.0 / 32.0).abs() < 1e-4, "count_log={}", pat2[3]);
    }

    #[test]
    fn pattern_from_meta_values_in_unit_range() {
        // Use extreme values to verify clamping.
        let meta = make_store_meta(
            make_key(1, 0),
            Tier::Tier1,
            2.0,      // ema > 1, should be clamped
            u64::MAX, // all bits set
            u32::MAX, // max access count
            u32::MAX, // max tier age
        );
        let pat = pattern_from_meta(&meta);
        for (i, &v) in pat.iter().enumerate() {
            assert!(v >= 0.0 && v <= 1.0, "dim {i} out of [0,1]: {v}");
        }
    }

    // -- AdaptiveTiering ---------------------------------------------------

    #[test]
    fn adaptive_new_and_register() {
        let idx = InMemoryPatternIndex::new();
        let config = TierConfig::default();
        let mut at = AdaptiveTiering::new(idx, config);

        assert_eq!(at.registered_count(), 0);

        at.register_block(make_key(1, 0), Tier::Tier1);
        assert_eq!(at.registered_count(), 1);

        at.register_block(make_key(1, 0), Tier::Tier2);
        assert_eq!(at.registered_count(), 1); // same key, updated
    }

    #[test]
    fn adaptive_remove_block() {
        let mut idx = InMemoryPatternIndex::new();
        let key = make_key(1, 0);
        idx.insert(&PatternVector {
            key,
            embedding: vec![1.0, 0.0, 0.0, 0.0],
            score: 0.5,
        });

        let config = TierConfig::default();
        let mut at = AdaptiveTiering::new(idx, config);
        at.register_block(key, Tier::Tier1);
        assert_eq!(at.registered_count(), 1);
        assert_eq!(at.index.len(), 1);

        at.remove_block(key);
        assert_eq!(at.registered_count(), 0);
        assert_eq!(at.index.len(), 0);
    }

    #[test]
    fn suggest_tier_empty_neighbors() {
        let idx = InMemoryPatternIndex::new();
        let config = TierConfig::default();
        let at = AdaptiveTiering::new(idx, config);

        let meta = make_store_meta(make_key(1, 0), Tier::Tier1, 0.5, 0, 10, 5);
        let result = at.suggest_tier(&meta, &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn suggest_tier_no_known_neighbors() {
        let idx = InMemoryPatternIndex::new();
        let config = TierConfig::default();
        let at = AdaptiveTiering::new(idx, config);

        let meta = make_store_meta(make_key(1, 0), Tier::Tier1, 0.5, 0, 10, 5);
        // Neighbors exist but their tiers are not registered.
        let neighbors = vec![(make_key(2, 0), 0.9), (make_key(3, 0), 0.8)];
        let result = at.suggest_tier(&meta, &neighbors);
        assert_eq!(result, None);
    }

    #[test]
    fn suggest_tier_unanimous_vote() {
        let idx = InMemoryPatternIndex::new();
        let config = TierConfig::default();
        let mut at = AdaptiveTiering::new(idx, config);

        // Register three neighbors all in Tier3.
        at.register_block(make_key(2, 0), Tier::Tier3);
        at.register_block(make_key(3, 0), Tier::Tier3);
        at.register_block(make_key(4, 0), Tier::Tier3);

        let meta = make_store_meta(make_key(1, 0), Tier::Tier1, 0.5, 0, 10, 5);
        let neighbors = vec![
            (make_key(2, 0), 0.9),
            (make_key(3, 0), 0.8),
            (make_key(4, 0), 0.7),
        ];

        let result = at.suggest_tier(&meta, &neighbors);
        assert_eq!(result, Some(Tier::Tier3));
    }

    #[test]
    fn suggest_tier_same_as_current_returns_none() {
        let idx = InMemoryPatternIndex::new();
        let config = TierConfig::default();
        let mut at = AdaptiveTiering::new(idx, config);

        // Neighbors all in Tier1, same as the block.
        at.register_block(make_key(2, 0), Tier::Tier1);
        at.register_block(make_key(3, 0), Tier::Tier1);

        let meta = make_store_meta(make_key(1, 0), Tier::Tier1, 0.5, 0, 10, 5);
        let neighbors = vec![(make_key(2, 0), 0.9), (make_key(3, 0), 0.8)];

        let result = at.suggest_tier(&meta, &neighbors);
        assert_eq!(result, None);
    }

    #[test]
    fn suggest_tier_weighted_majority() {
        let idx = InMemoryPatternIndex::new();
        let config = TierConfig::default();
        let mut at = AdaptiveTiering::new(idx, config);

        // Two neighbors in Tier1 with moderate similarity.
        at.register_block(make_key(2, 0), Tier::Tier1);
        at.register_block(make_key(3, 0), Tier::Tier1);
        // One neighbor in Tier3 with very high similarity.
        at.register_block(make_key(4, 0), Tier::Tier3);

        let meta = make_store_meta(make_key(1, 0), Tier::Tier2, 0.5, 0, 10, 5);
        let neighbors = vec![
            (make_key(2, 0), 0.3), // votes Tier1 with weight 0.3
            (make_key(3, 0), 0.3), // votes Tier1 with weight 0.3
            (make_key(4, 0), 0.9), // votes Tier3 with weight 0.9
        ];
        // Tier1 total = 0.6, Tier3 total = 0.9. Tier3 wins.
        let result = at.suggest_tier(&meta, &neighbors);
        assert_eq!(result, Some(Tier::Tier3));
    }

    #[test]
    fn suggest_tier_negative_similarity_ignored() {
        let idx = InMemoryPatternIndex::new();
        let config = TierConfig::default();
        let mut at = AdaptiveTiering::new(idx, config);

        at.register_block(make_key(2, 0), Tier::Tier3);
        at.register_block(make_key(3, 0), Tier::Tier1);

        let meta = make_store_meta(make_key(1, 0), Tier::Tier2, 0.5, 0, 10, 5);
        let neighbors = vec![
            (make_key(2, 0), -0.5), // negative similarity, weight clamped to 0
            (make_key(3, 0), 0.5),  // positive similarity, votes Tier1
        ];
        // Tier3 gets 0 weight (clamped), Tier1 gets 0.5. Tier1 wins.
        let result = at.suggest_tier(&meta, &neighbors);
        assert_eq!(result, Some(Tier::Tier1));
    }

    #[test]
    fn suggest_tier_zero_similarity_all() {
        let idx = InMemoryPatternIndex::new();
        let config = TierConfig::default();
        let mut at = AdaptiveTiering::new(idx, config);

        at.register_block(make_key(2, 0), Tier::Tier3);

        let meta = make_store_meta(make_key(1, 0), Tier::Tier1, 0.5, 0, 10, 5);
        let neighbors = vec![(make_key(2, 0), 0.0)];

        // Zero similarity means zero weight => total_weight == 0 => None.
        let result = at.suggest_tier(&meta, &neighbors);
        assert_eq!(result, None);
    }

    // -- Integration: pattern_from_meta + index + adaptive -----------------

    #[test]
    fn integration_end_to_end() {
        let mut idx = InMemoryPatternIndex::new();
        let config = TierConfig::default();

        // Create several blocks with different access patterns.
        let hot_key = make_key(1, 0);
        let warm_key = make_key(2, 0);
        let cold_key = make_key(3, 0);

        let hot_meta = make_store_meta(hot_key, Tier::Tier1, 0.9, u64::MAX, 1000, 2);
        let warm_meta = make_store_meta(warm_key, Tier::Tier2, 0.5, 0xFFFF_FFFF, 100, 10);
        let cold_meta = make_store_meta(cold_key, Tier::Tier3, 0.05, 0x0F, 5, 100);

        // Build embeddings and insert into index.
        let hot_emb = pattern_from_meta(&hot_meta);
        let warm_emb = pattern_from_meta(&warm_meta);
        let cold_emb = pattern_from_meta(&cold_meta);

        idx.insert(&PatternVector {
            key: hot_key,
            embedding: hot_emb.clone(),
            score: 0.9,
        });
        idx.insert(&PatternVector {
            key: warm_key,
            embedding: warm_emb.clone(),
            score: 0.5,
        });
        idx.insert(&PatternVector {
            key: cold_key,
            embedding: cold_emb.clone(),
            score: 0.1,
        });

        let mut at = AdaptiveTiering::new(idx, config);
        at.register_block(hot_key, Tier::Tier1);
        at.register_block(warm_key, Tier::Tier2);
        at.register_block(cold_key, Tier::Tier3);

        // Query: a new block with a hot-like pattern.
        let new_key = make_key(4, 0);
        let new_meta = make_store_meta(new_key, Tier::Tier3, 0.85, u64::MAX, 800, 3);
        let new_emb = pattern_from_meta(&new_meta);

        let neighbors = at.index.search_nearest(&new_emb, 3);
        assert!(!neighbors.is_empty());

        let suggestion = at.suggest_tier(&new_meta, &neighbors);
        // The new block's pattern is closest to the hot block, so
        // the suggestion should be to promote it (away from Tier3).
        assert!(
            suggestion.is_some(),
            "expected a tier suggestion for a hot-like pattern in Tier3"
        );
        let suggested = suggestion.unwrap();
        assert_ne!(suggested, Tier::Tier3, "should not stay cold");
    }
}
