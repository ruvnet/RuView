//! Search parameter optimization using learned patterns

use super::reasoning_bank::ReasoningBank;
use std::sync::Arc;

/// Search parameters for query execution
#[derive(Debug, Clone, Copy)]
pub struct SearchParams {
    pub ef_search: usize,
    pub probes: usize,
    pub confidence: f64,
}

impl SearchParams {
    /// Create default search parameters
    pub fn default() -> Self {
        Self {
            ef_search: 50,
            probes: 10,
            confidence: 0.0,
        }
    }

    /// Create with specific values
    pub fn new(ef_search: usize, probes: usize, confidence: f64) -> Self {
        Self {
            ef_search,
            probes,
            confidence,
        }
    }
}

/// Search optimizer using learned patterns
pub struct SearchOptimizer {
    /// ReasoningBank for pattern lookup
    bank: Arc<ReasoningBank>,
    /// Number of patterns to consider
    k_patterns: usize,
    /// Minimum confidence threshold
    min_confidence: f64,
}

impl SearchOptimizer {
    /// Create a new search optimizer
    pub fn new(bank: Arc<ReasoningBank>) -> Self {
        Self {
            bank,
            k_patterns: 5,
            min_confidence: 0.5,
        }
    }

    /// Create with custom parameters
    pub fn with_params(bank: Arc<ReasoningBank>, k_patterns: usize, min_confidence: f64) -> Self {
        Self {
            bank,
            k_patterns,
            min_confidence,
        }
    }

    /// Optimize search parameters for a query
    pub fn optimize(&self, query: &[f32]) -> SearchParams {
        // Lookup similar patterns
        let patterns = self.bank.lookup(query, self.k_patterns);

        if patterns.is_empty() {
            return SearchParams::default();
        }

        // Filter by confidence
        let valid_patterns: Vec<_> = patterns
            .iter()
            .filter(|(_, pattern, _)| pattern.confidence >= self.min_confidence)
            .collect();

        if valid_patterns.is_empty() {
            return SearchParams::default();
        }

        // Interpolate parameters based on similarity and confidence
        let mut total_weight = 0.0;
        let mut weighted_ef = 0.0;
        let mut weighted_probes = 0.0;
        let mut weighted_confidence = 0.0;

        for (_, pattern, similarity) in valid_patterns.iter() {
            // Weight combines similarity and pattern confidence
            let weight = similarity * pattern.confidence;

            weighted_ef += pattern.optimal_ef as f64 * weight;
            weighted_probes += pattern.optimal_probes as f64 * weight;
            weighted_confidence += pattern.confidence * weight;
            total_weight += weight;
        }

        if total_weight == 0.0 {
            return SearchParams::default();
        }

        SearchParams {
            ef_search: (weighted_ef / total_weight).round() as usize,
            probes: (weighted_probes / total_weight).round() as usize,
            confidence: weighted_confidence / total_weight,
        }
    }

    /// Optimize with quality target (speed vs accuracy)
    pub fn optimize_with_target(&self, query: &[f32], target: OptimizationTarget) -> SearchParams {
        let mut params = self.optimize(query);

        // Adjust based on target
        match target {
            OptimizationTarget::Speed => {
                // Reduce ef_search and probes for faster search
                params.ef_search = (params.ef_search as f64 * 0.7) as usize;
                params.probes = (params.probes as f64 * 0.7) as usize;
            }
            OptimizationTarget::Accuracy => {
                // Increase ef_search and probes for better accuracy
                params.ef_search = (params.ef_search as f64 * 1.3) as usize;
                params.probes = (params.probes as f64 * 1.3) as usize;
            }
            OptimizationTarget::Balanced => {
                // Use as-is
            }
        }

        // Enforce minimum values
        params.ef_search = params.ef_search.max(10);
        params.probes = params.probes.max(1);

        params
    }

    /// Get recommendations for a query
    pub fn recommendations(&self, query: &[f32]) -> Vec<SearchRecommendation> {
        let patterns = self.bank.lookup(query, self.k_patterns);

        patterns
            .iter()
            .filter(|(_, pattern, _)| pattern.confidence >= self.min_confidence)
            .map(|(id, pattern, similarity)| {
                let estimated_latency = pattern.avg_latency_us;
                let estimated_precision = pattern.avg_precision.unwrap_or(0.95);

                SearchRecommendation {
                    pattern_id: *id,
                    ef_search: pattern.optimal_ef,
                    probes: pattern.optimal_probes,
                    similarity: *similarity,
                    confidence: pattern.confidence,
                    estimated_latency_us: estimated_latency,
                    estimated_precision,
                }
            })
            .collect()
    }

