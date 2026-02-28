//! Conformal Prediction for Uncertainty Quantification
//!
//! Implements conformal prediction to provide statistically valid uncertainty estimates
//! and prediction sets with guaranteed coverage (1-α).

use crate::error::{Result, RuvectorError};
use crate::types::{SearchResult, VectorId};
use serde::{Deserialize, Serialize};

/// Configuration for conformal prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformalConfig {
    /// Significance level (alpha) - typically 0.05 or 0.10
    pub alpha: f32,
    /// Size of calibration set (as fraction of total data)
    pub calibration_fraction: f32,
    /// Non-conformity measure type
    pub nonconformity_measure: NonconformityMeasure,
}

impl Default for ConformalConfig {
    fn default() -> Self {
        Self {
            alpha: 0.1, // 90% coverage
            calibration_fraction: 0.2,
            nonconformity_measure: NonconformityMeasure::Distance,
        }
    }
}

/// Type of non-conformity measure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NonconformityMeasure {
    /// Use distance score as non-conformity
    Distance,
    /// Use inverse rank as non-conformity
    InverseRank,
    /// Use normalized distance (distance / avg_distance)
    NormalizedDistance,
}

/// Prediction set with conformal guarantees
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionSet {
    /// Results in the prediction set
    pub results: Vec<SearchResult>,
    /// Conformal threshold used
    pub threshold: f32,
    /// Confidence level (1 - alpha)
    pub confidence: f32,
    /// Coverage guarantee
    pub coverage_guarantee: f32,
}

/// Conformal predictor for vector search
#[derive(Debug, Clone)]
pub struct ConformalPredictor {
    /// Configuration
    pub config: ConformalConfig,
    /// Calibration set: non-conformity scores
    pub calibration_scores: Vec<f32>,
    /// Conformal threshold (quantile of calibration scores)
    pub threshold: Option<f32>,
}

impl ConformalPredictor {
    /// Create a new conformal predictor
    pub fn new(config: ConformalConfig) -> Result<Self> {
        if !(0.0..=1.0).contains(&config.alpha) {
            return Err(RuvectorError::InvalidParameter(format!(
                "Alpha must be in [0, 1], got {}",
                config.alpha
            )));
        }

        if !(0.0..=1.0).contains(&config.calibration_fraction) {
            return Err(RuvectorError::InvalidParameter(format!(
                "Calibration fraction must be in [0, 1], got {}",
                config.calibration_fraction
            )));
        }

        Ok(Self {
            config,
            calibration_scores: Vec::new(),
            threshold: None,
        })
    }

    /// Calibrate on a set of validation examples
    ///
    /// # Arguments
    /// * `validation_queries` - Query vectors for calibration
    /// * `true_neighbors` - Ground truth neighbors for each query
    /// * `search_fn` - Function to perform search
    pub fn calibrate<F>(
        &mut self,
        validation_queries: &[Vec<f32>],
        true_neighbors: &[Vec<VectorId>],
        search_fn: F,
    ) -> Result<()>
    where
        F: Fn(&[f32], usize) -> Result<Vec<SearchResult>>,
    {
        if validation_queries.len() != true_neighbors.len() {
            return Err(RuvectorError::InvalidParameter(
                "Number of queries must match number of true neighbor sets".to_string(),
            ));
        }

        if validation_queries.is_empty() {
            return Err(RuvectorError::InvalidParameter(
                "Calibration set cannot be empty".to_string(),
            ));
        }

        let mut all_scores = Vec::new();

        // Compute non-conformity scores for calibration set
        for (query, true_ids) in validation_queries.iter().zip(true_neighbors) {
            // Search for neighbors
            let results = search_fn(query, 100)?; // Fetch more results

            // Compute non-conformity scores for true neighbors
            for true_id in true_ids {
                let score = self.compute_nonconformity_score(&results, true_id)?;
                all_scores.push(score);
            }
        }

        self.calibration_scores = all_scores;

        // Compute threshold as (1 - alpha) quantile
        self.compute_threshold()?;

        Ok(())
    }

    /// Compute conformal threshold from calibration scores
    fn compute_threshold(&mut self) -> Result<()> {
        if self.calibration_scores.is_empty() {
            return Err(RuvectorError::InvalidParameter(
                "No calibration scores available".to_string(),
            ));
        }

        let mut sorted_scores = self.calibration_scores.clone();
        sorted_scores.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Compute (1 - alpha) quantile
        let n = sorted_scores.len();
        let quantile_index = ((1.0 - self.config.alpha) * (n as f32 + 1.0)).ceil() as usize;
        let quantile_index = quantile_index.min(n - 1);

        self.threshold = Some(sorted_scores[quantile_index]);

        Ok(())
    }

