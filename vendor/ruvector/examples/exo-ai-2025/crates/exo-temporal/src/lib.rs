//! # exo-temporal: Temporal Memory Coordinator
//!
//! Causal memory coordination for the EXO-AI cognitive substrate.
//!
//! This crate implements temporal memory with:
//! - Short-term volatile buffer
//! - Long-term consolidated store
//! - Causal graph tracking antecedent relationships
//! - Memory consolidation with salience-based filtering
//! - Predictive anticipation and pre-fetching
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                  TemporalMemory                         │
//! ├─────────────────────────────────────────────────────────┤
//! │ ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │
//! │ │ Short-Term  │  │ Long-Term   │  │   Causal    │      │
//! │ │   Buffer    │→ │    Store    │  │    Graph    │      │
//! │ └─────────────┘  └─────────────┘  └─────────────┘      │
//! │        ↓                ↑                 ↑             │
//! │ ┌─────────────────────────────────────────────┐         │
//! │ │          Consolidation Engine               │         │
//! │ │  (Salience computation & filtering)         │         │
//! │ └─────────────────────────────────────────────┘         │
//! │        ↓                                                │
//! │ ┌─────────────────────────────────────────────┐         │
//! │ │       Anticipation & Prefetch               │         │
//! │ └─────────────────────────────────────────────┘         │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use exo_temporal::{TemporalMemory, TemporalConfig};
//! use exo_core::Pattern;
//!
//! // Create temporal memory
//! let memory = TemporalMemory::new(TemporalConfig::default());
//!
//! // Store pattern with causal context
//! let pattern = Pattern::new(vec![1.0, 2.0, 3.0], metadata);
//! let id = memory.store(pattern, &[]).unwrap();
//!
//! // Causal query
//! let results = memory.causal_query(
//!     &query,
//!     reference_time,
//!     CausalConeType::Past,
//! );
//!
//! // Trigger consolidation
//! memory.consolidate();
//! ```

pub mod anticipation;
pub mod causal;
pub mod consolidation;
pub mod long_term;
pub mod quantum_decay;
pub mod short_term;
pub mod transfer_timeline;
pub mod types;

pub use anticipation::{
    anticipate, AnticipationHint, PrefetchCache, SequentialPatternTracker, TemporalPhase,
};
pub use causal::{CausalConeType, CausalGraph, CausalGraphStats};
pub use consolidation::{
    compute_salience, compute_salience_batch, consolidate, ConsolidationConfig,
    ConsolidationResult, ConsolidationStats,
};
pub use long_term::{LongTermConfig, LongTermStats, LongTermStore};
pub use quantum_decay::{PatternDecoherence, QuantumDecayPool};
pub use short_term::{ShortTermBuffer, ShortTermConfig, ShortTermStats};
pub use types::*;

use thiserror::Error;

/// Error type for temporal memory operations
#[derive(Debug, Error)]
pub enum TemporalError {
    /// Pattern not found
    #[error("Pattern not found: {0}")]
    PatternNotFound(PatternId),

    /// Invalid query
    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),
}

/// Result type for temporal operations
pub type Result<T> = std::result::Result<T, TemporalError>;

/// Configuration for temporal memory
#[derive(Debug, Clone)]
pub struct TemporalConfig {
    /// Short-term buffer configuration
    pub short_term: ShortTermConfig,
    /// Long-term store configuration
    pub long_term: LongTermConfig,
    /// Consolidation configuration
    pub consolidation: ConsolidationConfig,
    /// Prefetch cache capacity
    pub prefetch_capacity: usize,
    /// Auto-consolidation enabled
    pub auto_consolidate: bool,
}

impl Default for TemporalConfig {
    fn default() -> Self {
        Self {
            short_term: ShortTermConfig::default(),
            long_term: LongTermConfig::default(),
            consolidation: ConsolidationConfig::default(),
            prefetch_capacity: 1000,
            auto_consolidate: true,
        }
    }
}

/// Temporal memory coordinator
pub struct TemporalMemory {
    /// Short-term volatile memory
    short_term: ShortTermBuffer,
    /// Long-term consolidated memory
    long_term: LongTermStore,
    /// Causal graph tracking antecedent relationships
    causal_graph: CausalGraph,
    /// Prefetch cache for anticipated queries
    prefetch_cache: PrefetchCache,
    /// Sequential pattern tracker
    sequential_tracker: SequentialPatternTracker,
    /// Configuration
    config: TemporalConfig,
}

