//! Agent learning pattern management.
//!
//! Stores learned patterns as vectors with metadata (pattern type, description,
//! effectiveness score) in the RVF store. Patterns can be searched by embedding
//! similarity and ranked by their effectiveness scores.

use std::collections::HashMap;

/// A learning pattern search result.
#[derive(Clone, Debug)]
pub struct PatternResult {
    /// Unique pattern identifier.
    pub id: u64,
    /// The cognitive pattern type (e.g., "convergent", "divergent", "lateral").
    pub pattern_type: String,
    /// Human-readable description of the pattern.
    pub description: String,
    /// Effectiveness score (0.0 - 1.0).
    pub score: f32,
    /// Distance from query embedding (only meaningful in search results).
    pub distance: f32,
}

/// In-memory metadata for a stored pattern.
#[derive(Clone, Debug)]
struct PatternMeta {
    pattern_type: String,
    description: String,
    score: f32,
}

/// Agent learning pattern store.
///
/// Wraps a vector store to provide pattern-specific operations: store, search,
/// update scores, and retrieve top patterns. Each pattern has a type, description,
/// effectiveness score, and an embedding vector for similarity search.
pub struct LearningPatternStore {
    patterns: HashMap<u64, PatternMeta>,
    /// Ordered list of (score, id) for efficient top-k retrieval.
    score_index: Vec<(f32, u64)>,
    next_id: u64,
}

impl LearningPatternStore {
    /// Create a new, empty learning pattern store.
    pub fn new() -> Self {
        Self {
            patterns: HashMap::new(),
            score_index: Vec::new(),
            next_id: 1,
        }
    }

    /// Store a learned pattern.
    ///
    /// The `embedding` is stored in the parent `RvfSwarmStore` via metadata;
    /// this struct tracks the pattern metadata for filtering and ranking.
    ///
    /// Returns the assigned pattern ID.
    pub fn store_pattern(
        &mut self,
        pattern_type: &str,
        description: &str,
        score: f32,
    ) -> Result<u64, LearningError> {
        if pattern_type.is_empty() {
            return Err(LearningError::EmptyPatternType);
        }
        let clamped_score = score.clamp(0.0, 1.0);
        let id = self.next_id;
        self.next_id += 1;

        self.patterns.insert(
            id,
            PatternMeta {
                pattern_type: pattern_type.to_string(),
                description: description.to_string(),
                score: clamped_score,
            },
        );
        self.score_index.push((clamped_score, id));

        Ok(id)
    }

    /// Search patterns by returning those whose IDs are in the given candidate
    /// set (from a vector similarity search), enriched with metadata.
    pub fn enrich_results(
        &self,
        candidates: &[(u64, f32)],
        k: usize,
    ) -> Vec<PatternResult> {
        let mut results: Vec<PatternResult> = candidates
            .iter()
            .filter_map(|&(id, distance)| {
                let meta = self.patterns.get(&id)?;
                Some(PatternResult {
                    id,
                    pattern_type: meta.pattern_type.clone(),
                    description: meta.description.clone(),
                    score: meta.score,
                    distance,
                })
            })
            .collect();
        results.truncate(k);
        results
    }

    /// Update the effectiveness score for a pattern.
    pub fn update_score(&mut self, id: u64, new_score: f32) -> Result<(), LearningError> {
        let meta = self
            .patterns
            .get_mut(&id)
            .ok_or(LearningError::PatternNotFound(id))?;
        let clamped = new_score.clamp(0.0, 1.0);
        meta.score = clamped;

        // Update the score index entry.
        if let Some(entry) = self.score_index.iter_mut().find(|(_, eid)| *eid == id) {
            entry.0 = clamped;
        }

        Ok(())
    }

    /// Get the top-k patterns by effectiveness score (highest first).
    pub fn get_top_patterns(&self, k: usize) -> Vec<PatternResult> {
        let mut sorted = self.score_index.clone();
        sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(k);

        sorted
            .into_iter()
            .filter_map(|(_, id)| {
                let meta = self.patterns.get(&id)?;
                Some(PatternResult {
                    id,
                    pattern_type: meta.pattern_type.clone(),
                    description: meta.description.clone(),
                    score: meta.score,
                    distance: 0.0,
                })
            })
            .collect()
    }

