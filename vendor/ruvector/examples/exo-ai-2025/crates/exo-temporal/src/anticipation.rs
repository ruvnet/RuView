//! Predictive anticipation and pre-fetching

use crate::causal::CausalGraph;
use crate::long_term::LongTermStore;
use crate::types::{PatternId, Query, SearchResult};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;

/// Anticipation hint types
#[derive(Debug, Clone)]
pub enum AnticipationHint {
    /// Sequential pattern: if A then B
    SequentialPattern {
        /// Recent query patterns
        recent: Vec<PatternId>,
    },
    /// Temporal cycle (time-of-day patterns)
    TemporalCycle {
        /// Current temporal phase
        phase: TemporalPhase,
    },
    /// Causal chain prediction
    CausalChain {
        /// Current context pattern
        context: PatternId,
    },
}

/// Temporal phase for cyclic patterns
#[derive(Debug, Clone, Copy)]
pub enum TemporalPhase {
    /// Hour of day (0-23)
    HourOfDay(u8),
    /// Day of week (0-6)
    DayOfWeek(u8),
    /// Custom phase
    Custom(u32),
}

/// Prefetch cache for anticipated queries
pub struct PrefetchCache {
    /// Cached query results
    cache: DashMap<u64, Vec<SearchResult>>,
    /// Cache capacity
    capacity: usize,
    /// LRU tracking
    lru: Arc<RwLock<VecDeque<u64>>>,
}

