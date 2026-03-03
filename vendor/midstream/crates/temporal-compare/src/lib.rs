//! # Temporal-Compare
//!
//! Advanced temporal sequence comparison and pattern matching.
//!
//! ## Features
//! - Dynamic Time Warping (DTW)
//! - Longest Common Subsequence (LCS)
//! - Edit Distance (Levenshtein)
//! - Pattern matching and detection
//! - Efficient caching

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use thiserror::Error;
use dashmap::DashMap;
use lru::LruCache;
use std::sync::{Arc, Mutex};
use std::num::NonZeroUsize;

/// Errors that can occur during temporal comparison
#[derive(Debug, Error)]
pub enum TemporalError {
    #[error("Sequence too long: {0}")]
    SequenceTooLong(usize),

    #[error("Invalid algorithm: {0}")]
    InvalidAlgorithm(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Invalid pattern length: min={0}, max={1}")]
    InvalidPatternLength(usize, usize),

    #[error("Pattern not found")]
    PatternNotFound,
}

/// A temporal sequence element
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemporalElement<T> {
    pub value: T,
    pub timestamp: u64,
}

/// A temporal sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sequence<T> {
    pub elements: Vec<TemporalElement<T>>,
}

impl<T> Sequence<T> {
    pub fn new() -> Self {
        Self { elements: Vec::new() }
    }

    pub fn push(&mut self, value: T, timestamp: u64) {
        self.elements.push(TemporalElement { value, timestamp });
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

impl<T> Default for Sequence<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Comparison algorithm types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComparisonAlgorithm {
    /// Dynamic Time Warping
    DTW,
    /// Longest Common Subsequence
    LCS,
    /// Edit Distance (Levenshtein)
    EditDistance,
    /// Euclidean distance
    Euclidean,
}

/// Result of a temporal comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub distance: f64,
    pub algorithm: ComparisonAlgorithm,
    pub alignment: Option<Vec<(usize, usize)>>,
}

/// Statistics about cache performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub size: usize,
    pub capacity: usize,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }
}

/// A detected pattern in a sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern<T> {
    /// The pattern sequence
    pub sequence: Vec<T>,
    /// Starting indices of all occurrences
    pub occurrences: Vec<usize>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
}

impl<T> Pattern<T> {
    /// Create a new pattern
    pub fn new(sequence: Vec<T>, occurrences: Vec<usize>, confidence: f64) -> Self {
        Self {
            sequence,
            occurrences,
            confidence,
        }
    }

    /// Get the number of times this pattern occurs
    pub fn frequency(&self) -> usize {
        self.occurrences.len()
    }

    /// Get the length of the pattern
    pub fn length(&self) -> usize {
        self.sequence.len()
    }
}

/// Match result for similarity search
#[derive(Debug, Clone, PartialEq)]
pub struct SimilarityMatch {
    /// Starting index in the haystack
    pub start_index: usize,
    /// Similarity score (0.0 to 1.0, higher is more similar)
    pub similarity: f64,
    /// DTW distance (lower is better)
    pub distance: f64,
}

impl SimilarityMatch {
    pub fn new(start_index: usize, distance: f64) -> Self {
        // Convert distance to similarity score (inverse exponential decay)
        let similarity = (-distance / 10.0).exp();
        Self {
            start_index,
            similarity,
            distance,
        }
    }
}

/// Temporal comparator with caching
pub struct TemporalComparator<T> {
    cache: Arc<Mutex<LruCache<String, ComparisonResult>>>,
    pattern_cache: Arc<Mutex<LruCache<String, Vec<Pattern<T>>>>>,
    similarity_cache: Arc<Mutex<LruCache<String, Vec<SimilarityMatch>>>>,
    cache_hits: Arc<DashMap<String, u64>>,
    cache_misses: Arc<DashMap<String, u64>>,
    max_sequence_length: usize,
}