    /// Compute non-conformity score for a specific result
    fn compute_nonconformity_score(
        &self,
        results: &[SearchResult],
        target_id: &VectorId,
    ) -> Result<f32> {
        match self.config.nonconformity_measure {
            NonconformityMeasure::Distance => {
                // Use distance score directly
                results
                    .iter()
                    .find(|r| &r.id == target_id)
                    .map(|r| r.score)
                    .ok_or_else(|| {
                        RuvectorError::VectorNotFound(format!(
                            "Target {} not in results",
                            target_id
                        ))
                    })
            }
            NonconformityMeasure::InverseRank => {
                // Use inverse rank: 1 / (rank + 1)
                let rank = results
                    .iter()
                    .position(|r| &r.id == target_id)
                    .ok_or_else(|| {
                        RuvectorError::VectorNotFound(format!(
                            "Target {} not in results",
                            target_id
                        ))
                    })?;
                Ok(1.0 / (rank as f32 + 1.0))
            }
            NonconformityMeasure::NormalizedDistance => {
                // Normalize by average distance
                let target_score = results
                    .iter()
                    .find(|r| &r.id == target_id)
                    .map(|r| r.score)
                    .ok_or_else(|| {
                        RuvectorError::VectorNotFound(format!(
                            "Target {} not in results",
                            target_id
                        ))
                    })?;

                // Guard against empty results
                if results.is_empty() {
                    return Ok(target_score);
                }

                let avg_score = results.iter().map(|r| r.score).sum::<f32>() / results.len() as f32;

                Ok(if avg_score > 0.0 {
                    target_score / avg_score
                } else {
                    target_score
                })
            }
        }
    }

    /// Make prediction with conformal guarantee
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `search_fn` - Function to perform search
    ///
    /// # Returns
    /// Prediction set with coverage guarantee
    pub fn predict<F>(&self, query: &[f32], search_fn: F) -> Result<PredictionSet>
    where
        F: Fn(&[f32], usize) -> Result<Vec<SearchResult>>,
    {
        let threshold = self.threshold.ok_or_else(|| {
            RuvectorError::InvalidParameter("Predictor not calibrated yet".to_string())
        })?;

        // Perform search with large k
        let results = search_fn(query, 1000)?;

        // Select results based on non-conformity threshold
        let prediction_set: Vec<SearchResult> = match self.config.nonconformity_measure {
            NonconformityMeasure::Distance => {
                // Include all results with distance <= threshold
                results
                    .into_iter()
                    .filter(|r| r.score <= threshold)
                    .collect()
            }
            NonconformityMeasure::InverseRank => {
                // Include top-k results where k is determined by threshold
                let k = (1.0 / threshold).ceil() as usize;
                results.into_iter().take(k).collect()
            }
            NonconformityMeasure::NormalizedDistance => {
                // Guard against empty results
                if results.is_empty() {
                    return Ok(PredictionSet {
                        results: vec![],
                        threshold,
                        confidence: 1.0 - self.config.alpha,
                        coverage_guarantee: 1.0 - self.config.alpha,
                    });
                }

                let avg_score = results.iter().map(|r| r.score).sum::<f32>() / results.len() as f32;
                let adjusted_threshold = threshold * avg_score;
                results
                    .into_iter()
                    .filter(|r| r.score <= adjusted_threshold)
                    .collect()
            }
        };

        Ok(PredictionSet {
            results: prediction_set,
            threshold,
            confidence: 1.0 - self.config.alpha,
            coverage_guarantee: 1.0 - self.config.alpha,
        })
    }

    /// Compute adaptive top-k based on uncertainty
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `search_fn` - Function to perform search
    ///
    /// # Returns
    /// Number of results to return based on uncertainty
    pub fn adaptive_top_k<F>(&self, query: &[f32], search_fn: F) -> Result<usize>
    where
        F: Fn(&[f32], usize) -> Result<Vec<SearchResult>>,
    {
        let prediction_set = self.predict(query, search_fn)?;
        Ok(prediction_set.results.len())
    }

    /// Get calibration statistics
    pub fn get_statistics(&self) -> Option<CalibrationStats> {
        if self.calibration_scores.is_empty() {
            return None;
        }

        let n = self.calibration_scores.len() as f32;
        let mean = self.calibration_scores.iter().sum::<f32>() / n;
        let variance = self
            .calibration_scores
            .iter()
            .map(|&s| (s - mean).powi(2))
            .sum::<f32>()
            / n;
        let std = variance.sqrt();

        let mut sorted = self.calibration_scores.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        Some(CalibrationStats {
            num_samples: self.calibration_scores.len(),
            mean,
            std,
            min: sorted.first().copied().unwrap(),
            max: sorted.last().copied().unwrap(),
            median: sorted[sorted.len() / 2],
            threshold: self.threshold,
        })
    }
}

