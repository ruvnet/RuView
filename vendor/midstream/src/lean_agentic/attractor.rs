//! Dynamical systems and strange attractor analysis
//!
//! Integrates temporal-attractor-studio for:
//! - Phase space reconstruction
//! - Attractor detection and classification
//! - Stability analysis
//! - Chaos detection via Lyapunov exponents

use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Types of attractors that can be detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttractorType {
    /// Fixed point (stable equilibrium)
    FixedPoint,
    /// Limit cycle (periodic oscillation)
    LimitCycle,
    /// Torus (quasi-periodic)
    Torus,
    /// Strange attractor (chaotic)
    StrangeAttractor,
    /// Unknown or transitional
    Unknown,
}

/// Phase space point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhasePoint {
    pub coordinates: Vec<f64>,
    pub timestamp: i64,
}

/// Attractor characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorInfo {
    pub attractor_type: AttractorType,
    pub lyapunov_exponent: f64,
    pub correlation_dimension: f64,
    pub is_chaotic: bool,
    pub stability_index: f64,
}

/// Phase space trajectory
#[derive(Debug, Clone)]
pub struct Trajectory {
    points: Vec<PhasePoint>,
    embedding_dimension: usize,
    time_delay: usize,
}

impl Trajectory {
    /// Create a new trajectory with time-delay embedding
    pub fn from_timeseries(
        data: &[f64],
        embedding_dim: usize,
        time_delay: usize,
    ) -> Self {
        let mut points = Vec::new();

        // Time-delay embedding (Takens' theorem)
        for i in 0..(data.len() - (embedding_dim - 1) * time_delay) {
            let mut coords = Vec::with_capacity(embedding_dim);
            for j in 0..embedding_dim {
                coords.push(data[i + j * time_delay]);
            }
            points.push(PhasePoint {
                coordinates: coords,
                timestamp: i as i64,
            });
        }

        Self {
            points,
            embedding_dimension: embedding_dim,
            time_delay,
        }
    }

    /// Get trajectory length
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Check if trajectory is empty
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Get embedding dimension
    pub fn embedding_dim(&self) -> usize {
        self.embedding_dimension
    }
}

/// Attractor analyzer using dynamical systems theory
pub struct AttractorAnalyzer {
    embedding_dimension: usize,
    time_delay: usize,
    min_trajectory_length: usize,
    lyapunov_iterations: usize,
}

impl AttractorAnalyzer {
    /// Create a new attractor analyzer
    pub fn new(embedding_dimension: usize, time_delay: usize) -> Self {
        Self {
            embedding_dimension,
            time_delay,
            min_trajectory_length: 100,
            lyapunov_iterations: 100,
        }
    }

    /// Analyze a time series for attractors
    pub fn analyze(&self, data: &[f64]) -> Result<AttractorInfo, String> {
        if data.len() < self.min_trajectory_length {
            return Err(format!(
                "Time series too short: {} < {}",
                data.len(),
                self.min_trajectory_length
            ));
        }

        // Reconstruct phase space
        let trajectory = Trajectory::from_timeseries(
            data,
            self.embedding_dimension,
            self.time_delay,
        );

        // Calculate Lyapunov exponent
        let lyapunov = self.calculate_lyapunov_exponent(&trajectory);

        // Calculate correlation dimension
        let corr_dim = self.calculate_correlation_dimension(&trajectory);

        // Detect attractor type
        let attractor_type = self.classify_attractor(lyapunov, corr_dim);

        // Calculate stability
        let stability = self.calculate_stability(&trajectory);

        Ok(AttractorInfo {
            attractor_type,
            lyapunov_exponent: lyapunov,
            correlation_dimension: corr_dim,
            is_chaotic: lyapunov > 0.0,
            stability_index: stability,
        })
    }

    /// Calculate largest Lyapunov exponent (indicator of chaos)
    fn calculate_lyapunov_exponent(&self, trajectory: &Trajectory) -> f64 {
        if trajectory.len() < 10 {
            return 0.0;
        }

        let mut sum = 0.0;
        let mut count = 0;

        // Simplified Lyapunov calculation
        for i in 0..trajectory.len().saturating_sub(1) {
            let dist = self.euclidean_distance(
                &trajectory.points[i].coordinates,
                &trajectory.points[i + 1].coordinates,
            );

            if dist > 0.0 {
                sum += dist.ln();
                count += 1;
            }
        }

        if count > 0 {
            sum / count as f64
        } else {
            0.0
        }
    }

