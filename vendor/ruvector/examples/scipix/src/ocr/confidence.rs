//! Confidence Scoring Module
//!
//! This module provides confidence scoring and calibration for OCR results.
//! It includes per-character confidence calculation and aggregation methods.

use super::Result;
use std::collections::HashMap;
use tracing::debug;

/// Calculate confidence score for a single character prediction
///
/// # Arguments
/// * `logits` - Raw logits from the model for this character position
///
/// # Returns
/// Confidence score between 0.0 and 1.0
pub fn calculate_confidence(logits: &[f32]) -> f32 {
    if logits.is_empty() {
        return 0.0;
    }

    // Apply softmax to get probabilities
    let max_logit = logits.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let exp_sum: f32 = logits.iter().map(|&x| (x - max_logit).exp()).sum();

    // Return the maximum probability
    let max_prob = logits
        .iter()
        .map(|&x| (x - max_logit).exp() / exp_sum)
        .fold(0.0f32, |a, b| a.max(b));

    max_prob.clamp(0.0, 1.0)
}

/// Aggregate multiple confidence scores into a single score
///
/// # Arguments
/// * `confidences` - Individual confidence scores
///
/// # Returns
/// Aggregated confidence score using geometric mean
pub fn aggregate_confidence(confidences: &[f32]) -> f32 {
    if confidences.is_empty() {
        return 0.0;
    }

    // Use geometric mean for aggregation (more conservative than arithmetic mean)
    let product: f32 = confidences.iter().product();
    let n = confidences.len() as f32;
    product.powf(1.0 / n).clamp(0.0, 1.0)
}

/// Alternative aggregation using arithmetic mean
pub fn aggregate_confidence_mean(confidences: &[f32]) -> f32 {
    if confidences.is_empty() {
        return 0.0;
    }

    let sum: f32 = confidences.iter().sum();
    (sum / confidences.len() as f32).clamp(0.0, 1.0)
}

/// Alternative aggregation using minimum (most conservative)
pub fn aggregate_confidence_min(confidences: &[f32]) -> f32 {
    confidences
        .iter()
        .fold(1.0f32, |a, &b| a.min(b))
        .clamp(0.0, 1.0)
}

/// Alternative aggregation using harmonic mean
pub fn aggregate_confidence_harmonic(confidences: &[f32]) -> f32 {
    if confidences.is_empty() {
        return 0.0;
    }

    let sum_reciprocals: f32 = confidences.iter().map(|&c| 1.0 / c.max(0.001)).sum();
    let n = confidences.len() as f32;
    (n / sum_reciprocals).clamp(0.0, 1.0)
}

/// Confidence calibrator using isotonic regression
///
/// This calibrator learns a mapping from raw confidence scores to calibrated
/// probabilities using historical data.
pub struct ConfidenceCalibrator {
    /// Calibration mapping: raw_score -> calibrated_score
    calibration_map: HashMap<u8, f32>, // Use u8 for binned scores (0-100)
    /// Whether the calibrator has been trained
    is_trained: bool,
}

impl ConfidenceCalibrator {
    /// Create a new, untrained calibrator
    pub fn new() -> Self {
        Self {
            calibration_map: HashMap::new(),
            is_trained: false,
        }
    }

    /// Train the calibrator on labeled data
    ///
    /// # Arguments
    /// * `predictions` - Raw confidence scores from the model
    /// * `ground_truth` - Binary labels (1.0 if correct, 0.0 if incorrect)
    pub fn train(&mut self, predictions: &[f32], ground_truth: &[f32]) -> Result<()> {
        debug!(
            "Training confidence calibrator on {} samples",
            predictions.len()
        );

        if predictions.len() != ground_truth.len() {
            return Err(super::OcrError::InvalidConfig(
                "Predictions and ground truth must have same length".to_string(),
            ));
        }

        if predictions.is_empty() {
            return Err(super::OcrError::InvalidConfig(
                "Cannot train on empty data".to_string(),
            ));
        }

        // Bin the scores (0.0-1.0 -> 0-100)
        let mut bins: HashMap<u8, Vec<f32>> = HashMap::new();

        for (&pred, &truth) in predictions.iter().zip(ground_truth.iter()) {
            let bin = (pred * 100.0).clamp(0.0, 100.0) as u8;
            bins.entry(bin).or_insert_with(Vec::new).push(truth);
        }

        // Calculate mean accuracy for each bin
        self.calibration_map.clear();
        for (bin, truths) in bins {
            let mean_accuracy = truths.iter().sum::<f32>() / truths.len() as f32;
            self.calibration_map.insert(bin, mean_accuracy);
        }

        // Perform isotonic regression (simplified version)
        self.enforce_monotonicity();

        self.is_trained = true;
        debug!(
            "Calibrator trained with {} bins",
            self.calibration_map.len()
        );

        Ok(())
    }

