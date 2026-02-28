//! EWC++ Style Pattern Consolidation
//!
//! Implements Elastic Weight Consolidation Plus Plus (EWC++) techniques
//! to prevent catastrophic forgetting when learning new patterns while
//! preserving important existing knowledge.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::Pattern;

/// Configuration for pattern consolidation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationConfig {
    /// Lambda parameter for EWC regularization (higher = more protection)
    pub lambda: f32,
    /// Minimum lambda value
    pub min_lambda: f32,
    /// Maximum lambda value
    pub max_lambda: f32,
    /// Fisher information decay factor
    pub fisher_decay: f32,
    /// Minimum usage count to consider pattern important
    pub min_usage_for_importance: u32,
    /// Minimum quality to keep a pattern
    pub min_quality_threshold: f32,
    /// Similarity threshold for merging patterns
    pub merge_similarity_threshold: f32,
    /// Maximum age for unused patterns (seconds)
    pub max_unused_age_secs: u64,
    /// Enable automatic lambda adaptation
    pub auto_adapt_lambda: bool,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            lambda: 2000.0,
            min_lambda: 100.0,
            max_lambda: 15000.0,
            fisher_decay: 0.999,
            min_usage_for_importance: 3,
            min_quality_threshold: 0.3,
            merge_similarity_threshold: 0.85,
            max_unused_age_secs: 86400 * 7, // 7 days
            auto_adapt_lambda: true,
        }
    }
}

/// Fisher information for a pattern dimension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FisherInformation {
    /// Diagonal of the Fisher information matrix
    pub diagonal: Vec<f32>,
    /// Number of samples used to estimate
    pub sample_count: u64,
    /// Running EMA of squared gradients
    pub ema_grad_squared: Vec<f32>,
}

impl FisherInformation {
    /// Create new Fisher information
    pub fn new(dim: usize) -> Self {
        Self {
            diagonal: vec![1.0; dim],
            sample_count: 0,
            ema_grad_squared: vec![0.0; dim],
        }
    }

    /// Update with new gradient observation
    pub fn update(&mut self, gradient: &[f32], decay: f32) {
        if gradient.len() != self.diagonal.len() {
            return;
        }

        self.sample_count += 1;

        for (i, &g) in gradient.iter().enumerate() {
            // EMA update: F_t = decay * F_{t-1} + (1 - decay) * g^2
            self.ema_grad_squared[i] = decay * self.ema_grad_squared[i] + (1.0 - decay) * g * g;
            self.diagonal[i] = self.ema_grad_squared[i];
        }
    }

    /// Get importance score for a dimension
    pub fn importance(&self, dim: usize) -> f32 {
        if dim < self.diagonal.len() {
            self.diagonal[dim]
        } else {
            0.0
        }
    }

    /// Get total importance
    pub fn total_importance(&self) -> f32 {
        self.diagonal.iter().sum()
    }

    /// Merge with another Fisher information (weighted average)
    pub fn merge(&mut self, other: &FisherInformation, self_weight: f32) {
        if self.diagonal.len() != other.diagonal.len() {
            return;
        }

        let other_weight = 1.0 - self_weight;
        for i in 0..self.diagonal.len() {
            self.diagonal[i] = self.diagonal[i] * self_weight + other.diagonal[i] * other_weight;
            self.ema_grad_squared[i] =
                self.ema_grad_squared[i] * self_weight + other.ema_grad_squared[i] * other_weight;
        }

        self.sample_count = ((self.sample_count as f32 * self_weight)
            + (other.sample_count as f32 * other_weight)) as u64;
    }
}

/// Importance score for a pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportanceScore {
    /// Pattern ID
    pub pattern_id: u64,
    /// Overall importance score
    pub score: f32,
    /// Breakdown by factor
    pub factors: ImportanceFactors,
}

/// Factors contributing to importance
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportanceFactors {
    /// Usage-based importance
    pub usage_factor: f32,
    /// Quality-based importance
    pub quality_factor: f32,
    /// Recency-based importance
    pub recency_factor: f32,
    /// Success rate factor
    pub success_factor: f32,
    /// Fisher information factor
    pub fisher_factor: f32,
}

