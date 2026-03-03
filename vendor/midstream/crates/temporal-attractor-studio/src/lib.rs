//! # Temporal-Attractor-Studio
//!
//! Dynamical systems and strange attractors analysis.
//!
//! ## Features
//! - Attractor classification (point, limit cycle, strange)
//! - Lyapunov exponent calculation
//! - Phase space analysis
//! - Trajectory visualization data
//! - Stability detection

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use thiserror::Error;

/// Attractor analysis errors
#[derive(Debug, Error)]
pub enum AttractorError {
    #[error("Insufficient data: need at least {0} points")]
    InsufficientData(usize),

    #[error("Invalid dimension: {0}")]
    InvalidDimension(usize),

    #[error("Computation error: {0}")]
    ComputationError(String),
}

/// Types of attractors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttractorType {
    /// Point attractor (stable equilibrium)
    PointAttractor,
    /// Limit cycle (periodic behavior)
    LimitCycle,
    /// Strange attractor (chaotic behavior)
    StrangeAttractor,
    /// No clear attractor detected
    Unknown,
}

/// A point in phase space
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhasePoint {
    pub coordinates: Vec<f64>,
    pub timestamp: u64,
}

impl PhasePoint {
    pub fn new(coordinates: Vec<f64>, timestamp: u64) -> Self {
        Self { coordinates, timestamp }
    }

    pub fn dimension(&self) -> usize {
        self.coordinates.len()
    }
}

/// A trajectory in phase space
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub points: VecDeque<PhasePoint>,
    pub max_length: usize,
}

impl Trajectory {
    pub fn new(max_length: usize) -> Self {
        Self {
            points: VecDeque::new(),
            max_length,
        }
    }

    pub fn push(&mut self, point: PhasePoint) {
        if self.points.len() >= self.max_length {
            self.points.pop_front();
        }
        self.points.push_back(point);
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }
}

/// Information about a detected attractor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorInfo {
    pub attractor_type: AttractorType,
    pub dimension: usize,
    pub lyapunov_exponents: Vec<f64>,
    pub is_stable: bool,
    pub confidence: f64,
}

impl AttractorInfo {
    pub fn is_chaotic(&self) -> bool {
        matches!(self.attractor_type, AttractorType::StrangeAttractor)
    }

