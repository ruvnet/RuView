//! Temporal sequence comparison and pattern matching
//!
//! Integrates temporal-compare crate for:
//! - Dynamic Time Warping (DTW)
//! - Longest Common Subsequence (LCS)
//! - Edit Distance
//! - Pattern detection in temporal sequences

use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::collections::HashMap;
use std::num::NonZeroUsize;

/// Comparison algorithm selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonAlgorithm {
    /// Dynamic Time Warping - best for temporal alignment
    DTW,
    /// Longest Common Subsequence - best for pattern matching
    LCS,
    /// Edit Distance (Levenshtein) - best for similarity measurement
    EditDistance,
    /// Cross-correlation - best for signal processing
    Correlation,
}

/// A sequence of temporal elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sequence<T> {
    pub data: Vec<T>,
    pub timestamp: i64,
    pub id: String,
}

impl<T: Hash> Hash for Sequence<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.hash(state);
        self.id.hash(state);
    }
}

/// Pair of sequences for caching
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SequencePair {
    id1: String,
    id2: String,
    algorithm: ComparisonAlgorithm,
}

/// Temporal comparator with caching
pub struct TemporalComparator<T: Clone + PartialEq> {
    sequences: Vec<Sequence<T>>,
    cache: LruCache<SequencePair, f64>,
    algorithm_cache: HashMap<ComparisonAlgorithm, usize>,
}

impl<T: Clone + PartialEq + Hash> TemporalComparator<T> {
    /// Create a new temporal comparator
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }

    /// Create with specific cache capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            sequences: Vec::new(),
            cache: LruCache::new(NonZeroUsize::new(capacity).unwrap()),
            algorithm_cache: HashMap::new(),
        }
    }

    /// Add a sequence to the store
    pub fn add_sequence(&mut self, sequence: Sequence<T>) {
        self.sequences.push(sequence);
    }

    /// Compare two sequences using specified algorithm
    pub fn compare(
        &mut self,
        seq1: &[T],
        seq2: &[T],
        algorithm: ComparisonAlgorithm,
    ) -> f64 {
        // Check cache first
        let cache_key = SequencePair {
            id1: format!("{:?}", seq1),
            id2: format!("{:?}", seq2),
            algorithm,
        };

        if let Some(&cached) = self.cache.get(&cache_key) {
            return cached;
        }

        // Compute similarity
        let similarity = match algorithm {
            ComparisonAlgorithm::DTW => self.dtw(seq1, seq2),
            ComparisonAlgorithm::LCS => self.lcs(seq1, seq2),
            ComparisonAlgorithm::EditDistance => self.edit_distance(seq1, seq2),
            ComparisonAlgorithm::Correlation => self.correlation(seq1, seq2),
        };

        // Cache result
        self.cache.put(cache_key.clone(), similarity);
        *self.algorithm_cache.entry(algorithm).or_insert(0) += 1;

        similarity
    }

    /// Dynamic Time Warping distance
    fn dtw(&self, seq1: &[T], seq2: &[T]) -> f64 {
        let n = seq1.len();
        let m = seq2.len();

        if n == 0 || m == 0 {
            return f64::MAX;
        }

        // Initialize DTW matrix
        let mut dtw = vec![vec![f64::MAX; m + 1]; n + 1];
        dtw[0][0] = 0.0;

        // Fill DTW matrix
        for i in 1..=n {
            for j in 1..=m {
                let cost = if seq1[i - 1] == seq2[j - 1] { 0.0 } else { 1.0 };
                dtw[i][j] = cost + dtw[i - 1][j - 1].min(dtw[i - 1][j]).min(dtw[i][j - 1]);
            }
        }

        // Return normalized distance
        dtw[n][m] / (n + m) as f64
    }

    /// Longest Common Subsequence
    fn lcs(&self, seq1: &[T], seq2: &[T]) -> f64 {
        let n = seq1.len();
        let m = seq2.len();

        if n == 0 || m == 0 {
            return 0.0;
        }

        // Initialize LCS matrix
        let mut lcs = vec![vec![0; m + 1]; n + 1];

        // Fill LCS matrix
        for i in 1..=n {
            for j in 1..=m {
                if seq1[i - 1] == seq2[j - 1] {
                    lcs[i][j] = lcs[i - 1][j - 1] + 1;
                } else {
                    lcs[i][j] = lcs[i - 1][j].max(lcs[i][j - 1]);
                }
            }
        }

        // Return normalized similarity (0.0 to 1.0)
        lcs[n][m] as f64 / n.min(m) as f64
    }

    /// Edit distance (Levenshtein)
    fn edit_distance(&self, seq1: &[T], seq2: &[T]) -> f64 {
        let n = seq1.len();
        let m = seq2.len();

        if n == 0 {
            return m as f64;
        }
        if m == 0 {
            return n as f64;
        }

        // Initialize distance matrix
        let mut dist = vec![vec![0; m + 1]; n + 1];

        for i in 0..=n {
            dist[i][0] = i;
        }
        for j in 0..=m {
            dist[0][j] = j;
        }

        // Fill distance matrix
        for i in 1..=n {
            for j in 1..=m {
                let cost = if seq1[i - 1] == seq2[j - 1] { 0 } else { 1 };
                dist[i][j] = (dist[i - 1][j] + 1)
                    .min(dist[i][j - 1] + 1)
                    .min(dist[i - 1][j - 1] + cost);
            }
        }

        // Return normalized distance
        dist[n][m] as f64 / n.max(m) as f64
    }

    /// Cross-correlation (simple version for discrete sequences)
    fn correlation(&self, seq1: &[T], seq2: &[T]) -> f64 {
        if seq1.is_empty() || seq2.is_empty() {
            return 0.0;
        }

        let min_len = seq1.len().min(seq2.len());
        let mut matches = 0;

        for i in 0..min_len {
            if seq1[i] == seq2[i] {
                matches += 1;
            }
        }

        matches as f64 / min_len as f64
    }

    /// Find sequences similar to query above threshold
    pub fn find_similar(
        &mut self,
        query: &[T],
        threshold: f64,
        algorithm: ComparisonAlgorithm,
    ) -> Vec<(usize, f64)> {
        let mut results = Vec::new();

        for (idx, seq) in self.sequences.iter().enumerate() {
            let similarity = self.compare(query, &seq.data, algorithm);

            // For DTW and EditDistance, lower is better
            let passes = match algorithm {
                ComparisonAlgorithm::DTW | ComparisonAlgorithm::EditDistance => {
                    similarity <= threshold
                }
                ComparisonAlgorithm::LCS | ComparisonAlgorithm::Correlation => {
                    similarity >= threshold
                }
            };

            if passes {
                results.push((idx, similarity));
            }
        }

        // Sort by similarity (best first)
        results.sort_by(|a, b| {
            match algorithm {
                ComparisonAlgorithm::DTW | ComparisonAlgorithm::EditDistance => {
                    a.1.partial_cmp(&b.1).unwrap()
                }
                ComparisonAlgorithm::LCS | ComparisonAlgorithm::Correlation => {
                    b.1.partial_cmp(&a.1).unwrap()
                }
            }
        });

        results
    }

    /// Detect pattern occurrences in sequence
    pub fn detect_pattern(&self, sequence: &[T], pattern: &[T]) -> Vec<usize> {
        let mut positions = Vec::new();

        if pattern.is_empty() || sequence.len() < pattern.len() {
            return positions;
        }

        for i in 0..=(sequence.len() - pattern.len()) {
            if &sequence[i..i + pattern.len()] == pattern {
                positions.push(i);
            }
        }

        positions
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            cache_size: self.cache.len(),
            total_comparisons: self.algorithm_cache.values().sum(),
            dtw_count: *self.algorithm_cache.get(&ComparisonAlgorithm::DTW).unwrap_or(&0),
            lcs_count: *self.algorithm_cache.get(&ComparisonAlgorithm::LCS).unwrap_or(&0),
            edit_distance_count: *self.algorithm_cache.get(&ComparisonAlgorithm::EditDistance).unwrap_or(&0),
            correlation_count: *self.algorithm_cache.get(&ComparisonAlgorithm::Correlation).unwrap_or(&0),
        }
    }

    /// Clear all caches
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.algorithm_cache.clear();
    }
}