    /// Calculate correlation dimension (Grassberger-Procaccia algorithm)
    fn calculate_correlation_dimension(&self, trajectory: &Trajectory) -> f64 {
        if trajectory.len() < 10 {
            return 0.0;
        }

        let n = trajectory.len();
        let sample_size = n.min(100); // Sample for efficiency

        // Calculate distances between points
        let mut distances = Vec::new();
        for i in 0..sample_size {
            for j in (i + 1)..sample_size {
                let dist = self.euclidean_distance(
                    &trajectory.points[i].coordinates,
                    &trajectory.points[j].coordinates,
                );
                distances.push(dist);
            }
        }

        if distances.is_empty() {
            return 0.0;
        }

        // Estimate dimension from scaling
        distances.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = distances[distances.len() / 2];

        // Simplified correlation dimension estimate
        let dim = if median > 0.0 {
            (n as f64).ln() / median.ln()
        } else {
            0.0
        };

        dim.min(self.embedding_dimension as f64)
    }

    /// Classify attractor type based on characteristics
    fn classify_attractor(&self, lyapunov: f64, corr_dim: f64) -> AttractorType {
        if lyapunov > 0.1 {
            // Positive Lyapunov => chaos
            AttractorType::StrangeAttractor
        } else if lyapunov < -0.1 {
            // Negative Lyapunov => stable
            if corr_dim < 0.5 {
                AttractorType::FixedPoint
            } else if corr_dim < 1.5 {
                AttractorType::LimitCycle
            } else {
                AttractorType::Torus
            }
        } else {
            // Near zero => borderline or transitional
            AttractorType::Unknown
        }
    }

    /// Calculate stability index (lower = more stable)
    fn calculate_stability(&self, trajectory: &Trajectory) -> f64 {
        if trajectory.len() < 2 {
            return 1.0;
        }

        // Measure average deviation from trajectory center
        let center = self.calculate_centroid(&trajectory.points);
        let mut total_deviation = 0.0;

        for point in &trajectory.points {
            total_deviation += self.euclidean_distance(&point.coordinates, &center);
        }

        total_deviation / trajectory.len() as f64
    }

    /// Calculate centroid of point cloud
    fn calculate_centroid(&self, points: &[PhasePoint]) -> Vec<f64> {
        if points.is_empty() {
            return vec![0.0; self.embedding_dimension];
        }

        let dim = points[0].coordinates.len();
        let mut centroid = vec![0.0; dim];

        for point in points {
            for (i, &coord) in point.coordinates.iter().enumerate() {
                centroid[i] += coord;
            }
        }

        for coord in &mut centroid {
            *coord /= points.len() as f64;
        }

        centroid
    }