impl<T> TemporalComparator<T>
where
    T: Clone + PartialEq + fmt::Debug + Serialize + Hash + Eq,
{
    /// Create a new temporal comparator
    pub fn new(cache_size: usize, max_sequence_length: usize) -> Self {
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(cache_size).unwrap()
            ))),
            pattern_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(cache_size).unwrap()
            ))),
            similarity_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(cache_size).unwrap()
            ))),
            cache_hits: Arc::new(DashMap::new()),
            cache_misses: Arc::new(DashMap::new()),
            max_sequence_length,
        }
    }

    /// Compare two sequences using the specified algorithm
    pub fn compare(
        &self,
        seq1: &Sequence<T>,
        seq2: &Sequence<T>,
        algorithm: ComparisonAlgorithm,
    ) -> Result<ComparisonResult, TemporalError> {
        // Check sequence length
        if seq1.len() > self.max_sequence_length || seq2.len() > self.max_sequence_length {
            return Err(TemporalError::SequenceTooLong(
                seq1.len().max(seq2.len())
            ));
        }

        // Generate cache key
        let cache_key = self.cache_key(seq1, seq2, algorithm);

        // Check cache
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(result) = cache.get(&cache_key) {
                self.record_cache_hit(&cache_key);
                return Ok(result.clone());
            }
        }

        self.record_cache_miss(&cache_key);

        // Compute comparison
        let result = match algorithm {
            ComparisonAlgorithm::DTW => self.dtw(seq1, seq2),
            ComparisonAlgorithm::LCS => self.lcs(seq1, seq2),
            ComparisonAlgorithm::EditDistance => self.edit_distance(seq1, seq2),
            ComparisonAlgorithm::Euclidean => self.euclidean(seq1, seq2),
        }?;

        // Store in cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.put(cache_key, result.clone());
        }

        Ok(result)
    }

    /// Dynamic Time Warping implementation
    fn dtw(&self, seq1: &Sequence<T>, seq2: &Sequence<T>) -> Result<ComparisonResult, TemporalError> {
        let n = seq1.len();
        let m = seq2.len();

        if n == 0 || m == 0 {
            return Ok(ComparisonResult {
                distance: (n + m) as f64,
                algorithm: ComparisonAlgorithm::DTW,
                alignment: None,
            });
        }

        // Initialize DTW matrix
        let mut dtw = vec![vec![f64::INFINITY; m + 1]; n + 1];
        dtw[0][0] = 0.0;

        // Fill DTW matrix
        for i in 1..=n {
            for j in 1..=m {
                let cost = if seq1.elements[i-1].value == seq2.elements[j-1].value {
                    0.0
                } else {
                    1.0
                };

                dtw[i][j] = cost + dtw[i-1][j-1].min(dtw[i-1][j]).min(dtw[i][j-1]);
            }
        }

        // Backtrack for alignment
        let mut alignment = Vec::new();
        let (mut i, mut j) = (n, m);

        while i > 0 && j > 0 {
            alignment.push((i - 1, j - 1));

            let min_val = dtw[i-1][j-1].min(dtw[i-1][j]).min(dtw[i][j-1]);

            if dtw[i-1][j-1] == min_val {
                i -= 1;
                j -= 1;
            } else if dtw[i-1][j] == min_val {
                i -= 1;
            } else {
                j -= 1;
            }
        }

        alignment.reverse();

        Ok(ComparisonResult {
            distance: dtw[n][m],
            algorithm: ComparisonAlgorithm::DTW,
            alignment: Some(alignment),
        })
    }

    /// Longest Common Subsequence implementation
    fn lcs(&self, seq1: &Sequence<T>, seq2: &Sequence<T>) -> Result<ComparisonResult, TemporalError> {
        let n = seq1.len();
        let m = seq2.len();

        let mut dp = vec![vec![0; m + 1]; n + 1];

        for i in 1..=n {
            for j in 1..=m {
                if seq1.elements[i-1].value == seq2.elements[j-1].value {
                    dp[i][j] = dp[i-1][j-1] + 1;
                } else {
                    dp[i][j] = dp[i-1][j].max(dp[i][j-1]);
                }
            }
        }

        let lcs_length = dp[n][m];
        let distance = (n + m - 2 * lcs_length) as f64;

        Ok(ComparisonResult {
            distance,
            algorithm: ComparisonAlgorithm::LCS,
            alignment: None,
        })
    }

    /// Edit Distance (Levenshtein) implementation
    fn edit_distance(&self, seq1: &Sequence<T>, seq2: &Sequence<T>) -> Result<ComparisonResult, TemporalError> {
        let n = seq1.len();
        let m = seq2.len();

        let mut dp = vec![vec![0; m + 1]; n + 1];

        for i in 0..=n {
            dp[i][0] = i;
        }
        for j in 0..=m {
            dp[0][j] = j;
        }

        for i in 1..=n {
            for j in 1..=m {
                let cost = if seq1.elements[i-1].value == seq2.elements[j-1].value {
                    0
                } else {
                    1
                };

                dp[i][j] = (dp[i-1][j] + 1)
                    .min(dp[i][j-1] + 1)
                    .min(dp[i-1][j-1] + cost);
            }
        }

        Ok(ComparisonResult {
            distance: dp[n][m] as f64,
            algorithm: ComparisonAlgorithm::EditDistance,
            alignment: None,
        })
    }

    /// Euclidean distance (for numeric sequences)
    fn euclidean(&self, seq1: &Sequence<T>, seq2: &Sequence<T>) -> Result<ComparisonResult, TemporalError> {
        let n = seq1.len().min(seq2.len());
        let mut sum: f64 = 0.0;

        for i in 0..n {
            // Simplified: just count mismatches
            if seq1.elements[i].value != seq2.elements[i].value {
                sum += 1.0;
            }
        }

        Ok(ComparisonResult {
            distance: sum.sqrt(), // f64 type is now explicit from declaration
            algorithm: ComparisonAlgorithm::Euclidean,
            alignment: None,
        })
    }

    /// Generate cache key for a comparison
    fn cache_key(&self, seq1: &Sequence<T>, seq2: &Sequence<T>, algorithm: ComparisonAlgorithm) -> String {
        format!(
            "{:?}:{:?}:{:?}",
            seq1.elements.len(),
            seq2.elements.len(),
            algorithm
        )
    }

    fn record_cache_hit(&self, key: &str) {
        self.cache_hits.entry(key.to_string())
            .and_modify(|v| *v += 1)
            .or_insert(1);
    }

    fn record_cache_miss(&self, key: &str) {
        self.cache_misses.entry(key.to_string())
            .and_modify(|v| *v += 1)
            .or_insert(1);
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        let hits: u64 = self.cache_hits.iter().map(|r| *r.value()).sum();
        let misses: u64 = self.cache_misses.iter().map(|r| *r.value()).sum();

        let (size, capacity) = if let Ok(cache) = self.cache.lock() {
            (cache.len(), cache.cap().get())
        } else {
            (0, 0)
        };

        CacheStats {
            hits,
            misses,
            size,
            capacity,
        }
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.pattern_cache.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.similarity_cache.lock() {
            cache.clear();
        }
        self.cache_hits.clear();
        self.cache_misses.clear();
    }

    /// Find similar sequences within a haystack using generic types
    pub fn find_similar_generic(
        &self,
        haystack: &[T],
        needle: &[T],
        threshold: f64,
    ) -> Result<Vec<SimilarityMatch>, TemporalError> {
        if needle.is_empty() || haystack.len() < needle.len() {
            return Ok(Vec::new());
        }

        // Generate cache key
        let cache_key = format!(
            "similar:{:?}:{:?}:{}",
            haystack.len(),
            needle.len(),
            threshold
        );

        // Check cache
        if let Ok(mut cache) = self.similarity_cache.lock() {
            if let Some(results) = cache.get(&cache_key) {
                self.record_cache_hit(&cache_key);
                return Ok(results.clone());
            }
        }

        self.record_cache_miss(&cache_key);

        let needle_len = needle.len();
        let mut matches = Vec::new();

        // Sliding window approach
        for start_idx in 0..=(haystack.len() - needle_len) {
            let window = &haystack[start_idx..start_idx + needle_len];

            // Convert to Sequence for comparison
            let mut seq1 = Sequence::new();
            for (i, item) in window.iter().enumerate() {
                seq1.push(item.clone(), i as u64);
            }

            let mut seq2 = Sequence::new();
            for (i, item) in needle.iter().enumerate() {
                seq2.push(item.clone(), i as u64);
            }

            // Compute DTW distance
            if let Ok(result) = self.dtw(&seq1, &seq2) {
                // Normalize distance by pattern length
                let normalized_distance = result.distance / needle_len as f64;

                if normalized_distance <= threshold {
                    matches.push(SimilarityMatch::new(start_idx, result.distance));
                }
            }
        }

        // Sort by distance (best matches first)
        matches.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Store in cache
        if let Ok(mut cache) = self.similarity_cache.lock() {
            cache.put(cache_key, matches.clone());
        }

        Ok(matches)
    }

    /// Detect recurring patterns in a sequence
    pub fn detect_recurring_patterns(
        &self,
        sequence: &[T],
        min_length: usize,
        max_length: usize,
    ) -> Result<Vec<Pattern<T>>, TemporalError> {
        if min_length > max_length {
            return Err(TemporalError::InvalidPatternLength(min_length, max_length));
        }

        if sequence.len() < min_length {
            return Ok(Vec::new());
        }

        // Generate cache key
        let cache_key = format!(
            "patterns:{:?}:{}:{}",
            sequence.len(),
            min_length,
            max_length
        );

        // Check cache
        if let Ok(mut cache) = self.pattern_cache.lock() {
            if let Some(patterns) = cache.get(&cache_key) {
                self.record_cache_hit(&cache_key);
                return Ok(patterns.clone());
            }
        }

        self.record_cache_miss(&cache_key);

        let mut pattern_map: HashMap<Vec<T>, Vec<usize>> = HashMap::new();

        // Search for patterns of each length
        for pattern_len in min_length..=max_length.min(sequence.len()) {
            for start_idx in 0..=(sequence.len() - pattern_len) {
                let pattern_seq = sequence[start_idx..start_idx + pattern_len].to_vec();

                pattern_map
                    .entry(pattern_seq)
                    .or_insert_with(Vec::new)
                    .push(start_idx);
            }
        }

        // Filter patterns that occur at least twice
        let mut patterns: Vec<Pattern<T>> = pattern_map
            .into_iter()
            .filter(|(_, occurrences)| occurrences.len() >= 2)
            .map(|(seq, occurrences)| {
                // Calculate confidence based on frequency and pattern length
                let frequency = occurrences.len() as f64;
                let pattern_len = seq.len() as f64;
                let total_possible = (sequence.len() - seq.len() + 1) as f64;

                // Confidence is weighted by frequency and pattern length
                let confidence = ((frequency / total_possible) * (pattern_len / max_length as f64))
                    .min(1.0);

                Pattern::new(seq, occurrences, confidence)
            })
            .collect();

        // Sort by frequency (most common first), then by confidence
        patterns.sort_by(|a, b| {
            b.frequency()
                .cmp(&a.frequency())
                .then_with(|| {
                    b.confidence
                        .partial_cmp(&a.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        // Store in cache
        if let Ok(mut cache) = self.pattern_cache.lock() {
            cache.put(cache_key, patterns.clone());
        }

        Ok(patterns)
    }
}

impl<T> Default for TemporalComparator<T>
where
    T: Clone + PartialEq + fmt::Debug + Serialize + Hash + Eq,
{
    fn default() -> Self {
        Self::new(1000, 10000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_creation() {
        let mut seq: Sequence<i32> = Sequence::new();
        seq.push(1, 100);
        seq.push(2, 200);

        assert_eq!(seq.len(), 2);
        assert!(!seq.is_empty());
    }

    #[test]
    fn test_dtw() {
        let comparator = TemporalComparator::new(100, 1000);

        let mut seq1: Sequence<i32> = Sequence::new();
        seq1.push(1, 100);
        seq1.push(2, 200);
        seq1.push(3, 300);

        let mut seq2: Sequence<i32> = Sequence::new();
        seq2.push(1, 100);
        seq2.push(2, 200);
        seq2.push(3, 300);

        let result = comparator.compare(&seq1, &seq2, ComparisonAlgorithm::DTW).unwrap();
        assert_eq!(result.distance, 0.0);
    }

    #[test]
    fn test_cache() {
        let comparator = TemporalComparator::new(100, 1000);

        let mut seq1: Sequence<i32> = Sequence::new();
        seq1.push(1, 1);
        seq1.push(2, 2);

        let mut seq2: Sequence<i32> = Sequence::new();
        seq2.push(1, 1);
        seq2.push(2, 2);

        // First comparison - cache miss
        comparator.compare(&seq1, &seq2, ComparisonAlgorithm::DTW).unwrap();

        // Second comparison - cache hit
        comparator.compare(&seq1, &seq2, ComparisonAlgorithm::DTW).unwrap();

        let stats = comparator.cache_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_find_similar_generic_integers() {
        let comparator: TemporalComparator<i32> = TemporalComparator::new(100, 1000);

        let haystack = vec![1, 2, 3, 4, 5, 3, 4, 5];
        let needle = vec![3, 4, 5];

        let matches = comparator.find_similar_generic(&haystack, &needle, 0.1).unwrap();

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].start_index, 2);
        assert_eq!(matches[1].start_index, 5);
        assert!(matches[0].similarity > 0.9); // High similarity for exact match
    }

    #[test]
    fn test_detect_recurring_patterns_simple() {
        let comparator: TemporalComparator<char> = TemporalComparator::new(100, 1000);

        let sequence = vec!['a', 'b', 'c', 'a', 'b', 'c', 'a', 'b', 'c'];

        let patterns = comparator.detect_recurring_patterns(&sequence, 2, 4).unwrap();

        assert!(!patterns.is_empty());
        // Should find 'abc' pattern recurring
        let abc_pattern = patterns.iter().find(|p| p.sequence == vec!['a', 'b', 'c']);
        assert!(abc_pattern.is_some());

        let pattern = abc_pattern.unwrap();
        assert_eq!(pattern.frequency(), 3);
        assert!(pattern.confidence > 0.0);
    }
}
