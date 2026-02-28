//! ReasoningBank - Storage and retrieval of learned patterns

use super::patterns::LearnedPattern;
use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;

/// Pattern storage entry
#[derive(Debug, Clone)]
struct PatternEntry {
    pattern: LearnedPattern,
    usage_count: usize,
    last_used: SystemTime,
}

/// ReasoningBank for storing and retrieving learned patterns
pub struct ReasoningBank {
    /// Stored patterns indexed by ID
    patterns: DashMap<usize, PatternEntry>,
    /// Next pattern ID
    next_id: AtomicUsize,
}

impl ReasoningBank {
    /// Create a new ReasoningBank
    pub fn new() -> Self {
        Self {
            patterns: DashMap::new(),
            next_id: AtomicUsize::new(0),
        }
    }

    /// Store a new pattern
    pub fn store(&self, pattern: LearnedPattern) -> usize {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let entry = PatternEntry {
            pattern,
            usage_count: 0,
            last_used: SystemTime::now(),
        };

        self.patterns.insert(id, entry);
        id
    }

    /// Lookup k most similar patterns to a query
    pub fn lookup(&self, query: &[f32], k: usize) -> Vec<(usize, LearnedPattern, f64)> {
        let mut similarities: Vec<(usize, LearnedPattern, f64)> = self
            .patterns
            .iter()
            .map(|entry| {
                let id = *entry.key();
                let pattern = &entry.value().pattern;
                let similarity = pattern.similarity(query);
                (id, pattern.clone(), similarity)
            })
            .collect();

        // Sort by similarity (descending) and confidence
        similarities.sort_by(|a, b| {
            let score_a = a.2 * a.1.confidence;
            let score_b = b.2 * b.1.confidence;
            score_b.partial_cmp(&score_a).unwrap()
        });

        // Take top k
        similarities.truncate(k);

        // Update usage statistics
        for (id, _, _) in &similarities {
            if let Some(mut entry) = self.patterns.get_mut(id) {
                entry.usage_count += 1;
                entry.last_used = SystemTime::now();
            }
        }

        similarities
    }

    /// Get a specific pattern by ID
    pub fn get(&self, id: usize) -> Option<LearnedPattern> {
        self.patterns.get_mut(&id).map(|mut entry| {
            entry.usage_count += 1;
            entry.last_used = SystemTime::now();
            entry.pattern.clone()
        })
    }

    /// Consolidate similar patterns
    pub fn consolidate(&self, similarity_threshold: f64) -> usize {
        let patterns: Vec<(usize, LearnedPattern)> = self
            .patterns
            .iter()
            .map(|entry| (*entry.key(), entry.value().pattern.clone()))
            .collect();

        if patterns.len() < 2 {
            return 0;
        }

        let mut to_remove = Vec::new();
        let mut merged = 0;

        for i in 0..patterns.len() {
            if to_remove.contains(&patterns[i].0) {
                continue;
            }

            for j in (i + 1)..patterns.len() {
                if to_remove.contains(&patterns[j].0) {
                    continue;
                }

                let sim = patterns[i].1.similarity(&patterns[j].1.centroid);

                if sim >= similarity_threshold {
                    // Merge j into i
                    if let Some(mut entry_i) = self.patterns.get_mut(&patterns[i].0) {
                        if let Some(entry_j) = self.patterns.get(&patterns[j].0) {
                            // Weighted merge based on sample counts
                            let total_samples =
                                entry_i.pattern.sample_count + entry_j.pattern.sample_count;
                            let weight_i =
                                entry_i.pattern.sample_count as f64 / total_samples as f64;
                            let weight_j =
                                entry_j.pattern.sample_count as f64 / total_samples as f64;

                            // Merge centroids
                            for k in 0..entry_i.pattern.centroid.len() {
                                entry_i.pattern.centroid[k] = (entry_i.pattern.centroid[k] as f64
                                    * weight_i
                                    + entry_j.pattern.centroid[k] as f64 * weight_j)
                                    as f32;
                            }

                            // Merge parameters (weighted average)
                            entry_i.pattern.optimal_ef = (entry_i.pattern.optimal_ef as f64
                                * weight_i
                                + entry_j.pattern.optimal_ef as f64 * weight_j)
                                as usize;

                            entry_i.pattern.optimal_probes = (entry_i.pattern.optimal_probes as f64
                                * weight_i
                                + entry_j.pattern.optimal_probes as f64 * weight_j)
                                as usize;

                            // Update statistics
                            entry_i.pattern.sample_count += entry_j.pattern.sample_count;
                            entry_i.pattern.avg_latency_us = entry_i.pattern.avg_latency_us
                                * weight_i
                                + entry_j.pattern.avg_latency_us * weight_j;

                            entry_i.pattern.confidence = (entry_i.pattern.confidence * weight_i
                                + entry_j.pattern.confidence * weight_j)
                                .min(1.0);

                            entry_i.usage_count += entry_j.usage_count;
                        }
                    }

                    to_remove.push(patterns[j].0);
                    merged += 1;
                }
            }
        }

        // Remove merged patterns
        for id in to_remove {
            self.patterns.remove(&id);
        }

        merged
    }