impl ImportanceScore {
    /// Compute importance score for a pattern
    pub fn compute(
        pattern: &Pattern,
        fisher: Option<&FisherInformation>,
        max_age_secs: u64,
    ) -> Self {
        let mut factors = ImportanceFactors::default();

        // Usage factor (log scale to avoid domination)
        factors.usage_factor = (pattern.usage_count as f32 + 1.0).ln() / 10.0;
        factors.usage_factor = factors.usage_factor.min(1.0);

        // Quality factor
        factors.quality_factor = pattern.avg_quality;

        // Recency factor (exponential decay)
        let age_secs = (chrono::Utc::now() - pattern.last_accessed).num_seconds() as f32;
        let decay_rate = -age_secs / max_age_secs as f32;
        factors.recency_factor = decay_rate.exp();

        // Success rate factor
        factors.success_factor = pattern.success_rate();

        // Fisher information factor
        if let Some(fi) = fisher {
            factors.fisher_factor = (fi.total_importance() / fi.diagonal.len() as f32).min(1.0);
        } else {
            factors.fisher_factor = 0.5; // Default if no Fisher info
        }

        // Weighted combination
        let score = 0.25 * factors.usage_factor
            + 0.25 * factors.quality_factor
            + 0.15 * factors.recency_factor
            + 0.20 * factors.success_factor
            + 0.15 * factors.fisher_factor;

        Self {
            pattern_id: pattern.id,
            score,
            factors,
        }
    }
}

/// Result of consolidation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationResult {
    /// Patterns that were merged (source IDs)
    pub merged_pattern_ids: Vec<u64>,
    /// Patterns that were pruned (removed)
    pub pruned_pattern_ids: Vec<u64>,
    /// Number of patterns before consolidation
    pub patterns_before: usize,
    /// Number of patterns after consolidation
    pub patterns_after: usize,
    /// Total importance preserved
    pub importance_preserved: f32,
    /// Consolidation timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Lambda used
    pub lambda_used: f32,
    /// Statistics
    pub stats: ConsolidationStats,
}

/// Statistics from consolidation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsolidationStats {
    /// Patterns merged
    pub merged_count: usize,
    /// Patterns pruned
    pub pruned_count: usize,
    /// Average importance of pruned patterns
    pub avg_pruned_importance: f32,
    /// Average importance of kept patterns
    pub avg_kept_importance: f32,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

/// Pattern consolidator implementing EWC++ techniques
pub struct PatternConsolidator {
    /// Configuration
    config: ConsolidationConfig,
    /// Fisher information for each pattern
    fisher_info: HashMap<u64, FisherInformation>,
    /// Current lambda value
    lambda: f32,
    /// Consolidation count
    consolidation_count: u64,
    /// Total patterns consolidated
    total_consolidated: u64,
}

impl PatternConsolidator {
    /// Create a new consolidator
    pub fn new(config: ConsolidationConfig) -> Self {
        let lambda = config.lambda;
        Self {
            config,
            fisher_info: HashMap::new(),
            lambda,
            consolidation_count: 0,
            total_consolidated: 0,
        }
    }

    /// Consolidate patterns to prevent catastrophic forgetting
    pub fn consolidate_patterns(&self, patterns: &[Pattern]) -> Result<ConsolidationResult> {
        let start = std::time::Instant::now();
        let patterns_before = patterns.len();

        // Compute importance scores
        let scores: Vec<ImportanceScore> = patterns
            .iter()
            .map(|p| {
                ImportanceScore::compute(
                    p,
                    self.fisher_info.get(&p.id),
                    self.config.max_unused_age_secs,
                )
            })
            .collect();

        // Identify patterns to prune (low importance)
        let pruned_ids: Vec<u64> = scores
            .iter()
            .filter(|s| {
                let pattern = patterns.iter().find(|p| p.id == s.pattern_id);
                if let Some(p) = pattern {
                    s.score < 0.2 && p.avg_quality < self.config.min_quality_threshold
                } else {
                    false
                }
            })
            .map(|s| s.pattern_id)
            .collect();

        // Identify patterns to merge (high similarity)
        let merged_ids = self.find_mergeable_patterns(patterns, &pruned_ids)?;

        // Compute statistics
        let pruned_importance: f32 = scores
            .iter()
            .filter(|s| pruned_ids.contains(&s.pattern_id))
            .map(|s| s.score)
            .sum();

        let kept_importance: f32 = scores
            .iter()
            .filter(|s| !pruned_ids.contains(&s.pattern_id) && !merged_ids.contains(&s.pattern_id))
            .map(|s| s.score)
            .sum();

        let patterns_after = patterns_before - pruned_ids.len() - merged_ids.len();
        let processing_time_ms = start.elapsed().as_millis() as u64;

        let stats = ConsolidationStats {
            merged_count: merged_ids.len(),
            pruned_count: pruned_ids.len(),
            avg_pruned_importance: if pruned_ids.is_empty() {
                0.0
            } else {
                pruned_importance / pruned_ids.len() as f32
            },
            avg_kept_importance: if patterns_after == 0 {
                0.0
            } else {
                kept_importance / patterns_after as f32
            },
            processing_time_ms,
        };

        Ok(ConsolidationResult {
            merged_pattern_ids: merged_ids,
            pruned_pattern_ids: pruned_ids,
            patterns_before,
            patterns_after,
            importance_preserved: kept_importance,
            timestamp: chrono::Utc::now(),
            lambda_used: self.lambda,
            stats,
        })
    }