impl<T: Clone + PartialEq + Hash> Default for TemporalComparator<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub cache_size: usize,
    pub total_comparisons: usize,
    pub dtw_count: usize,
    pub lcs_count: usize,
    pub edit_distance_count: usize,
    pub correlation_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dtw() {
        let mut comparator = TemporalComparator::<i32>::new();

        let seq1 = vec![1, 2, 3, 4, 5];
        let seq2 = vec![1, 2, 3, 4, 5];

        let distance = comparator.compare(&seq1, &seq2, ComparisonAlgorithm::DTW);
        assert!(distance < 0.1); // Should be very similar
    }

    #[test]
    fn test_lcs() {
        let mut comparator = TemporalComparator::<char>::new();

        let seq1 = vec!['a', 'b', 'c', 'd'];
        let seq2 = vec!['a', 'x', 'c', 'd'];

        let similarity = comparator.compare(&seq1, &seq2, ComparisonAlgorithm::LCS);
        assert!(similarity > 0.7); // Should find common subsequence
    }

    #[test]
    fn test_edit_distance() {
        let mut comparator = TemporalComparator::<char>::new();

        let seq1 = vec!['k', 'i', 't', 't', 'e', 'n'];
        let seq2 = vec!['s', 'i', 't', 't', 'i', 'n', 'g'];

        let distance = comparator.compare(&seq1, &seq2, ComparisonAlgorithm::EditDistance);
        assert!(distance > 0.0); // Should detect differences
    }

    #[test]
    fn test_pattern_detection() {
        let comparator = TemporalComparator::<i32>::new();

        let sequence = vec![1, 2, 3, 1, 2, 3, 4, 1, 2, 3];
        let pattern = vec![1, 2, 3];

        let positions = comparator.detect_pattern(&sequence, &pattern);
        assert_eq!(positions, vec![0, 3, 7]);
    }

    #[test]
    fn test_find_similar() {
        let mut comparator = TemporalComparator::<i32>::new();

        comparator.add_sequence(Sequence {
            data: vec![1, 2, 3, 4],
            timestamp: 1000,
            id: "seq1".to_string(),
        });

        comparator.add_sequence(Sequence {
            data: vec![1, 2, 3, 5],
            timestamp: 2000,
            id: "seq2".to_string(),
        });

        comparator.add_sequence(Sequence {
            data: vec![5, 6, 7, 8],
            timestamp: 3000,
            id: "seq3".to_string(),
        });

        let query = vec![1, 2, 3, 4];
        let similar = comparator.find_similar(&query, 0.5, ComparisonAlgorithm::LCS);

        assert!(!similar.is_empty());
    }

    #[test]
    fn test_cache() {
        let mut comparator = TemporalComparator::<i32>::new();

        let seq1 = vec![1, 2, 3];
        let seq2 = vec![1, 2, 4];

        // First comparison - not cached
        let result1 = comparator.compare(&seq1, &seq2, ComparisonAlgorithm::DTW);

        // Second comparison - should be cached
        let result2 = comparator.compare(&seq1, &seq2, ComparisonAlgorithm::DTW);

        assert_eq!(result1, result2);

        let stats = comparator.cache_stats();
        assert_eq!(stats.dtw_count, 1); // Only computed once
    }
}