impl TemporalMemory {
    /// Create new temporal memory
    pub fn new(config: TemporalConfig) -> Self {
        Self {
            short_term: ShortTermBuffer::new(config.short_term.clone()),
            long_term: LongTermStore::new(config.long_term.clone()),
            causal_graph: CausalGraph::new(),
            prefetch_cache: PrefetchCache::new(config.prefetch_capacity),
            sequential_tracker: SequentialPatternTracker::new(),
            config,
        }
    }

    /// Store pattern with causal context
    pub fn store(&self, pattern: Pattern, antecedents: &[PatternId]) -> Result<PatternId> {
        let id = pattern.id;
        let timestamp = pattern.timestamp;

        // Wrap in TemporalPattern
        let temporal_pattern = TemporalPattern::new(pattern);

        // Add to short-term buffer
        self.short_term.insert(temporal_pattern);

        // Record causal relationships
        self.causal_graph.add_pattern(id, timestamp);
        for &antecedent in antecedents {
            self.causal_graph.add_edge(antecedent, id);
        }

        // Auto-consolidate if needed
        if self.config.auto_consolidate && self.short_term.should_consolidate() {
            self.consolidate();
        }

        Ok(id)
    }

    /// Retrieve pattern by ID
    pub fn get(&self, id: &PatternId) -> Option<Pattern> {
        // Check short-term first
        if let Some(temporal_pattern) = self.short_term.get(id) {
            return Some(temporal_pattern.pattern);
        }

        // Check long-term
        self.long_term.get(id).map(|tp| tp.pattern)
    }

    /// Update pattern access tracking
    pub fn mark_accessed(&self, id: &PatternId) {
        // Update in short-term if present
        self.short_term.get_mut(id, |p| p.mark_accessed());

        // Update in long-term if present
        if let Some(mut temporal_pattern) = self.long_term.get(id) {
            temporal_pattern.mark_accessed();
            self.long_term.update(temporal_pattern);
        }
    }

    /// Causal cone query: retrieve within light-cone constraints
    pub fn causal_query(
        &self,
        query: &Query,
        reference_time: SubstrateTime,
        cone_type: CausalConeType,
    ) -> Vec<CausalResult> {
        // Determine time range based on cone type
        let time_range = match cone_type {
            CausalConeType::Past => TimeRange::past(reference_time),
            CausalConeType::Future => TimeRange::future(reference_time),
            CausalConeType::LightCone { .. } => {
                // Simplified: use full range for now
                // In full implementation, would compute relativistic constraint
                TimeRange::new(SubstrateTime::MIN, SubstrateTime::MAX)
            }
        };

        // Search long-term with temporal filter
        let search_results = self.long_term.search_with_time_range(query, time_range);

        // Compute causal and temporal distances
        let mut results = Vec::new();

        for search_result in search_results {
            let temporal_pattern = search_result.pattern;
            let similarity = search_result.score;

            // Causal distance
            let causal_distance = if let Some(origin) = query.origin {
                self.causal_graph.distance(origin, temporal_pattern.id())
            } else {
                None
            };

            // Temporal distance (in nanoseconds)
            let time_diff = (reference_time - temporal_pattern.pattern.timestamp).abs();
            let temporal_distance_ns = time_diff.0;

            // Combined score (weighted combination)
            const ALPHA: f32 = 0.5; // Similarity weight
            const BETA: f32 = 0.25; // Temporal weight
            const GAMMA: f32 = 0.25; // Causal weight

            let temporal_score = 1.0 / (1.0 + (temporal_distance_ns / 1_000_000_000) as f32); // Convert to seconds
            let causal_score = if let Some(dist) = causal_distance {
                1.0 / (1.0 + dist as f32)
            } else {
                0.0
            };

            let combined_score = ALPHA * similarity + BETA * temporal_score + GAMMA * causal_score;

            results.push(CausalResult {
                pattern: temporal_pattern,
                similarity,
                causal_distance,
                temporal_distance_ns,
                combined_score,
            });
        }

        // Sort by combined score
        results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());