    /// Find patterns that can be merged
    fn find_mergeable_patterns(
        &self,
        patterns: &[Pattern],
        exclude_ids: &[u64],
    ) -> Result<Vec<u64>> {
        let mut merged = Vec::new();
        let mut checked = std::collections::HashSet::new();

        for i in 0..patterns.len() {
            if exclude_ids.contains(&patterns[i].id) || checked.contains(&patterns[i].id) {
                continue;
            }

            for j in (i + 1)..patterns.len() {
                if exclude_ids.contains(&patterns[j].id)
                    || checked.contains(&patterns[j].id)
                    || merged.contains(&patterns[j].id)
                {
                    continue;
                }

                // Check same category
                if patterns[i].category != patterns[j].category {
                    continue;
                }

                // Check similarity
                let sim = patterns[i].similarity(&patterns[j].embedding);
                if sim > self.config.merge_similarity_threshold {
                    // Mark j for merging into i
                    merged.push(patterns[j].id);
                    checked.insert(patterns[j].id);
                }
            }

            checked.insert(patterns[i].id);
        }

        Ok(merged)
    }

    /// Prune low-quality patterns
    pub fn prune_low_quality(&self, patterns: &[Pattern]) -> Vec<u64> {
        patterns
            .iter()
            .filter(|p| {
                p.avg_quality < self.config.min_quality_threshold
                    && p.usage_count < self.config.min_usage_for_importance
            })
            .map(|p| p.id)
            .collect()
    }

    /// Merge similar patterns (returns the merged pattern)
    pub fn merge_patterns(&self, patterns: &[Pattern]) -> Option<Pattern> {
        if patterns.is_empty() {
            return None;
        }

        if patterns.len() == 1 {
            return Some(patterns[0].clone());
        }

        let mut merged = patterns[0].clone();
        for pattern in &patterns[1..] {
            merged.merge(pattern);
        }

        Some(merged)
    }

    /// Update Fisher information for a pattern
    pub fn update_fisher(&mut self, pattern_id: u64, gradient: &[f32]) {
        let fisher = self
            .fisher_info
            .entry(pattern_id)
            .or_insert_with(|| FisherInformation::new(gradient.len()));

        fisher.update(gradient, self.config.fisher_decay);
    }

    /// Apply EWC constraint to gradient
    pub fn apply_constraint(&self, pattern_id: u64, gradient: &[f32]) -> Vec<f32> {
        if let Some(fisher) = self.fisher_info.get(&pattern_id) {
            gradient
                .iter()
                .enumerate()
                .map(|(i, &g)| {
                    let importance = fisher.importance(i);
                    if importance > 1e-8 {
                        let penalty = self.lambda * importance;
                        g / (1.0 + penalty)
                    } else {
                        g
                    }
                })
                .collect()
        } else {
            gradient.to_vec()
        }
    }

    /// Compute EWC regularization loss
    pub fn regularization_loss(
        &self,
        pattern_id: u64,
        current_weights: &[f32],
        optimal_weights: &[f32],
    ) -> f32 {
        if current_weights.len() != optimal_weights.len() {
            return 0.0;
        }

        if let Some(fisher) = self.fisher_info.get(&pattern_id) {
            let mut loss = 0.0f32;
            for i in 0..current_weights.len().min(fisher.diagonal.len()) {
                let diff = current_weights[i] - optimal_weights[i];
                loss += fisher.diagonal[i] * diff * diff;
            }
            self.lambda * loss / 2.0
        } else {
            0.0
        }
    }