    /// Get a pattern by ID.
    pub fn get_pattern(&self, id: u64) -> Option<PatternResult> {
        let meta = self.patterns.get(&id)?;
        Some(PatternResult {
            id,
            pattern_type: meta.pattern_type.clone(),
            description: meta.description.clone(),
            score: meta.score,
            distance: 0.0,
        })
    }

    /// Get the total number of stored patterns.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Returns true if no patterns are stored.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

impl Default for LearningPatternStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors from learning pattern operations.
#[derive(Clone, Debug, PartialEq)]
pub enum LearningError {
    /// Pattern type must not be empty.
    EmptyPatternType,
    /// Pattern with the given ID was not found.
    PatternNotFound(u64),
}

impl std::fmt::Display for LearningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPatternType => write!(f, "pattern_type must not be empty"),
            Self::PatternNotFound(id) => write!(f, "pattern not found: {id}"),
        }
    }
}

impl std::error::Error for LearningError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_and_retrieve() {
        let mut store = LearningPatternStore::new();
        let id = store.store_pattern("convergent", "Use batched writes", 0.85).unwrap();

        let p = store.get_pattern(id).unwrap();
        assert_eq!(p.pattern_type, "convergent");
        assert_eq!(p.description, "Use batched writes");
        assert!((p.score - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn update_score() {
        let mut store = LearningPatternStore::new();
        let id = store.store_pattern("lateral", "Try alternative approach", 0.5).unwrap();

        store.update_score(id, 0.95).unwrap();
        let p = store.get_pattern(id).unwrap();
        assert!((p.score - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn update_nonexistent_pattern() {
        let mut store = LearningPatternStore::new();
        assert_eq!(
            store.update_score(999, 0.5),
            Err(LearningError::PatternNotFound(999))
        );
    }

    #[test]
    fn top_patterns() {
        let mut store = LearningPatternStore::new();
        store.store_pattern("a", "low", 0.2).unwrap();
        store.store_pattern("b", "mid", 0.5).unwrap();
        store.store_pattern("c", "high", 0.9).unwrap();
        store.store_pattern("d", "highest", 1.0).unwrap();

        let top = store.get_top_patterns(2);
        assert_eq!(top.len(), 2);
        assert!((top[0].score - 1.0).abs() < f32::EPSILON);
        assert!((top[1].score - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn score_clamping() {
        let mut store = LearningPatternStore::new();
        let id1 = store.store_pattern("a", "over", 1.5).unwrap();
        let id2 = store.store_pattern("b", "under", -0.3).unwrap();

        assert!((store.get_pattern(id1).unwrap().score - 1.0).abs() < f32::EPSILON);
        assert!(store.get_pattern(id2).unwrap().score.abs() < f32::EPSILON);
    }

    #[test]
    fn empty_pattern_type_rejected() {
        let mut store = LearningPatternStore::new();
        assert_eq!(
            store.store_pattern("", "desc", 0.5),
            Err(LearningError::EmptyPatternType)
        );
    }

    #[test]
    fn enrich_results() {
        let mut store = LearningPatternStore::new();
        let id1 = store.store_pattern("convergent", "desc1", 0.8).unwrap();
        let id2 = store.store_pattern("divergent", "desc2", 0.6).unwrap();
        let _id3 = store.store_pattern("lateral", "desc3", 0.4).unwrap();

        let candidates = vec![(id1, 0.1), (id2, 0.3), (999, 0.5)];
        let results = store.enrich_results(&candidates, 10);
        // id 999 is filtered out (not in patterns map)
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, id1);
        assert_eq!(results[1].id, id2);
    }

    #[test]
    fn len_and_is_empty() {
        let mut store = LearningPatternStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        store.store_pattern("a", "desc", 0.5).unwrap();
        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn get_nonexistent_pattern() {
        let store = LearningPatternStore::new();
        assert!(store.get_pattern(42).is_none());
    }

    #[test]
    fn top_patterns_empty_store() {
        let store = LearningPatternStore::new();
        assert!(store.get_top_patterns(5).is_empty());
    }
}