    /// Prune low-quality patterns
    pub fn prune(&self, min_usage: usize, min_confidence: f64) -> usize {
        let to_remove: Vec<usize> = self
            .patterns
            .iter()
            .filter(|entry| {
                entry.value().usage_count < min_usage
                    || entry.value().pattern.confidence < min_confidence
            })
            .map(|entry| *entry.key())
            .collect();

        let count = to_remove.len();
        for id in to_remove {
            self.patterns.remove(&id);
        }

        count
    }

    /// Get total number of patterns
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Check if bank is empty
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Get statistics
    pub fn stats(&self) -> BankStats {
        if self.patterns.is_empty() {
            return BankStats::default();
        }

        let total = self.patterns.len();
        let total_samples: usize = self
            .patterns
            .iter()
            .map(|e| e.value().pattern.sample_count)
            .sum();

        let avg_confidence: f64 = self
            .patterns
            .iter()
            .map(|e| e.value().pattern.confidence)
            .sum::<f64>()
            / total as f64;

        let total_usage: usize = self.patterns.iter().map(|e| e.value().usage_count).sum();

        BankStats {
            total_patterns: total,
            total_samples,
            avg_confidence,
            total_usage,
        }
    }

    /// Clear all patterns
    pub fn clear(&self) {
        self.patterns.clear();
        self.next_id.store(0, Ordering::SeqCst);
    }
}

impl Default for ReasoningBank {
    fn default() -> Self {
        Self::new()
    }
}

/// ReasoningBank statistics
#[derive(Debug, Clone, Default)]
pub struct BankStats {
    pub total_patterns: usize,
    pub total_samples: usize,
    pub avg_confidence: f64,
    pub total_usage: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pattern(centroid: Vec<f32>, ef: usize) -> LearnedPattern {
        LearnedPattern::new(centroid, ef, 10, 0.9, 100, 1000.0, Some(0.95))
    }

    #[test]
    fn test_store_and_lookup() {
        let bank = ReasoningBank::new();

        let pattern1 = create_test_pattern(vec![1.0, 0.0, 0.0], 50);
        let pattern2 = create_test_pattern(vec![0.0, 1.0, 0.0], 60);

        bank.store(pattern1);
        bank.store(pattern2);

        assert_eq!(bank.len(), 2);

        let query = vec![0.9, 0.1, 0.0];
        let results = bank.lookup(&query, 2);

        assert_eq!(results.len(), 2);
        assert!(results[0].2 > results[1].2); // First result more similar
    }

    #[test]
    fn test_consolidate() {
        let bank = ReasoningBank::new();

        // Store similar patterns
        let pattern1 = create_test_pattern(vec![1.0, 0.0], 50);
        let pattern2 = create_test_pattern(vec![0.99, 0.01], 50);
        let pattern3 = create_test_pattern(vec![0.0, 1.0], 60);

        bank.store(pattern1);
        bank.store(pattern2);
        bank.store(pattern3);

        assert_eq!(bank.len(), 3);

        let merged = bank.consolidate(0.95);

        assert!(merged > 0);
        assert!(bank.len() < 3);
    }

    #[test]
    fn test_prune() {
        let bank = ReasoningBank::new();

        let mut pattern_low_conf = create_test_pattern(vec![1.0, 0.0], 50);
        pattern_low_conf.confidence = 0.3;

        bank.store(pattern_low_conf);
        bank.store(create_test_pattern(vec![0.0, 1.0], 60));

        assert_eq!(bank.len(), 2);

        let pruned = bank.prune(0, 0.5);

        assert_eq!(pruned, 1);
        assert_eq!(bank.len(), 1);
    }

    #[test]
    fn test_stats() {
        let bank = ReasoningBank::new();

        bank.store(create_test_pattern(vec![1.0], 50));
        bank.store(create_test_pattern(vec![2.0], 60));

        let stats = bank.stats();

        assert_eq!(stats.total_patterns, 2);
        assert_eq!(stats.total_samples, 200);
        assert_eq!(stats.avg_confidence, 0.9);
    }
}