    pub fn max_lyapunov_exponent(&self) -> Option<f64> {
        self.lyapunov_exponents.iter().copied().max_by(|a, b| {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Behavior summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorSummary {
    pub total_points: usize,
    pub dimension: usize,
    pub attractor_info: Option<AttractorInfo>,
    pub mean_velocity: f64,
    pub trajectory_length: f64,
}

/// Attractor analyzer
pub struct AttractorAnalyzer {
    embedding_dimension: usize,
    min_points_for_analysis: usize,
    trajectory: Trajectory,
}

impl AttractorAnalyzer {
    /// Create a new attractor analyzer
    pub fn new(embedding_dimension: usize, max_trajectory_length: usize) -> Self {
        Self {
            embedding_dimension,
            min_points_for_analysis: 100,
            trajectory: Trajectory::new(max_trajectory_length),
        }
    }

    /// Add a point to the trajectory
    pub fn add_point(&mut self, point: PhasePoint) -> Result<(), AttractorError> {
        if point.dimension() != self.embedding_dimension {
            return Err(AttractorError::InvalidDimension(point.dimension()));
        }

        self.trajectory.push(point);
        Ok(())
    }

    /// Analyze the current trajectory
    pub fn analyze(&self) -> Result<AttractorInfo, AttractorError> {
        if self.trajectory.len() < self.min_points_for_analysis {
            return Err(AttractorError::InsufficientData(self.min_points_for_analysis));
        }

        // Calculate Lyapunov exponents
        let lyapunov_exponents = self.calculate_lyapunov_exponents()?;

        // Classify attractor type based on Lyapunov exponents
        let attractor_type = self.classify_attractor(&lyapunov_exponents);

        // Determine stability
        let is_stable = lyapunov_exponents.iter().all(|&l| l < 0.0);

        // Calculate confidence based on data quality
        let confidence = self.calculate_confidence();

        Ok(AttractorInfo {
            attractor_type,
            dimension: self.embedding_dimension,
            lyapunov_exponents,
            is_stable,
            confidence,
        })
    }

    /// Calculate Lyapunov exponents for the trajectory
    fn calculate_lyapunov_exponents(&self) -> Result<Vec<f64>, AttractorError> {
        if self.trajectory.len() < 2 {
            return Ok(vec![0.0; self.embedding_dimension]);
        }

        let mut exponents = vec![0.0; self.embedding_dimension];

        // Simplified Lyapunov calculation
        // In production, this would use more sophisticated methods
        let points: Vec<&PhasePoint> = self.trajectory.points.iter().collect();

        for dim in 0..self.embedding_dimension {
            let mut sum_log_divergence = 0.0;
            let mut count = 0;

            for i in 1..points.len() {
                let diff = points[i].coordinates[dim] - points[i-1].coordinates[dim];
                if diff.abs() > 1e-10 {
                    sum_log_divergence += diff.abs().ln();
                    count += 1;
                }
            }

            if count > 0 {
                exponents[dim] = sum_log_divergence / count as f64;
            }
        }

        Ok(exponents)
    }

    /// Classify attractor based on Lyapunov exponents
    fn classify_attractor(&self, lyapunov_exponents: &[f64]) -> AttractorType {
        let max_exponent = lyapunov_exponents.iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        if max_exponent > 0.1 {
            // Positive Lyapunov exponent indicates chaos
            AttractorType::StrangeAttractor
        } else if max_exponent > -0.1 && self.detect_periodicity() {
            // Near-zero with periodicity indicates limit cycle
            AttractorType::LimitCycle
        } else if max_exponent < -0.1 {
            // Negative Lyapunov exponent indicates stable point
            AttractorType::PointAttractor
        } else {
            AttractorType::Unknown
        }
    }

    /// Detect if trajectory shows periodic behavior
    fn detect_periodicity(&self) -> bool {
        if self.trajectory.len() < 20 {
            return false;
        }

        // Simple autocorrelation check
        let points: Vec<&PhasePoint> = self.trajectory.points.iter().collect();
        let n = points.len();

        // Check for repeating patterns
        for lag in 5..n/4 {
            let mut correlation = 0.0;
            let mut count = 0;

            for i in 0..n-lag {
                for dim in 0..self.embedding_dimension {
                    let diff = (points[i].coordinates[dim] - points[i+lag].coordinates[dim]).abs();
                    correlation += diff;
                    count += 1;
                }
            }

            let avg_diff = correlation / count as f64;
            if avg_diff < 0.1 {
                return true; // Found periodic pattern
            }
        }

        false
    }

    /// Calculate confidence in the analysis
    fn calculate_confidence(&self) -> f64 {
        let data_ratio = self.trajectory.len() as f64 / self.min_points_for_analysis as f64;
        data_ratio.min(1.0)
    }

    /// Get trajectory statistics
    pub fn get_trajectory_stats(&self) -> BehaviorSummary {
        let total_points = self.trajectory.len();

        let mut trajectory_length = 0.0;
        let mut velocity_sum = 0.0;

        let points: Vec<&PhasePoint> = self.trajectory.points.iter().collect();

        for i in 1..points.len() {
            let mut distance = 0.0;
            for dim in 0..self.embedding_dimension {
                let diff = points[i].coordinates[dim] - points[i-1].coordinates[dim];
                distance += diff * diff;
            }
            let segment_length = distance.sqrt();
            trajectory_length += segment_length;

            let time_diff = (points[i].timestamp - points[i-1].timestamp) as f64;
            if time_diff > 0.0 {
                velocity_sum += segment_length / time_diff;
            }
        }

        let mean_velocity = if points.len() > 1 {
            velocity_sum / (points.len() - 1) as f64
        } else {
            0.0
        };

        let attractor_info = if total_points >= self.min_points_for_analysis {
            self.analyze().ok()
        } else {
            None
        };

        BehaviorSummary {
            total_points,
            dimension: self.embedding_dimension,
            attractor_info,
            mean_velocity,
            trajectory_length,
        }
    }

    /// Clear the trajectory
    pub fn clear(&mut self) {
        self.trajectory.clear();
    }

    /// Get current trajectory length
    pub fn trajectory_length(&self) -> usize {
        self.trajectory.len()
    }
}

impl Default for AttractorAnalyzer {
    fn default() -> Self {
        Self::new(3, 10000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_point() {
        let point = PhasePoint::new(vec![1.0, 2.0, 3.0], 100);
        assert_eq!(point.dimension(), 3);
    }

    #[test]
    fn test_trajectory() {
        let mut traj = Trajectory::new(10);
        assert!(traj.is_empty());

        traj.push(PhasePoint::new(vec![1.0, 2.0], 1));
        assert_eq!(traj.len(), 1);

        // Fill to capacity
        for i in 2..=11 {
            traj.push(PhasePoint::new(vec![i as f64, i as f64 * 2.0], i as u64));
        }

        // Should maintain max length
        assert_eq!(traj.len(), 10);
    }

    #[test]
    fn test_attractor_analyzer() {
        let mut analyzer = AttractorAnalyzer::new(2, 1000);

        // Add some points
        for i in 0..150 {
            let point = PhasePoint::new(
                vec![i as f64, (i * 2) as f64],
                i as u64 * 1000,
            );
            analyzer.add_point(point).unwrap();
        }

        assert_eq!(analyzer.trajectory_length(), 150);

        let result = analyzer.analyze();
        assert!(result.is_ok());

        let info = result.unwrap();
        assert_eq!(info.dimension, 2);
        assert!(!info.lyapunov_exponents.is_empty());
    }

    #[test]
    fn test_invalid_dimension() {
        let mut analyzer = AttractorAnalyzer::new(3, 1000);

        let point = PhasePoint::new(vec![1.0, 2.0], 100); // Only 2D

        let result = analyzer.add_point(point);
        assert!(result.is_err());
    }

    #[test]
    fn test_insufficient_data() {
        let analyzer = AttractorAnalyzer::new(2, 1000);

        // Not enough points for analysis
        let result = analyzer.analyze();
        assert!(result.is_err());
    }

    #[test]
    fn test_behavior_summary() {
        let mut analyzer = AttractorAnalyzer::new(2, 1000);

        for i in 0..50 {
            let point = PhasePoint::new(
                vec![i as f64, i as f64],
                i as u64 * 100,
            );
            analyzer.add_point(point).unwrap();
        }

        let summary = analyzer.get_trajectory_stats();
        assert_eq!(summary.total_points, 50);
        assert_eq!(summary.dimension, 2);
        assert!(summary.trajectory_length > 0.0);
    }

    #[test]
    fn test_nan_handling_in_lyapunov_exponents() {
        // Test that NaN values don't cause panics in max_lyapunov_exponent
        let info = AttractorInfo {
            attractor_type: AttractorType::StrangeAttractor,
            dimension: 3,
            lyapunov_exponents: vec![1.0, f64::NAN, -0.5],
            is_stable: false,
            confidence: 0.95,
        };

        // Should not panic, should handle NaN gracefully
        let max_exp = info.max_lyapunov_exponent();
        assert!(max_exp.is_some());
        // With NaN handling, should return one of the valid values
        let val = max_exp.unwrap();
        assert!(val.is_finite(), "Should not return NaN");
    }

    #[test]
    fn test_nan_handling_in_trajectory() {
        // Test that NaN values in trajectory don't cause panics during analysis
        let mut analyzer = AttractorAnalyzer::new(2, 1000);

        // Add points including some with NaN to trigger edge cases
        for i in 0..150 {
            let coords = if i == 50 {
                // Insert a point that could lead to NaN in calculations
                vec![f64::NAN, i as f64]
            } else {
                vec![i as f64, (i * 2) as f64]
            };

            let point = PhasePoint::new(coords, i as u64 * 1000);
            // Should not panic when adding or analyzing
            let _ = analyzer.add_point(point);
        }

        // Analysis should complete without panicking
        let result = analyzer.analyze();
        assert!(result.is_ok(), "Analysis should handle NaN gracefully");

        let info = result.unwrap();
        // Verify the result is usable
        assert_eq!(info.dimension, 2);
    }

    #[test]
    fn test_all_nan_lyapunov_exponents() {
        // Edge case: all NaN values
        let info = AttractorInfo {
            attractor_type: AttractorType::Unknown,
            dimension: 2,
            lyapunov_exponents: vec![f64::NAN, f64::NAN],
            is_stable: false,
            confidence: 0.5,
        };

        // Should not panic even with all NaN
        let max_exp = info.max_lyapunov_exponent();
        assert!(max_exp.is_some());
    }
}