        results
    }

    /// Anticipatory pre-fetch for predictive retrieval
    pub fn anticipate(&self, hints: &[AnticipationHint]) {
        anticipate(
            hints,
            &self.long_term,
            &self.causal_graph,
            &self.prefetch_cache,
            &self.sequential_tracker,
        );
    }

    /// Check prefetch cache for query
    pub fn check_cache(&self, query: &Query) -> Option<Vec<SearchResult>> {
        self.prefetch_cache.get(query.hash())
    }

    /// Memory consolidation: short-term -> long-term
    pub fn consolidate(&self) -> ConsolidationResult {
        consolidate(
            &self.short_term,
            &self.long_term,
            &self.causal_graph,
            &self.config.consolidation,
        )
    }

    /// Strategic forgetting in long-term memory
    pub fn forget(&self) {
        self.long_term
            .decay_low_salience(self.config.long_term.decay_rate);
    }

    /// Get causal graph reference
    pub fn causal_graph(&self) -> &CausalGraph {
        &self.causal_graph
    }

    /// Get short-term buffer reference
    pub fn short_term(&self) -> &ShortTermBuffer {
        &self.short_term
    }

    /// Get long-term store reference
    pub fn long_term(&self) -> &LongTermStore {
        &self.long_term
    }

    /// Get statistics
    pub fn stats(&self) -> TemporalStats {
        TemporalStats {
            short_term: self.short_term.stats(),
            long_term: self.long_term.stats(),
            causal_graph: self.causal_graph.stats(),
            prefetch_cache_size: self.prefetch_cache.len(),
        }
    }
}

impl Default for TemporalMemory {
    fn default() -> Self {
        Self::new(TemporalConfig::default())
    }
}

/// Temporal memory statistics
#[derive(Debug, Clone)]
pub struct TemporalStats {
    /// Short-term buffer stats
    pub short_term: ShortTermStats,
    /// Long-term store stats
    pub long_term: LongTermStats,
    /// Causal graph stats
    pub causal_graph: CausalGraphStats,
    /// Prefetch cache size
    pub prefetch_cache_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_memory() {
        let memory = TemporalMemory::default();

        let pattern = Pattern {
            id: PatternId::new(),
            embedding: vec![1.0, 2.0, 3.0],
            metadata: Metadata::default(),
            timestamp: SubstrateTime::now(),
            antecedents: Vec::new(),
            salience: 1.0,
        };
        let id = pattern.id;

        memory.store(pattern, &[]).unwrap();

        assert!(memory.get(&id).is_some());
    }

    #[test]
    fn test_causal_query() {
        // Use low salience threshold to ensure all patterns are consolidated
        let config = TemporalConfig {
            consolidation: ConsolidationConfig {
                salience_threshold: 0.0, // Accept all patterns
                ..Default::default()
            },
            ..Default::default()
        };
        let memory = TemporalMemory::new(config);

        // Create causal chain: p1 -> p2 -> p3
        let t1 = SubstrateTime::now();
        let p1 = Pattern {
            id: PatternId::new(),
            embedding: vec![1.0, 0.0, 0.0],
            metadata: Metadata::default(),
            timestamp: t1,
            antecedents: Vec::new(),
            salience: 1.0,
        };
        let id1 = p1.id;
        memory.store(p1, &[]).unwrap();

        let p2 = Pattern {
            id: PatternId::new(),
            embedding: vec![0.9, 0.1, 0.0],
            metadata: Metadata::default(),
            timestamp: SubstrateTime::now(),
            antecedents: Vec::new(),
            salience: 1.0,
        };
        let id2 = p2.id;
        memory.store(p2, &[id1]).unwrap();

        let p3 = Pattern {
            id: PatternId::new(),
            embedding: vec![0.8, 0.2, 0.0],
            metadata: Metadata::default(),
            timestamp: SubstrateTime::now(),
            antecedents: Vec::new(),
            salience: 1.0,
        };
        memory.store(p3, &[id2]).unwrap();

        // Consolidate to long-term
        let result = memory.consolidate();
        assert!(
            result.num_consolidated >= 3,
            "Should consolidate all patterns"
        );

        // Query with causal context - use p1's timestamp as reference for future cone
        let query = Query::from_embedding(vec![1.0, 0.0, 0.0]).with_origin(id1);
        let results = memory.causal_query(
            &query,
            t1, // Use p1's timestamp as reference, so p2 and p3 are in the future
            CausalConeType::Future,
        );

        // Should find patterns in the causal future of p1
        assert!(
            !results.is_empty(),
            "Should find causal descendants in future cone"
        );
    }
}