/// Calibration statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationStats {
    /// Number of calibration samples
    pub num_samples: usize,
    /// Mean non-conformity score
    pub mean: f32,
    /// Standard deviation
    pub std: f32,
    /// Minimum score
    pub min: f32,
    /// Maximum score
    pub max: f32,
    /// Median score
    pub median: f32,
    /// Conformal threshold
    pub threshold: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_search_result(id: &str, score: f32) -> SearchResult {
        SearchResult {
            id: id.to_string(),
            score,
            vector: Some(vec![0.0; 10]),
            metadata: None,
        }
    }

    fn mock_search_fn(_query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        Ok((0..k)
            .map(|i| create_search_result(&format!("doc_{}", i), i as f32 * 0.1))
            .collect())
    }

    #[test]
    fn test_conformal_config_validation() {
        let config = ConformalConfig {
            alpha: 0.1,
            ..Default::default()
        };
        assert!(ConformalPredictor::new(config).is_ok());

        let invalid_config = ConformalConfig {
            alpha: 1.5,
            ..Default::default()
        };
        assert!(ConformalPredictor::new(invalid_config).is_err());
    }

    #[test]
    fn test_conformal_calibration() {
        let config = ConformalConfig::default();
        let mut predictor = ConformalPredictor::new(config).unwrap();

        // Create calibration data
        let queries = vec![vec![1.0; 10], vec![2.0; 10], vec![3.0; 10]];
        let true_neighbors = vec![
            vec!["doc_0".to_string(), "doc_1".to_string()],
            vec!["doc_0".to_string()],
            vec!["doc_1".to_string(), "doc_2".to_string()],
        ];

        predictor
            .calibrate(&queries, &true_neighbors, mock_search_fn)
            .unwrap();

        assert!(!predictor.calibration_scores.is_empty());
        assert!(predictor.threshold.is_some());
    }

    #[test]
    fn test_conformal_prediction() {
        let config = ConformalConfig {
            alpha: 0.1,
            calibration_fraction: 0.2,
            nonconformity_measure: NonconformityMeasure::Distance,
        };
        let mut predictor = ConformalPredictor::new(config).unwrap();

        // Calibrate
        let queries = vec![vec![1.0; 10], vec![2.0; 10]];
        let true_neighbors = vec![vec!["doc_0".to_string()], vec!["doc_1".to_string()]];

        predictor
            .calibrate(&queries, &true_neighbors, mock_search_fn)
            .unwrap();

        // Make prediction
        let query = vec![1.5; 10];
        let prediction_set = predictor.predict(&query, mock_search_fn).unwrap();

        assert!(!prediction_set.results.is_empty());
        assert_eq!(prediction_set.confidence, 0.9);
        assert!(prediction_set.threshold > 0.0);
    }

    #[test]
    fn test_nonconformity_distance() {
        let config = ConformalConfig {
            nonconformity_measure: NonconformityMeasure::Distance,
            ..Default::default()
        };
        let predictor = ConformalPredictor::new(config).unwrap();

        let results = vec![
            create_search_result("doc_0", 0.1),
            create_search_result("doc_1", 0.3),
            create_search_result("doc_2", 0.5),
        ];

        let score = predictor
            .compute_nonconformity_score(&results, &"doc_1".to_string())
            .unwrap();
        assert!((score - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_nonconformity_inverse_rank() {
        let config = ConformalConfig {
            nonconformity_measure: NonconformityMeasure::InverseRank,
            ..Default::default()
        };
        let predictor = ConformalPredictor::new(config).unwrap();

        let results = vec![
            create_search_result("doc_0", 0.1),
            create_search_result("doc_1", 0.3),
            create_search_result("doc_2", 0.5),
        ];

        let score = predictor
            .compute_nonconformity_score(&results, &"doc_1".to_string())
            .unwrap();
        assert!((score - 0.5).abs() < 0.01); // 1 / (1 + 1) = 0.5
    }

    #[test]
    fn test_calibration_stats() {
        let config = ConformalConfig::default();
        let mut predictor = ConformalPredictor::new(config).unwrap();

        predictor.calibration_scores = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        predictor.threshold = Some(0.4);

        let stats = predictor.get_statistics().unwrap();
        assert_eq!(stats.num_samples, 5);
        assert!((stats.mean - 0.3).abs() < 0.01);
        assert!((stats.min - 0.1).abs() < 0.01);
        assert!((stats.max - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_adaptive_top_k() {
        let config = ConformalConfig::default();
        let mut predictor = ConformalPredictor::new(config).unwrap();

        // Calibrate
        let queries = vec![vec![1.0; 10], vec![2.0; 10]];
        let true_neighbors = vec![vec!["doc_0".to_string()], vec!["doc_1".to_string()]];

        predictor
            .calibrate(&queries, &true_neighbors, mock_search_fn)
            .unwrap();

        // Test adaptive k
        let query = vec![1.5; 10];
        let k = predictor.adaptive_top_k(&query, mock_search_fn).unwrap();
        assert!(k > 0);
    }
}
