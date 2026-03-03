//! Behavioral analysis using temporal attractors
//!
//! Uses temporal-attractor-studio for attractor-based anomaly detection
//! with Lyapunov exponent calculations.
//!
//! Performance target: <100ms p99 (87ms baseline + 13ms overhead)

use midstreamer_attractor::{AttractorAnalyzer, AttractorInfo};
use crate::errors::{AnalysisError, AnalysisResult};
use std::sync::Arc;
use std::sync::RwLock;

/// Behavioral profile representing normal system behavior
#[derive(Debug, Clone)]
pub struct BehaviorProfile {
    /// Baseline attractors learned from normal behavior
    pub baseline_attractors: Vec<AttractorInfo>,
    /// Dimensions of state space
    pub dimensions: usize,
    /// Anomaly detection threshold
    pub threshold: f64,
}

impl Default for BehaviorProfile {
    fn default() -> Self {
        Self {
            baseline_attractors: Vec::new(),
            dimensions: 10,
            threshold: 0.75,
        }
    }
}

/// Anomaly score from behavioral analysis
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnomalyScore {
    /// Anomaly score (0.0 = normal, 1.0 = highly anomalous)
    pub score: f64,
    /// Whether this is classified as anomalous
    pub is_anomalous: bool,
    /// Confidence in the classification
    pub confidence: f64,
}

impl AnomalyScore {
    /// Create normal score
    pub fn normal() -> Self {
        Self {
            score: 0.0,
            is_anomalous: false,
            confidence: 1.0,
        }
    }

    /// Create anomalous score
    pub fn anomalous(score: f64, confidence: f64) -> Self {
        Self {
            score,
            is_anomalous: true,
            confidence,
        }
    }
}

/// Behavioral analyzer using temporal attractors
pub struct BehavioralAnalyzer {
    #[allow(dead_code)]
    analyzer: Arc<AttractorAnalyzer>,
    profile: Arc<RwLock<BehaviorProfile>>,
}

impl BehavioralAnalyzer {
    /// Create new behavioral analyzer
    pub fn new(dimensions: usize) -> AnalysisResult<Self> {
        let analyzer = AttractorAnalyzer::new(dimensions, 1000);

        let profile = BehaviorProfile {
            dimensions,
            threshold: 0.75,
            ..Default::default()
        };

        Ok(Self {
            analyzer: Arc::new(analyzer),
            profile: Arc::new(RwLock::new(profile)),
        })
    }

    /// Analyze behavior sequence for anomalies
    ///
    /// Uses temporal-attractor-studio to:
    /// 1. Calculate Lyapunov exponents
    /// 2. Identify attractors in state space
    /// 3. Compare against baseline behavior
    ///
    /// Performance: <100ms p99 (87ms baseline + overhead)
    pub async fn analyze_behavior(&self, sequence: &[f64]) -> AnalysisResult<AnomalyScore> {
        if sequence.is_empty() {
            return Err(AnalysisError::InvalidInput("Empty sequence".to_string()));
        }

        // Extract needed values before await to avoid holding lock across await
        let (dimensions, baseline_attractors, baseline_len, threshold) = {
            let profile = self.profile.read().unwrap();
            (profile.dimensions, profile.baseline_attractors.clone(), profile.baseline_attractors.len(), profile.threshold)
        };

        // Validate dimensions
        let expected_len = dimensions;
        if !sequence.len().is_multiple_of(expected_len) {
            return Err(AnalysisError::InvalidInput(
                format!("Sequence length {} not divisible by dimensions {}",
                    sequence.len(), expected_len)
            ));
        }

        // Use temporal-attractor-studio for analysis
        let attractor_result = tokio::task::spawn_blocking({
            let seq = sequence.to_vec();
            move || {
                // Create temporary analyzer for thread safety
                let mut temp_analyzer = AttractorAnalyzer::new(dimensions, 1000);

                // Add all points from sequence
                for (i, chunk) in seq.chunks(dimensions).enumerate() {
                    let point = midstreamer_attractor::PhasePoint::new(
                        chunk.to_vec(),
                        i as u64,
                    );
                    temp_analyzer.add_point(point)?;
                }

                // Analyze trajectory
                temp_analyzer.analyze()
            }
        })
        .await
        .map_err(|e| AnalysisError::Internal(e.to_string()))?
        .map_err(|e| AnalysisError::TemporalAttractor(e.to_string()))?;

        // If no baseline, this is likely training data
        if baseline_attractors.is_empty() {
            return Ok(AnomalyScore::normal());
        }

        // Calculate deviation from baseline using Lyapunov exponents
        let current_lyapunov = attractor_result.lyapunov_exponents.first().copied().unwrap_or(0.0);
        let baseline_lyapunov: f64 = baseline_attractors.iter()
            .filter_map(|a| a.lyapunov_exponents.first().copied())
            .sum::<f64>() / baseline_len as f64;

        // Calculate deviation from baseline
        let deviation = (current_lyapunov - baseline_lyapunov).abs();
        let normalized_deviation = if baseline_lyapunov.abs() > 1e-10 {
            (deviation / baseline_lyapunov.abs()).min(1.0)
        } else {
            0.0
        };

        // Determine if anomalous
        let is_anomalous = normalized_deviation > threshold;
        let confidence: f64 = if is_anomalous {
            ((normalized_deviation - threshold) / (1.0 - threshold)).clamp(0.0, 1.0)
        } else {
            (1.0 - (normalized_deviation / threshold)).clamp(0.0, 1.0)
        };

        Ok(AnomalyScore {
            score: normalized_deviation,
            is_anomalous,
            confidence,
        })
    }