impl PrefetchCache {
    /// Create new prefetch cache
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: DashMap::new(),
            capacity,
            lru: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
        }
    }

    /// Insert into cache
    pub fn insert(&self, query_hash: u64, results: Vec<SearchResult>) {
        // Check capacity
        if self.cache.len() >= self.capacity {
            self.evict_lru();
        }

        // Insert
        self.cache.insert(query_hash, results);

        // Update LRU
        let mut lru = self.lru.write();
        lru.push_back(query_hash);
    }

    /// Get from cache
    pub fn get(&self, query_hash: u64) -> Option<Vec<SearchResult>> {
        self.cache.get(&query_hash).map(|v| v.clone())
    }

    /// Evict least recently used entry
    fn evict_lru(&self) {
        let mut lru = self.lru.write();
        if let Some(key) = lru.pop_front() {
            self.cache.remove(&key);
        }
    }

    /// Clear cache
    pub fn clear(&self) {
        self.cache.clear();
        self.lru.write().clear();
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for PrefetchCache {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Optimized sequential pattern tracker with pre-computed frequencies
pub struct SequentialPatternTracker {
    /// Pre-computed frequency maps for O(1) prediction lookup
    /// Key: source pattern, Value: sorted vector of (count, target pattern)
    frequency_cache: DashMap<PatternId, Vec<(usize, PatternId)>>,
    /// Raw counts for incremental updates
    counts: DashMap<(PatternId, PatternId), usize>,
    /// Cache validity flags
    cache_valid: DashMap<PatternId, bool>,
    /// Total sequences recorded (for statistics)
    total_sequences: std::sync::atomic::AtomicUsize,
}

impl SequentialPatternTracker {
    /// Create new tracker
    pub fn new() -> Self {
        Self {
            frequency_cache: DashMap::new(),
            counts: DashMap::new(),
            cache_valid: DashMap::new(),
            total_sequences: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Record sequence: A followed by B (optimized with lazy cache invalidation)
    pub fn record_sequence(&self, from: PatternId, to: PatternId) {
        // Increment count atomically
        *self.counts.entry((from, to)).or_insert(0) += 1;

        // Invalidate cache for this source pattern
        self.cache_valid.insert(from, false);

        // Track total sequences
        self.total_sequences
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Predict next pattern given current (optimized O(1) cache lookup)
    pub fn predict_next(&self, current: PatternId, top_k: usize) -> Vec<PatternId> {
        // Check if cache is valid
        let cache_valid = self.cache_valid.get(&current).map(|v| *v).unwrap_or(false);

        if !cache_valid {
            // Rebuild cache for this pattern
            self.rebuild_cache(current);
        }

        // Fast O(1) lookup from pre-sorted cache
        if let Some(sorted) = self.frequency_cache.get(&current) {
            sorted.iter().take(top_k).map(|(_, id)| *id).collect()
        } else {
            Vec::new()
        }
    }

    /// Rebuild frequency cache for a specific pattern
    fn rebuild_cache(&self, pattern: PatternId) {
        let mut freq_vec: Vec<(usize, PatternId)> = Vec::new();

        // Collect all (pattern, target) pairs for this source
        for entry in self.counts.iter() {
            let (from, to) = *entry.key();
            if from == pattern {
                freq_vec.push((*entry.value(), to));
            }
        }

        // Sort by count descending (higher frequency first)
        freq_vec.sort_by(|a, b| b.0.cmp(&a.0));

        // Update cache
        self.frequency_cache.insert(pattern, freq_vec);
        self.cache_valid.insert(pattern, true);
    }

    /// Get total number of recorded sequences
    pub fn total_sequences(&self) -> usize {
        self.total_sequences
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get prediction accuracy estimate (based on frequency distribution)
    pub fn prediction_confidence(&self, pattern: PatternId) -> f32 {
        if let Some(sorted) = self.frequency_cache.get(&pattern) {
            if sorted.is_empty() {
                return 0.0;
            }
            let total: usize = sorted.iter().map(|(c, _)| c).sum();
            if total == 0 {
                return 0.0;
            }
            // Confidence = top prediction count / total count
            sorted[0].0 as f32 / total as f32
        } else {
            0.0
        }
    }

    /// Batch record multiple sequences (optimized for bulk operations)
    pub fn record_sequences_batch(&self, sequences: &[(PatternId, PatternId)]) {
        let mut invalidated = std::collections::HashSet::new();

        for (from, to) in sequences {
            *self.counts.entry((*from, *to)).or_insert(0) += 1;
            invalidated.insert(*from);
        }

        // Batch invalidate caches
        for pattern in invalidated {
            self.cache_valid.insert(pattern, false);
        }

        self.total_sequences
            .fetch_add(sequences.len(), std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for SequentialPatternTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Anticipate future queries and pre-fetch
pub fn anticipate(
    hints: &[AnticipationHint],
    long_term: &LongTermStore,
    causal_graph: &CausalGraph,
    prefetch_cache: &PrefetchCache,
    sequential_tracker: &SequentialPatternTracker,
) -> usize {
    let mut num_prefetched = 0;

    for hint in hints {
        match hint {
            AnticipationHint::SequentialPattern { recent } => {
                // Predict next based on recent patterns
                if let Some(&last) = recent.last() {
                    let predicted = sequential_tracker.predict_next(last, 5);

                    for pattern_id in predicted {
                        if let Some(temporal_pattern) = long_term.get(&pattern_id) {
                            // Create query from pattern
                            let query =
                                Query::from_embedding(temporal_pattern.pattern.embedding.clone());
                            let query_hash = query.hash();

                            // Pre-fetch if not cached
                            if prefetch_cache.get(query_hash).is_none() {
                                let results = long_term.search(&query);
                                prefetch_cache.insert(query_hash, results);
                                num_prefetched += 1;
                            }
                        }
                    }
                }
            }

            AnticipationHint::TemporalCycle { phase } => {
                // Encode the temporal phase as a sinusoidal query vector and
                // pre-fetch high-salience patterns for this recurring time slot.
                let phase_ratio = match phase {
                    TemporalPhase::HourOfDay(h) => *h as f64 / 24.0,
                    TemporalPhase::DayOfWeek(d) => *d as f64 / 7.0,
                    TemporalPhase::Custom(c) => (*c as f64 % 1000.0) / 1000.0,
                };

                // Build a 32-dim sinusoidal embedding for the phase
                let dim = 32usize;
                let query_vec: Vec<f32> = (0..dim)
                    .map(|i| {
                        let angle =
                            2.0 * std::f64::consts::PI * phase_ratio * (i + 1) as f64 / dim as f64;
                        angle.sin() as f32
                    })
                    .collect();

                let query = Query::from_embedding(query_vec);
                let query_hash = query.hash();

                if prefetch_cache.get(query_hash).is_none() {
                    let results = long_term.search(&query);
                    if !results.is_empty() {
                        prefetch_cache.insert(query_hash, results);
                        num_prefetched += 1;
                    }
                }
            }

            AnticipationHint::CausalChain { context } => {
                // Predict downstream patterns in causal graph
                let downstream = causal_graph.causal_future(*context);

                for pattern_id in downstream.into_iter().take(5) {
                    if let Some(temporal_pattern) = long_term.get(&pattern_id) {
                        let query =
                            Query::from_embedding(temporal_pattern.pattern.embedding.clone());
                        let query_hash = query.hash();

                        // Pre-fetch if not cached
                        if prefetch_cache.get(query_hash).is_none() {
                            let results = long_term.search(&query);
                            prefetch_cache.insert(query_hash, results);
                            num_prefetched += 1;
                        }
                    }
                }
            }
        }
    }

    num_prefetched
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefetch_cache() {
        let cache = PrefetchCache::new(2);

        let results1 = vec![];
        let results2 = vec![];

        cache.insert(1, results1);
        cache.insert(2, results2);

        assert_eq!(cache.len(), 2);
        assert!(cache.get(1).is_some());

        // Insert third should evict first (LRU)
        cache.insert(3, vec![]);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(1).is_none());
    }

    #[test]
    fn test_sequential_tracker() {
        let tracker = SequentialPatternTracker::new();

        let p1 = PatternId::new();
        let p2 = PatternId::new();
        let p3 = PatternId::new();

        // p1 -> p2 (twice)
        tracker.record_sequence(p1, p2);
        tracker.record_sequence(p1, p2);

        // p1 -> p3 (once)
        tracker.record_sequence(p1, p3);

        let predicted = tracker.predict_next(p1, 2);

        // p2 should be first (more frequent)
        assert_eq!(predicted.len(), 2);
        assert_eq!(predicted[0], p2);

        // Test total sequences tracking
        assert_eq!(tracker.total_sequences(), 3);

        // Test prediction confidence
        let confidence = tracker.prediction_confidence(p1);
        assert!(confidence > 0.6); // p2 appears 2 out of 3 times
    }

    #[test]
    fn test_batch_recording() {
        let tracker = SequentialPatternTracker::new();

        let p1 = PatternId::new();
        let p2 = PatternId::new();
        let p3 = PatternId::new();

        let sequences = vec![(p1, p2), (p1, p2), (p1, p3), (p2, p3)];

        tracker.record_sequences_batch(&sequences);

        assert_eq!(tracker.total_sequences(), 4);

        let predicted = tracker.predict_next(p1, 1);
        assert_eq!(predicted[0], p2);
    }
}
