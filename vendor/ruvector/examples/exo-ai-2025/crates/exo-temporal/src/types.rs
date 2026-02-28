//! Core type definitions for temporal memory

use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

// Re-export core types from exo-core
pub use exo_core::{Metadata, MetadataValue, Pattern, PatternId, SubstrateTime};

/// Extended pattern with temporal tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalPattern {
    /// Base pattern
    pub pattern: Pattern,
    /// Access count
    pub access_count: usize,
    /// Last access time
    pub last_accessed: SubstrateTime,
}

impl TemporalPattern {
    /// Create new temporal pattern
    pub fn new(pattern: Pattern) -> Self {
        Self {
            pattern,
            access_count: 0,
            last_accessed: SubstrateTime::now(),
        }
    }

    /// Create from components
    pub fn from_embedding(embedding: Vec<f32>, metadata: Metadata) -> Self {
        let pattern = Pattern {
            id: PatternId::new(),
            embedding,
            metadata,
            timestamp: SubstrateTime::now(),
            antecedents: Vec::new(),
            salience: 1.0,
        };
        Self::new(pattern)
    }

    /// Create with antecedents
    pub fn with_antecedents(
        embedding: Vec<f32>,
        metadata: Metadata,
        antecedents: Vec<PatternId>,
    ) -> Self {
        let pattern = Pattern {
            id: PatternId::new(),
            embedding,
            metadata,
            timestamp: SubstrateTime::now(),
            antecedents,
            salience: 1.0,
        };
        Self::new(pattern)
    }

    /// Update access tracking
    pub fn mark_accessed(&mut self) {
        self.access_count += 1;
        self.last_accessed = SubstrateTime::now();
    }

    /// Get pattern ID
    pub fn id(&self) -> PatternId {
        self.pattern.id
    }
}

/// Query for pattern retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    /// Query vector embedding
    pub embedding: Vec<f32>,
    /// Origin pattern (for causal queries)
    pub origin: Option<PatternId>,
    /// Number of results requested
    pub k: usize,
}

impl Query {
    /// Create from embedding
    pub fn from_embedding(embedding: Vec<f32>) -> Self {
        Self {
            embedding,
            origin: None,
            k: 10,
        }
    }

    /// Set origin for causal queries
    pub fn with_origin(mut self, origin: PatternId) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Set number of results
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Compute hash for caching
    pub fn hash(&self) -> u64 {
        use ahash::AHasher;
        let mut hasher = AHasher::default();
        for &val in &self.embedding {
            val.to_bits().hash(&mut hasher);
        }
        if let Some(origin) = &self.origin {
            origin.hash(&mut hasher);
        }
        self.k.hash(&mut hasher);
        hasher.finish()
    }
}

/// Result from causal query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalResult {
    /// Retrieved pattern
    pub pattern: TemporalPattern,
    /// Similarity score
    pub similarity: f32,
    /// Causal distance (edges in causal graph)
    pub causal_distance: Option<usize>,
    /// Temporal distance in nanoseconds
    pub temporal_distance_ns: i64,
    /// Combined relevance score
    pub combined_score: f32,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Pattern ID
    pub id: PatternId,
    /// Pattern
    pub pattern: TemporalPattern,
    /// Similarity score
    pub score: f32,
}

/// Time range for queries
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TimeRange {
    /// Start time (inclusive)
    pub start: SubstrateTime,
    /// End time (inclusive)
    pub end: SubstrateTime,
}

impl TimeRange {
    /// Create new time range
    pub fn new(start: SubstrateTime, end: SubstrateTime) -> Self {
        Self { start, end }
    }

    /// Check if time is within range
    pub fn contains(&self, time: &SubstrateTime) -> bool {
        time >= &self.start && time <= &self.end
    }

    /// Past cone (everything before reference time)
    pub fn past(reference: SubstrateTime) -> Self {
        Self {
            start: SubstrateTime::MIN,
            end: reference,
        }
    }

    /// Future cone (everything after reference time)
    pub fn future(reference: SubstrateTime) -> Self {
        Self {
            start: reference,
            end: SubstrateTime::MAX,
        }
    }
}