    /// Train baseline behavior profile
    pub async fn train_baseline(&self, sequences: Vec<Vec<f64>>) -> AnalysisResult<()> {
        if sequences.is_empty() {
            return Err(AnalysisError::InvalidInput("No training sequences".to_string()));
        }

        let mut attractors = Vec::new();
        let dimensions = self.profile.read().unwrap().dimensions;

        for sequence in sequences {
            let result = tokio::task::spawn_blocking({
                let seq = sequence.clone();
                let dims = dimensions;
                move || {
                    let mut temp_analyzer = AttractorAnalyzer::new(dims, 1000);

                    // Add all points from sequence
                    for (i, chunk) in seq.chunks(dims).enumerate() {
                        let point = midstreamer_attractor::PhasePoint::new(
                            chunk.to_vec(),
                            i as u64,
                        );
                        temp_analyzer.add_point(point)?;
                    }

                    // Analyze trajectory
                    temp_analyzer.analyze()
                }
            })
            .await
            .map_err(|e| AnalysisError::Internal(e.to_string()))?
            .map_err(|e| AnalysisError::TemporalAttractor(e.to_string()))?;

            attractors.push(result);
        }

        let mut profile = self.profile.write().unwrap();
        profile.baseline_attractors = attractors;

        tracing::info!("Trained baseline with {} attractors", profile.baseline_attractors.len());

        Ok(())
    }

    /// Check if score indicates anomaly
    pub fn is_anomalous(&self, score: &AnomalyScore) -> bool {
        score.is_anomalous
    }

    /// Update anomaly detection threshold
    pub fn set_threshold(&self, threshold: f64) {
        let mut profile = self.profile.write().unwrap();
        profile.threshold = threshold.clamp(0.0, 1.0);
    }

    /// Get current threshold
    pub fn threshold(&self) -> f64 {
        self.profile.read().unwrap().threshold
    }

    /// Get number of baseline attractors
    pub fn baseline_count(&self) -> usize {
        self.profile.read().unwrap().baseline_attractors.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_analyzer_creation() {
        let analyzer = BehavioralAnalyzer::new(10).unwrap();
        assert_eq!(analyzer.threshold(), 0.75);
        assert_eq!(analyzer.baseline_count(), 0);
    }

    #[tokio::test]
    async fn test_empty_sequence() {
        let analyzer = BehavioralAnalyzer::new(10).unwrap();
        let result = analyzer.analyze_behavior(&[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_dimensions() {
        let analyzer = BehavioralAnalyzer::new(10).unwrap();
        let sequence = vec![1.0; 15]; // Not divisible by 10
        let result = analyzer.analyze_behavior(&sequence).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_normal_behavior_without_baseline() {
        let analyzer = BehavioralAnalyzer::new(10).unwrap();
        let sequence = vec![0.5; 1000]; // 10 dimensions * 100 points (minimum required)
        let score = analyzer.analyze_behavior(&sequence).await.unwrap();
        assert!(!score.is_anomalous);
    }

    #[tokio::test]
    async fn test_threshold_update() {
        let analyzer = BehavioralAnalyzer::new(10).unwrap();
        analyzer.set_threshold(0.9);
        assert!((analyzer.threshold() - 0.9).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_anomaly_score_helpers() {
        let normal = AnomalyScore::normal();
        assert!(!normal.is_anomalous);
        assert_eq!(normal.score, 0.0);

        let anomalous = AnomalyScore::anomalous(0.9, 0.95);
        assert!(anomalous.is_anomalous);
        assert_eq!(anomalous.score, 0.9);
    }
}