    /// Adapt lambda based on pattern statistics
    pub fn adapt_lambda(&mut self, patterns: &[Pattern]) {
        if !self.config.auto_adapt_lambda {
            return;
        }

        // Increase lambda when we have more important patterns to protect
        let important_count = patterns
            .iter()
            .filter(|p| p.usage_count >= self.config.min_usage_for_importance)
            .count();

        let scale = 1.0 + 0.1 * important_count as f32;
        self.lambda =
            (self.config.lambda * scale).clamp(self.config.min_lambda, self.config.max_lambda);
    }

    /// Consolidate all Fisher information (for memory efficiency)
    pub fn consolidate_fisher(&mut self) {
        if self.fisher_info.len() < 2 {
            return;
        }

        // Average all Fisher information
        let dim = self
            .fisher_info
            .values()
            .next()
            .map(|f| f.diagonal.len())
            .unwrap_or(0);
        if dim == 0 {
            return;
        }

        let mut consolidated = FisherInformation::new(dim);
        let count = self.fisher_info.len() as f32;

        for fisher in self.fisher_info.values() {
            for (i, &val) in fisher.diagonal.iter().enumerate() {
                if i < consolidated.diagonal.len() {
                    consolidated.diagonal[i] += val / count;
                }
            }
            for (i, &val) in fisher.ema_grad_squared.iter().enumerate() {
                if i < consolidated.ema_grad_squared.len() {
                    consolidated.ema_grad_squared[i] += val / count;
                }
            }
            consolidated.sample_count += fisher.sample_count;
        }

        // Replace with single consolidated entry (ID 0)
        self.fisher_info.clear();
        self.fisher_info.insert(0, consolidated);
    }

    /// Get current lambda
    pub fn lambda(&self) -> f32 {
        self.lambda
    }

    /// Set lambda manually
    pub fn set_lambda(&mut self, lambda: f32) {
        self.lambda = lambda.clamp(self.config.min_lambda, self.config.max_lambda);
    }

    /// Get statistics
    pub fn stats(&self) -> ConsolidatorStats {
        ConsolidatorStats {
            fisher_entries: self.fisher_info.len(),
            current_lambda: self.lambda,
            consolidation_count: self.consolidation_count,
            total_consolidated: self.total_consolidated,
        }
    }
}