    /// Estimate query performance
    pub fn estimate_performance(
        &self,
        query: &[f32],
        params: &SearchParams,
    ) -> PerformanceEstimate {
        let patterns = self.bank.lookup(query, self.k_patterns);

        if patterns.is_empty() {
            return PerformanceEstimate::unknown();
        }

        // Find patterns with similar parameters
        let similar_param_patterns: Vec<_> = patterns
            .iter()
            .filter(|(_, pattern, _)| {
                let ef_diff = (pattern.optimal_ef as i32 - params.ef_search as i32).abs();
                let probe_diff = (pattern.optimal_probes as i32 - params.probes as i32).abs();
                ef_diff < 20 && probe_diff < 5
            })
            .collect();

        if similar_param_patterns.is_empty() {
            return PerformanceEstimate::low_confidence();
        }

        // Weighted average of estimates
        let mut total_weight = 0.0;
        let mut weighted_latency = 0.0;
        let mut weighted_precision = 0.0;

        for (_, pattern, similarity) in similar_param_patterns.iter() {
            let weight = similarity * pattern.confidence;
            weighted_latency += pattern.avg_latency_us * weight;
            if let Some(precision) = pattern.avg_precision {
                weighted_precision += precision * weight;
            }
            total_weight += weight;
        }

        if total_weight == 0.0 {
            return PerformanceEstimate::low_confidence();
        }

        PerformanceEstimate {
            estimated_latency_us: weighted_latency / total_weight,
            estimated_precision: Some(weighted_precision / total_weight),
            confidence: total_weight / similar_param_patterns.len() as f64,
        }
    }
}

/// Optimization target
#[derive(Debug, Clone, Copy)]
pub enum OptimizationTarget {
    Speed,
    Accuracy,
    Balanced,
}

/// Search recommendation
#[derive(Debug, Clone)]
pub struct SearchRecommendation {
    pub pattern_id: usize,
    pub ef_search: usize,
    pub probes: usize,
    pub similarity: f64,
    pub confidence: f64,
    pub estimated_latency_us: f64,
    pub estimated_precision: f64,
}

/// Performance estimate
#[derive(Debug, Clone)]
pub struct PerformanceEstimate {
    pub estimated_latency_us: f64,
    pub estimated_precision: Option<f64>,
    pub confidence: f64,
}

impl PerformanceEstimate {
    fn unknown() -> Self {
        Self {
            estimated_latency_us: 0.0,
            estimated_precision: None,
            confidence: 0.0,
        }
    }

    fn low_confidence() -> Self {
        Self {
            estimated_latency_us: 1000.0,
            estimated_precision: Some(0.9),
            confidence: 0.3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::patterns::LearnedPattern;

    fn create_test_bank() -> Arc<ReasoningBank> {
        let bank = Arc::new(ReasoningBank::new());

        // Add test patterns
        let pattern1 =
            LearnedPattern::new(vec![1.0, 0.0, 0.0], 50, 10, 0.9, 100, 1000.0, Some(0.95));

        let pattern2 =
            LearnedPattern::new(vec![0.0, 1.0, 0.0], 60, 15, 0.85, 80, 1500.0, Some(0.92));

        bank.store(pattern1);
        bank.store(pattern2);

        bank
    }

    #[test]
    fn test_optimize_basic() {
        let bank = create_test_bank();
        let optimizer = SearchOptimizer::new(bank);

        let query = vec![0.9, 0.1, 0.0];
        let params = optimizer.optimize(&query);

        assert!(params.ef_search > 0);
        assert!(params.probes > 0);
        assert!(params.confidence > 0.0);
    }

    #[test]
    fn test_optimize_with_target() {
        let bank = create_test_bank();
        let optimizer = SearchOptimizer::new(bank);

        let query = vec![1.0, 0.0, 0.0];

        let speed_params = optimizer.optimize_with_target(&query, OptimizationTarget::Speed);
        let accuracy_params = optimizer.optimize_with_target(&query, OptimizationTarget::Accuracy);

        assert!(speed_params.ef_search < accuracy_params.ef_search);
        assert!(speed_params.probes <= accuracy_params.probes);
    }

    #[test]
    fn test_recommendations() {
        let bank = create_test_bank();
        let optimizer = SearchOptimizer::new(bank);

        let query = vec![1.0, 0.0, 0.0];
        let recs = optimizer.recommendations(&query);

        assert!(!recs.is_empty());
        assert!(recs[0].confidence >= 0.5);
    }

    #[test]
    fn test_performance_estimate() {
        let bank = create_test_bank();
        let optimizer = SearchOptimizer::new(bank);

        let query = vec![1.0, 0.0, 0.0];
        let params = SearchParams::new(50, 10, 0.9);

        let estimate = optimizer.estimate_performance(&query, &params);

        assert!(estimate.estimated_latency_us > 0.0);
        assert!(estimate.confidence > 0.0);
    }
}