    /// Enforce monotonicity constraint (isotonic regression)
    fn enforce_monotonicity(&mut self) {
        let mut sorted_bins: Vec<_> = self.calibration_map.iter().collect();
        sorted_bins.sort_by_key(|(bin, _)| *bin);

        // Simple isotonic regression: ensure calibrated scores are non-decreasing
        let mut adjusted = HashMap::new();
        let mut prev_value = 0.0;

        for (&bin, &value) in sorted_bins {
            let adjusted_value = value.max(prev_value);
            adjusted.insert(bin, adjusted_value);
            prev_value = adjusted_value;
        }

        self.calibration_map = adjusted;
    }

    /// Calibrate a raw confidence score
    pub fn calibrate(&self, raw_score: f32) -> f32 {
        if !self.is_trained {
            // If not trained, return raw score
            return raw_score.clamp(0.0, 1.0);
        }

        let bin = (raw_score * 100.0).clamp(0.0, 100.0) as u8;

        // Look up calibrated score, or interpolate
        if let Some(&calibrated) = self.calibration_map.get(&bin) {
            return calibrated;
        }

        // Interpolate between nearest bins
        self.interpolate(bin)
    }

    /// Interpolate calibrated score for a bin without direct mapping
    fn interpolate(&self, target_bin: u8) -> f32 {
        let mut lower = None;
        let mut upper = None;

        for &bin in self.calibration_map.keys() {
            if bin < target_bin {
                lower = Some(lower.map_or(bin, |l: u8| l.max(bin)));
            } else if bin > target_bin {
                upper = Some(upper.map_or(bin, |u: u8| u.min(bin)));
            }
        }

        match (lower, upper) {
            (Some(l), Some(u)) => {
                let l_val = self.calibration_map[&l];
                let u_val = self.calibration_map[&u];
                let alpha = (target_bin - l) as f32 / (u - l) as f32;
                l_val + alpha * (u_val - l_val)
            }
            (Some(l), None) => self.calibration_map[&l],
            (None, Some(u)) => self.calibration_map[&u],
            (None, None) => target_bin as f32 / 100.0, // Fallback
        }
    }

    /// Check if the calibrator is trained
    pub fn is_trained(&self) -> bool {
        self.is_trained
    }

    /// Reset the calibrator
    pub fn reset(&mut self) {
        self.calibration_map.clear();
        self.is_trained = false;
    }
}