/// Statistics for the consolidator
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsolidatorStats {
    /// Number of Fisher information entries
    pub fisher_entries: usize,
    /// Current lambda value
    pub current_lambda: f32,
    /// Total consolidations performed
    pub consolidation_count: u64,
    /// Total patterns consolidated
    pub total_consolidated: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning_bank::pattern_store::PatternCategory;

    fn make_pattern(id: u64, embedding: Vec<f32>, quality: f32, usage: u32) -> Pattern {
        let mut p = Pattern::new(embedding, PatternCategory::General, quality);
        p.id = id;
        p.usage_count = usage;
        p.avg_quality = quality;
        p
    }

    #[test]
    fn test_consolidation_config_default() {
        let config = ConsolidationConfig::default();
        assert_eq!(config.lambda, 2000.0);
        assert!(config.auto_adapt_lambda);
    }

    #[test]
    fn test_fisher_information() {
        let mut fisher = FisherInformation::new(4);
        assert_eq!(fisher.diagonal.len(), 4);

        let gradient = vec![0.5, 0.3, 0.2, 0.1];
        fisher.update(&gradient, 0.9);

        assert!(fisher.sample_count > 0);
        assert!(fisher.total_importance() > 0.0);
    }

    #[test]
    fn test_importance_score() {
        let pattern = make_pattern(1, vec![0.1; 4], 0.8, 10);
        let score = ImportanceScore::compute(&pattern, None, 86400);

        assert!(score.score > 0.0);
        assert!(score.score <= 1.0);
    }

    #[test]
    fn test_consolidator_creation() {
        let config = ConsolidationConfig::default();
        let consolidator = PatternConsolidator::new(config);

        assert_eq!(consolidator.lambda(), 2000.0);
    }

    #[test]
    fn test_prune_low_quality() {
        let config = ConsolidationConfig {
            min_quality_threshold: 0.5,
            min_usage_for_importance: 5,
            ..Default::default()
        };
        let consolidator = PatternConsolidator::new(config);

        let patterns = vec![
            make_pattern(1, vec![0.1; 4], 0.8, 10), // Keep (high quality)
            make_pattern(2, vec![0.2; 4], 0.3, 2),  // Prune (low quality, low usage)
            make_pattern(3, vec![0.3; 4], 0.4, 8),  // Keep (high usage)
        ];

        let pruned = consolidator.prune_low_quality(&patterns);
        assert_eq!(pruned.len(), 1);
        assert!(pruned.contains(&2));
    }

    #[test]
    fn test_consolidate_patterns() {
        let config = ConsolidationConfig::default();
        let consolidator = PatternConsolidator::new(config);

        let patterns = vec![
            make_pattern(1, vec![0.1; 4], 0.8, 10),
            make_pattern(2, vec![0.2; 4], 0.1, 1), // Low quality
            make_pattern(3, vec![0.3; 4], 0.7, 5),
        ];

        let result = consolidator.consolidate_patterns(&patterns).unwrap();

        assert_eq!(result.patterns_before, 3);
        assert!(result.patterns_after <= 3);
    }

    #[test]
    fn test_merge_similar_patterns() {
        let config = ConsolidationConfig {
            merge_similarity_threshold: 0.9,
            ..Default::default()
        };
        let consolidator = PatternConsolidator::new(config);

        // Very similar embeddings
        let patterns = vec![
            make_pattern(1, vec![1.0, 0.0, 0.0, 0.0], 0.8, 5),
            make_pattern(2, vec![0.99, 0.01, 0.0, 0.0], 0.7, 3), // Very similar to 1
            make_pattern(3, vec![0.0, 1.0, 0.0, 0.0], 0.9, 10),  // Different
        ];

        let merged = consolidator
            .find_mergeable_patterns(&patterns, &[])
            .unwrap();
        // Pattern 2 should be marked for merging into 1
        assert!(merged.contains(&2));
        assert!(!merged.contains(&1));
        assert!(!merged.contains(&3));
    }

    #[test]
    fn test_ewc_constraint() {
        let config = ConsolidationConfig::default();
        let mut consolidator = PatternConsolidator::new(config);

        // Build up Fisher information
        consolidator.update_fisher(1, &vec![1.0, 1.0, 1.0, 1.0]);
        consolidator.update_fisher(1, &vec![1.0, 1.0, 1.0, 1.0]);

        let gradient = vec![1.0, 1.0, 1.0, 1.0];
        let constrained = consolidator.apply_constraint(1, &gradient);

        // Constrained gradient should be smaller
        let orig_mag: f32 = gradient.iter().sum();
        let const_mag: f32 = constrained.iter().sum();
        assert!(const_mag <= orig_mag);
    }

    #[test]
    fn test_regularization_loss() {
        let config = ConsolidationConfig::default();
        let mut consolidator = PatternConsolidator::new(config);

        consolidator.update_fisher(1, &vec![1.0, 1.0]);

        let optimal = vec![0.0, 0.0];
        let current = vec![1.0, 1.0]; // Deviated from optimal

        let loss = consolidator.regularization_loss(1, &current, &optimal);
        assert!(loss > 0.0);

        // Loss should be zero at optimal
        let at_optimal = consolidator.regularization_loss(1, &optimal, &optimal);
        assert!(at_optimal < loss);
    }

    #[test]
    fn test_lambda_adaptation() {
        let config = ConsolidationConfig {
            lambda: 1000.0,
            min_usage_for_importance: 5,
            auto_adapt_lambda: true,
            ..Default::default()
        };
        let mut consolidator = PatternConsolidator::new(config);

        let initial_lambda = consolidator.lambda();

        // Add patterns with high usage
        let patterns = vec![
            make_pattern(1, vec![0.1; 4], 0.8, 10),
            make_pattern(2, vec![0.2; 4], 0.7, 8),
            make_pattern(3, vec![0.3; 4], 0.9, 15),
        ];

        consolidator.adapt_lambda(&patterns);

        // Lambda should increase with important patterns
        assert!(consolidator.lambda() >= initial_lambda);
    }

    #[test]
    fn test_consolidate_fisher() {
        let config = ConsolidationConfig::default();
        let mut consolidator = PatternConsolidator::new(config);

        consolidator.update_fisher(1, &vec![1.0, 0.0]);
        consolidator.update_fisher(2, &vec![0.0, 1.0]);
        consolidator.update_fisher(3, &vec![0.5, 0.5]);

        assert_eq!(consolidator.fisher_info.len(), 3);

        consolidator.consolidate_fisher();

        assert_eq!(consolidator.fisher_info.len(), 1);
    }
}