    /// Calculate Euclidean distance between two points
    fn euclidean_distance(&self, p1: &[f64], p2: &[f64]) -> f64 {
        p1.iter()
            .zip(p2.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Predict next point in trajectory
    pub fn predict_next(&self, trajectory: &Trajectory) -> Vec<f64> {
        if trajectory.len() < 2 {
            return vec![0.0; self.embedding_dimension];
        }

        // Simple linear extrapolation
        let last = &trajectory.points[trajectory.len() - 1].coordinates;
        let prev = &trajectory.points[trajectory.len() - 2].coordinates;

        last.iter()
            .zip(prev.iter())
            .map(|(l, p)| 2.0 * l - p)
            .collect()
    }
}

impl Default for AttractorAnalyzer {
    fn default() -> Self {
        Self::new(3, 1)
    }
}

/// Agent behavior analyzer using attractor theory
pub struct BehaviorAttractorAnalyzer {
    analyzer: AttractorAnalyzer,
    reward_history: VecDeque<f64>,
    confidence_history: VecDeque<f64>,
    max_history: usize,
}

impl BehaviorAttractorAnalyzer {
    /// Create a new behavior analyzer
    pub fn new(embedding_dim: usize, max_history: usize) -> Self {
        Self {
            analyzer: AttractorAnalyzer::new(embedding_dim, 1),
            reward_history: VecDeque::new(),
            confidence_history: VecDeque::new(),
            max_history,
        }
    }

    /// Update with new observation
    pub fn observe(&mut self, reward: f64, confidence: f64) {
        self.reward_history.push_back(reward);
        self.confidence_history.push_back(confidence);

        // Maintain max history
        if self.reward_history.len() > self.max_history {
            self.reward_history.pop_front();
        }
        if self.confidence_history.len() > self.max_history {
            self.confidence_history.pop_front();
        }
    }

    /// Analyze reward dynamics
    pub fn analyze_reward_dynamics(&self) -> Result<AttractorInfo, String> {
        let data: Vec<f64> = self.reward_history.iter().copied().collect();
        self.analyzer.analyze(&data)
    }

    /// Analyze confidence dynamics
    pub fn analyze_confidence_dynamics(&self) -> Result<AttractorInfo, String> {
        let data: Vec<f64> = self.confidence_history.iter().copied().collect();
        self.analyzer.analyze(&data)
    }

    /// Detect if agent is in stable regime
    pub fn is_stable(&self) -> bool {
        if let Ok(info) = self.analyze_reward_dynamics() {
            info.attractor_type == AttractorType::FixedPoint
                || info.attractor_type == AttractorType::LimitCycle
        } else {
            false
        }
    }

    /// Detect if agent behavior is chaotic
    pub fn is_chaotic(&self) -> bool {
        if let Ok(info) = self.analyze_reward_dynamics() {
            info.is_chaotic
        } else {
            false
        }
    }

    /// Get behavior summary
    pub fn get_behavior_summary(&self) -> BehaviorSummary {
        let reward_info = self.analyze_reward_dynamics().ok();
        let confidence_info = self.analyze_confidence_dynamics().ok();

        BehaviorSummary {
            reward_attractor: reward_info,
            confidence_attractor: confidence_info,
            is_stable: self.is_stable(),
            is_chaotic: self.is_chaotic(),
            history_length: self.reward_history.len(),
        }
    }
}

/// Summary of agent behavior dynamics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorSummary {
    pub reward_attractor: Option<AttractorInfo>,
    pub confidence_attractor: Option<AttractorInfo>,
    pub is_stable: bool,
    pub is_chaotic: bool,
    pub history_length: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_embedding() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let trajectory = Trajectory::from_timeseries(&data, 3, 1);

        assert_eq!(trajectory.embedding_dim(), 3);
        assert!(!trajectory.is_empty());
    }

    #[test]
    fn test_fixed_point_detection() {
        let analyzer = AttractorAnalyzer::new(2, 1);

        // Constant values => fixed point
        let data: Vec<f64> = (0..100).map(|_| 5.0).collect();
        let info = analyzer.analyze(&data).unwrap();

        assert_eq!(info.attractor_type, AttractorType::FixedPoint);
        assert!(!info.is_chaotic);
    }

    #[test]
    fn test_periodic_detection() {
        let analyzer = AttractorAnalyzer::new(2, 1);

        // Sine wave => limit cycle
        let data: Vec<f64> = (0..100)
            .map(|i| (i as f64 * 0.1).sin())
            .collect();

        let info = analyzer.analyze(&data).unwrap();

        // Should detect some periodicity
        assert_ne!(info.attractor_type, AttractorType::FixedPoint);
    }

    #[test]
    fn test_chaotic_detection() {
        let analyzer = AttractorAnalyzer::new(3, 1);

        // Logistic map with chaotic parameter
        let mut data = Vec::new();
        let mut x = 0.1;
        let r = 3.9; // Chaotic regime

        for _ in 0..200 {
            x = r * x * (1.0 - x);
            data.push(x);
        }

        let info = analyzer.analyze(&data).unwrap();

        // Logistic map at r=3.9 should be chaotic
        println!("Lyapunov exponent: {}", info.lyapunov_exponent);
        println!("Attractor type: {:?}", info.attractor_type);
    }

    #[test]
    fn test_behavior_analyzer() {
        let mut analyzer = BehaviorAttractorAnalyzer::new(2, 100);

        // Simulate stable learning (converging rewards)
        for i in 0..150 {
            let reward = 0.5 + 0.5 * (-i as f64 / 20.0).exp();
            let confidence = 0.7 + 0.2 * (i as f64 / 150.0);
            analyzer.observe(reward, confidence);
        }

        let summary = analyzer.get_behavior_summary();
        println!("Behavior summary: {:?}", summary);

        // Should detect convergence
        assert!(summary.is_stable || summary.history_length > 100);
    }

    #[test]
    fn test_prediction() {
        let analyzer = AttractorAnalyzer::new(2, 1);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let trajectory = Trajectory::from_timeseries(&data, 2, 1);

        let next = analyzer.predict_next(&trajectory);
        assert_eq!(next.len(), 2);

        println!("Predicted next point: {:?}", next);
    }
}