impl Default for ConfidenceCalibrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate Expected Calibration Error (ECE)
///
/// Measures the difference between predicted confidence and actual accuracy
pub fn calculate_ece(predictions: &[f32], ground_truth: &[f32], n_bins: usize) -> f32 {
    if predictions.len() != ground_truth.len() || predictions.is_empty() {
        return 0.0;
    }

    let mut bins: Vec<Vec<(f32, f32)>> = vec![Vec::new(); n_bins];

    // Assign predictions to bins
    for (&pred, &truth) in predictions.iter().zip(ground_truth.iter()) {
        let bin_idx = ((pred * n_bins as f32) as usize).min(n_bins - 1);
        bins[bin_idx].push((pred, truth));
    }

    // Calculate ECE
    let mut ece = 0.0;
    let total = predictions.len() as f32;

    for bin in bins {
        if bin.is_empty() {
            continue;
        }

        let bin_size = bin.len() as f32;
        let avg_confidence: f32 = bin.iter().map(|(p, _)| p).sum::<f32>() / bin_size;
        let avg_accuracy: f32 = bin.iter().map(|(_, t)| t).sum::<f32>() / bin_size;

        ece += (bin_size / total) * (avg_confidence - avg_accuracy).abs();
    }

    ece
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_confidence() {
        let logits = vec![1.0, 5.0, 2.0, 1.0];
        let conf = calculate_confidence(&logits);
        assert!(conf > 0.5);
        assert!(conf <= 1.0);
    }

    #[test]
    fn test_calculate_confidence_empty() {
        let logits: Vec<f32> = vec![];
        let conf = calculate_confidence(&logits);
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn test_aggregate_confidence() {
        let confidences = vec![0.9, 0.8, 0.95, 0.85];
        let agg = aggregate_confidence(&confidences);
        assert!(agg > 0.0 && agg <= 1.0);
        assert!(agg < 0.9); // Geometric mean should be less than max
    }

    #[test]
    fn test_aggregate_confidence_mean() {
        let confidences = vec![0.8, 0.9, 0.7];
        let mean = aggregate_confidence_mean(&confidences);
        assert_eq!(mean, 0.8); // (0.8 + 0.9 + 0.7) / 3
    }

    #[test]
    fn test_aggregate_confidence_min() {
        let confidences = vec![0.9, 0.7, 0.95];
        let min = aggregate_confidence_min(&confidences);
        assert_eq!(min, 0.7);
    }

    #[test]
    fn test_aggregate_confidence_harmonic() {
        let confidences = vec![0.5, 0.5];
        let harmonic = aggregate_confidence_harmonic(&confidences);
        assert_eq!(harmonic, 0.5);
    }

    #[test]
    fn test_calibrator_training() {
        let mut calibrator = ConfidenceCalibrator::new();
        assert!(!calibrator.is_trained());

        let predictions = vec![0.9, 0.8, 0.7, 0.6, 0.5];
        let ground_truth = vec![1.0, 1.0, 0.0, 1.0, 0.0];

        let result = calibrator.train(&predictions, &ground_truth);
        assert!(result.is_ok());
        assert!(calibrator.is_trained());
    }

    #[test]
    fn test_calibrator_calibrate() {
        let mut calibrator = ConfidenceCalibrator::new();

        // Before training, should return raw score
        assert_eq!(calibrator.calibrate(0.8), 0.8);

        // Train with some data
        let predictions = vec![0.9, 0.9, 0.8, 0.8, 0.7, 0.7];
        let ground_truth = vec![1.0, 1.0, 1.0, 0.0, 0.0, 0.0];
        calibrator.train(&predictions, &ground_truth).unwrap();

        // After training, should return calibrated score
        let calibrated = calibrator.calibrate(0.85);
        assert!(calibrated >= 0.0 && calibrated <= 1.0);
    }

    #[test]
    fn test_calibrator_reset() {
        let mut calibrator = ConfidenceCalibrator::new();
        let predictions = vec![0.9, 0.8];
        let ground_truth = vec![1.0, 0.0];

        calibrator.train(&predictions, &ground_truth).unwrap();
        assert!(calibrator.is_trained());

        calibrator.reset();
        assert!(!calibrator.is_trained());
    }

    #[test]
    fn test_calculate_ece() {
        let predictions = vec![0.9, 0.7, 0.6, 0.8];
        let ground_truth = vec![1.0, 1.0, 0.0, 1.0];
        let ece = calculate_ece(&predictions, &ground_truth, 3);
        assert!(ece >= 0.0 && ece <= 1.0);
    }

    #[test]
    fn test_calibrator_monotonicity() {
        let mut calibrator = ConfidenceCalibrator::new();

        // Create data that would violate monotonicity
        let predictions = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9];
        let ground_truth = vec![0.2, 0.3, 0.2, 0.5, 0.4, 0.7, 0.8, 0.9, 1.0];

        calibrator.train(&predictions, &ground_truth).unwrap();

        // Check monotonicity
        let score1 = calibrator.calibrate(0.3);
        let score2 = calibrator.calibrate(0.5);
        let score3 = calibrator.calibrate(0.7);

        assert!(score2 >= score1, "Calibrated scores should be monotonic");
        assert!(score3 >= score2, "Calibrated scores should be monotonic");
    }
}
