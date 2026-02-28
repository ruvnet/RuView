//! Coherence Collapse Prediction
//!
//! This module provides early warning systems for detecting when a graph's
//! structural coherence is degrading, potentially leading to "collapse" where
//! the graph loses its essential connectivity or community structure.
//!
//! ## Use Cases
//!
//! - **Multi-agent systems**: Detect when agent coordination is breaking down
//! - **Social networks**: Identify community fragmentation
//! - **Neural networks**: Monitor layer coherence during training
//! - **Knowledge graphs**: Track semantic drift
//!
//! ## Theoretical Foundation
//!
//! The predictor monitors several spectral invariants:
//! - Algebraic connectivity (Fiedler value)
//! - Spectral gap stability
//! - Cheeger constant changes
//! - Eigenvalue distribution entropy

use super::analyzer::SpectralAnalyzer;
use super::cheeger::{CheegerAnalyzer, CheegerBounds};
use super::types::{Graph, SpectralGap, Vector, EPS};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Warning levels for collapse prediction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarningLevel {
    /// No warning - system is stable
    None,
    /// Minor fluctuations detected
    Low,
    /// Significant changes in spectral properties
    Medium,
    /// Rapid degradation - intervention recommended
    High,
    /// Imminent collapse - immediate action required
    Critical,
}

impl WarningLevel {
    /// Convert to numeric severity (0-4)
    pub fn severity(&self) -> u8 {
        match self {
            WarningLevel::None => 0,
            WarningLevel::Low => 1,
            WarningLevel::Medium => 2,
            WarningLevel::High => 3,
            WarningLevel::Critical => 4,
        }
    }

    /// Create from numeric severity
    pub fn from_severity(s: u8) -> Self {
        match s {
            0 => WarningLevel::None,
            1 => WarningLevel::Low,
            2 => WarningLevel::Medium,
            3 => WarningLevel::High,
            _ => WarningLevel::Critical,
        }
    }
}

/// Warning signal with details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warning {
    /// Warning level
    pub level: WarningLevel,
    /// Description of the warning
    pub message: String,
    /// Specific metric that triggered the warning
    pub metric: String,
    /// Current value of the metric
    pub current_value: f64,
    /// Expected/threshold value
    pub threshold: f64,
    /// Rate of change (if applicable)
    pub rate_of_change: Option<f64>,
    /// Recommended actions
    pub recommendations: Vec<String>,
}

/// Snapshot of spectral properties at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralSnapshot {
    /// Timestamp or sequence number
    pub timestamp: u64,
    /// Algebraic connectivity (Fiedler value)
    pub algebraic_connectivity: f64,
    /// Spectral gap
    pub spectral_gap: SpectralGap,
    /// Cheeger bounds
    pub cheeger_bounds: CheegerBounds,
    /// First k eigenvalues
    pub eigenvalues: Vec<f64>,
    /// Number of near-zero eigenvalues (indicating components)
    pub near_zero_count: usize,
    /// Eigenvalue entropy (distribution uniformity)
    pub eigenvalue_entropy: f64,
    /// Graph statistics
    pub num_nodes: usize,
    pub num_edges: usize,
    pub total_weight: f64,
}

/// Collapse prediction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollapsePrediction {
    /// Overall collapse risk score (0-1, higher = more risk)
    pub risk_score: f64,
    /// Current warning level
    pub warning_level: WarningLevel,
    /// Detailed warnings
    pub warnings: Vec<Warning>,
    /// Estimated time to collapse (in timesteps, if predictable)
    pub estimated_collapse_time: Option<u64>,
    /// Components at risk of disconnection
    pub fragile_components: Vec<usize>,
    /// Trend analysis
    pub trend: CollapseTrend,
}

/// Trend analysis for collapse prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollapseTrend {
    /// Direction of algebraic connectivity change
    pub connectivity_trend: TrendDirection,
    /// Direction of spectral gap change
    pub gap_trend: TrendDirection,
    /// Direction of Cheeger constant change
    pub cheeger_trend: TrendDirection,
    /// Overall stability assessment
    pub stability: StabilityAssessment,
}

/// Direction of a trend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    Increasing,
    Stable,
    Decreasing,
    Oscillating,
    Unknown,
}

/// Overall stability assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StabilityAssessment {
    /// System is stable and healthy
    Stable,
    /// Minor fluctuations but generally stable
    SlightlyUnstable,
    /// Noticeable instability, monitoring recommended
    Unstable,
    /// Significant degradation occurring
    Deteriorating,
    /// System approaching critical state
    Critical,
}

/// Coherence collapse predictor
pub struct CollapsePredictor {
    /// History of spectral snapshots
    spectral_history: VecDeque<SpectralSnapshot>,
    /// Maximum history size
    max_history: usize,
    /// Warning threshold for algebraic connectivity drop
    connectivity_threshold: f64,
    /// Warning threshold for spectral gap drop
    gap_threshold: f64,
    /// Warning threshold for rate of change
    rate_threshold: f64,
    /// Smoothing factor for trend detection
    smoothing_factor: f64,
    /// Current timestamp counter
    current_timestamp: u64,
}

impl Default for CollapsePredictor {
    fn default() -> Self {
        Self {
            spectral_history: VecDeque::new(),
            max_history: 100,
            connectivity_threshold: 0.1,
            gap_threshold: 0.05,
            rate_threshold: 0.2,
            smoothing_factor: 0.3,
            current_timestamp: 0,
        }
    }
}

impl CollapsePredictor {
    /// Create a new collapse predictor
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with custom thresholds
    pub fn with_thresholds(
        connectivity_threshold: f64,
        gap_threshold: f64,
        rate_threshold: f64,
    ) -> Self {
        Self {
            connectivity_threshold,
            gap_threshold,
            rate_threshold,
            ..Default::default()
        }
    }

    /// Set maximum history size
    pub fn set_max_history(&mut self, max_history: usize) {
        self.max_history = max_history;
        while self.spectral_history.len() > max_history {
            self.spectral_history.pop_front();
        }
    }

    /// Record a new snapshot from a graph
    pub fn record(&mut self, graph: &Graph) -> &SpectralSnapshot {
        let snapshot = self.create_snapshot(graph);
        self.add_snapshot(snapshot);
        self.spectral_history.back().unwrap()
    }

    /// Add a pre-computed snapshot
    pub fn add_snapshot(&mut self, snapshot: SpectralSnapshot) {
        self.spectral_history.push_back(snapshot);
        if self.spectral_history.len() > self.max_history {
            self.spectral_history.pop_front();
        }
        self.current_timestamp += 1;
    }

    /// Create a spectral snapshot from a graph
    fn create_snapshot(&self, graph: &Graph) -> SpectralSnapshot {
        let mut analyzer = SpectralAnalyzer::new(graph.clone());
        analyzer.compute_laplacian_spectrum();

        let mut cheeger_analyzer = CheegerAnalyzer::with_spectral(graph, analyzer.clone());
        let cheeger_bounds = cheeger_analyzer.compute_cheeger_bounds();

        let eigenvalues = analyzer.eigenvalues.clone();
        let near_zero_count = eigenvalues.iter().filter(|&&ev| ev.abs() < 1e-6).count();
        let eigenvalue_entropy = self.compute_eigenvalue_entropy(&eigenvalues);

        SpectralSnapshot {
            timestamp: self.current_timestamp,
            algebraic_connectivity: analyzer.algebraic_connectivity(),
            spectral_gap: analyzer.spectral_gap(),
            cheeger_bounds,
            eigenvalues,
            near_zero_count,
            eigenvalue_entropy,
            num_nodes: graph.n,
            num_edges: graph.num_edges(),
            total_weight: graph.total_weight(),
        }
    }

    /// Compute entropy of eigenvalue distribution
    fn compute_eigenvalue_entropy(&self, eigenvalues: &[f64]) -> f64 {
        if eigenvalues.is_empty() {
            return 0.0;
        }

        // Normalize eigenvalues to form a probability distribution
        let total: f64 = eigenvalues.iter().filter(|&&ev| ev > EPS).sum();
        if total < EPS {
            return 0.0;
        }

        let mut entropy = 0.0;
        for &ev in eigenvalues {
            if ev > EPS {
                let p = ev / total;
                entropy -= p * p.ln();
            }
        }

        entropy
    }

    /// Predict coherence collapse
    pub fn predict_collapse(&self, graph: &Graph) -> CollapsePrediction {
        // Create current snapshot
        let mut analyzer = SpectralAnalyzer::new(graph.clone());
        analyzer.compute_laplacian_spectrum();

        let mut cheeger_analyzer = CheegerAnalyzer::with_spectral(graph, analyzer.clone());
        let cheeger_bounds = cheeger_analyzer.compute_cheeger_bounds();

        let current = SpectralSnapshot {
            timestamp: self.current_timestamp,
            algebraic_connectivity: analyzer.algebraic_connectivity(),
            spectral_gap: analyzer.spectral_gap(),
            cheeger_bounds,
            eigenvalues: analyzer.eigenvalues.clone(),
            near_zero_count: analyzer.eigenvalues.iter()
                .filter(|&&ev| ev.abs() < 1e-6)
                .count(),
            eigenvalue_entropy: self.compute_eigenvalue_entropy(&analyzer.eigenvalues),
            num_nodes: graph.n,
            num_edges: graph.num_edges(),
            total_weight: graph.total_weight(),
        };

        let mut warnings = Vec::new();
        let mut risk_score = 0.0;

        // Check absolute thresholds
        self.check_absolute_thresholds(&current, &mut warnings, &mut risk_score);

        // Check trends if we have history
        let trend = self.analyze_trends(&current);
        self.check_trend_warnings(&trend, &mut warnings, &mut risk_score);

        // Check rate of change
        if let Some(rate_warning) = self.check_rate_of_change(&current) {
            risk_score += 0.2;
            warnings.push(rate_warning);
        }

        // Determine warning level
        let warning_level = self.compute_warning_level(risk_score);

        // Estimate collapse time
        let estimated_collapse_time = self.estimate_collapse_time(&current, &trend);

        // Find fragile components
        let fragile_components = self.find_fragile_components(&current);

        CollapsePrediction {
            risk_score: risk_score.clamp(0.0, 1.0),
            warning_level,
            warnings,
            estimated_collapse_time,
            fragile_components,
            trend,
        }
    }

    /// Check absolute threshold violations
    fn check_absolute_thresholds(
        &self,
        current: &SpectralSnapshot,
        warnings: &mut Vec<Warning>,
        risk_score: &mut f64,
    ) {
        // Check algebraic connectivity
        if current.algebraic_connectivity < self.connectivity_threshold {
            *risk_score += 0.3;
            warnings.push(Warning {
                level: WarningLevel::High,
                message: "Algebraic connectivity is critically low".to_string(),
                metric: "algebraic_connectivity".to_string(),
                current_value: current.algebraic_connectivity,
                threshold: self.connectivity_threshold,
                rate_of_change: None,
                recommendations: vec![
                    "Add edges to strengthen connectivity".to_string(),
                    "Merge weakly connected components".to_string(),
                ],
            });
        }

        // Check spectral gap
        if current.spectral_gap.gap < self.gap_threshold {
            *risk_score += 0.2;
            warnings.push(Warning {
                level: WarningLevel::Medium,
                message: "Spectral gap indicates weak cluster separation".to_string(),
                metric: "spectral_gap".to_string(),
                current_value: current.spectral_gap.gap,
                threshold: self.gap_threshold,
                rate_of_change: None,
                recommendations: vec![
                    "Review cluster boundaries".to_string(),
                    "Consider merging overlapping communities".to_string(),
                ],
            });
        }

        // Check for multiple near-zero eigenvalues (disconnection)
        if current.near_zero_count > 1 {
            *risk_score += 0.1 * (current.near_zero_count - 1) as f64;
            warnings.push(Warning {
                level: WarningLevel::High,
                message: format!("Graph has {} disconnected components", current.near_zero_count),
                metric: "near_zero_eigenvalues".to_string(),
                current_value: current.near_zero_count as f64,
                threshold: 1.0,
                rate_of_change: None,
                recommendations: vec![
                    "Add edges to connect components".to_string(),
                    "Review component isolation".to_string(),
                ],
            });
        }

        // Check Cheeger constant
        if current.cheeger_bounds.cheeger_constant < 0.05 {
            *risk_score += 0.25;
            warnings.push(Warning {
                level: WarningLevel::High,
                message: "Cheeger constant indicates severe bottleneck".to_string(),
                metric: "cheeger_constant".to_string(),
                current_value: current.cheeger_bounds.cheeger_constant,
                threshold: 0.05,
                rate_of_change: None,
                recommendations: vec![
                    "Identify and strengthen bottleneck edges".to_string(),
                    "Add redundant connections".to_string(),
                ],
            });
        }
    }

    /// Analyze trends in spectral properties
    fn analyze_trends(&self, current: &SpectralSnapshot) -> CollapseTrend {
        if self.spectral_history.len() < 3 {
            return CollapseTrend {
                connectivity_trend: TrendDirection::Unknown,
                gap_trend: TrendDirection::Unknown,
                cheeger_trend: TrendDirection::Unknown,
                stability: StabilityAssessment::Stable,
            };
        }

        let connectivity_trend = self.compute_trend(
            self.spectral_history.iter()
                .map(|s| s.algebraic_connectivity)
                .collect::<Vec<_>>()
                .as_slice(),
            current.algebraic_connectivity,
        );

        let gap_trend = self.compute_trend(
            self.spectral_history.iter()
                .map(|s| s.spectral_gap.gap)
                .collect::<Vec<_>>()
                .as_slice(),
            current.spectral_gap.gap,
        );

        let cheeger_trend = self.compute_trend(
            self.spectral_history.iter()
                .map(|s| s.cheeger_bounds.cheeger_constant)
                .collect::<Vec<_>>()
                .as_slice(),
            current.cheeger_bounds.cheeger_constant,
        );

        let stability = self.assess_stability(&connectivity_trend, &gap_trend, &cheeger_trend);

        CollapseTrend {
            connectivity_trend,
            gap_trend,
            cheeger_trend,
            stability,
        }
    }

    /// Compute trend direction from history
    fn compute_trend(&self, history: &[f64], current: f64) -> TrendDirection {
        if history.len() < 2 {
            return TrendDirection::Unknown;
        }

        // Use exponential smoothing
        let mut smoothed = history[0];
        for &val in &history[1..] {
            smoothed = self.smoothing_factor * val + (1.0 - self.smoothing_factor) * smoothed;
        }

        // Compute recent slope
        let recent_avg: f64 = history.iter().rev().take(3).sum::<f64>() / 3.0;
        let older_avg: f64 = history.iter().take(3).sum::<f64>() / 3.0;

        let diff = current - smoothed;
        let slope = recent_avg - older_avg;

        // Check for oscillation
        let mut sign_changes = 0;
        for i in 1..history.len() {
            let prev_diff = history[i] - history[i - 1];
            let curr_diff = if i + 1 < history.len() {
                history[i + 1] - history[i]
            } else {
                current - history[i]
            };

            if prev_diff * curr_diff < 0.0 {
                sign_changes += 1;
            }
        }

        if sign_changes as f64 / history.len() as f64 > 0.3 {
            return TrendDirection::Oscillating;
        }

        // Determine direction
        if slope.abs() < EPS && diff.abs() < EPS {
            TrendDirection::Stable
        } else if slope > 0.0 {
            TrendDirection::Increasing
        } else {
            TrendDirection::Decreasing
        }
    }

    /// Assess overall stability
    fn assess_stability(
        &self,
        connectivity: &TrendDirection,
        gap: &TrendDirection,
        cheeger: &TrendDirection,
    ) -> StabilityAssessment {
        let negative_trends = [connectivity, gap, cheeger]
            .iter()
            .filter(|&&t| *t == TrendDirection::Decreasing)
            .count();

        let oscillating = [connectivity, gap, cheeger]
            .iter()
            .filter(|&&t| *t == TrendDirection::Oscillating)
            .count();

        if negative_trends >= 3 {
            StabilityAssessment::Critical
        } else if negative_trends >= 2 {
            StabilityAssessment::Deteriorating
        } else if negative_trends >= 1 || oscillating >= 2 {
            StabilityAssessment::Unstable
        } else if oscillating >= 1 {
            StabilityAssessment::SlightlyUnstable
        } else {
            StabilityAssessment::Stable
        }
    }

    /// Check trend-based warnings
    fn check_trend_warnings(
        &self,
        trend: &CollapseTrend,
        warnings: &mut Vec<Warning>,
        risk_score: &mut f64,
    ) {
        if trend.connectivity_trend == TrendDirection::Decreasing {
            *risk_score += 0.15;
            warnings.push(Warning {
                level: WarningLevel::Medium,
                message: "Algebraic connectivity is declining".to_string(),
                metric: "connectivity_trend".to_string(),
                current_value: 0.0,
                threshold: 0.0,
                rate_of_change: None,
                recommendations: vec![
                    "Monitor for further degradation".to_string(),
                    "Consider preventive edge additions".to_string(),
                ],
            });
        }

        match trend.stability {
            StabilityAssessment::Critical => {
                *risk_score += 0.3;
                warnings.push(Warning {
                    level: WarningLevel::Critical,
                    message: "System stability is critical - multiple metrics deteriorating".to_string(),
                    metric: "stability".to_string(),
                    current_value: 4.0,
                    threshold: 1.0,
                    rate_of_change: None,
                    recommendations: vec![
                        "Immediate intervention required".to_string(),
                        "Halt any changes that may affect connectivity".to_string(),
                        "Review and strengthen graph structure".to_string(),
                    ],
                });
            }
            StabilityAssessment::Deteriorating => {
                *risk_score += 0.2;
                warnings.push(Warning {
                    level: WarningLevel::High,
                    message: "System is deteriorating".to_string(),
                    metric: "stability".to_string(),
                    current_value: 3.0,
                    threshold: 1.0,
                    rate_of_change: None,
                    recommendations: vec![
                        "Investigate cause of degradation".to_string(),
                        "Plan corrective actions".to_string(),
                    ],
                });
            }
            _ => {}
        }
    }

    /// Check rate of change for sudden drops
    fn check_rate_of_change(&self, current: &SpectralSnapshot) -> Option<Warning> {
        if self.spectral_history.is_empty() {
            return None;
        }

        let prev = self.spectral_history.back().unwrap();

        // Check connectivity rate of change
        let connectivity_change = prev.algebraic_connectivity - current.algebraic_connectivity;
        let relative_change = if prev.algebraic_connectivity > EPS {
            connectivity_change / prev.algebraic_connectivity
        } else {
            0.0
        };

        if relative_change > self.rate_threshold {
            Some(Warning {
                level: WarningLevel::High,
                message: "Rapid drop in algebraic connectivity detected".to_string(),
                metric: "connectivity_rate".to_string(),
                current_value: current.algebraic_connectivity,
                threshold: prev.algebraic_connectivity,
                rate_of_change: Some(relative_change),
                recommendations: vec![
                    "Investigate recent changes".to_string(),
                    "Check for removed edges or nodes".to_string(),
                ],
            })
        } else {
            None
        }
    }

    /// Compute warning level from risk score
    fn compute_warning_level(&self, risk_score: f64) -> WarningLevel {
        if risk_score >= 0.8 {
            WarningLevel::Critical
        } else if risk_score >= 0.6 {
            WarningLevel::High
        } else if risk_score >= 0.4 {
            WarningLevel::Medium
        } else if risk_score >= 0.2 {
            WarningLevel::Low
        } else {
            WarningLevel::None
        }
    }

    /// Estimate time to collapse based on trends
    fn estimate_collapse_time(
        &self,
        current: &SpectralSnapshot,
        trend: &CollapseTrend,
    ) -> Option<u64> {
        if self.spectral_history.len() < 3 {
            return None;
        }

        if trend.connectivity_trend != TrendDirection::Decreasing {
            return None;
        }

        // Fit linear regression to connectivity
        let values: Vec<f64> = self.spectral_history
            .iter()
            .map(|s| s.algebraic_connectivity)
            .collect();

        let n = values.len() as f64;
        let sum_x: f64 = (0..values.len()).map(|i| i as f64).sum();
        let sum_y: f64 = values.iter().sum();
        let sum_xy: f64 = values.iter().enumerate().map(|(i, &y)| i as f64 * y).sum();
        let sum_xx: f64 = (0..values.len()).map(|i| (i as f64).powi(2)).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);

        if slope >= 0.0 {
            return None; // Not decreasing
        }

        // Estimate when connectivity reaches threshold
        let current_connectivity = current.algebraic_connectivity;
        let steps_to_threshold = (current_connectivity - self.connectivity_threshold) / (-slope);

        if steps_to_threshold > 0.0 && steps_to_threshold < 1000.0 {
            Some(steps_to_threshold.ceil() as u64)
        } else {
            None
        }
    }

    /// Find components that are at risk of disconnection
    fn find_fragile_components(&self, current: &SpectralSnapshot) -> Vec<usize> {
        // Components with near-zero eigenvalues
        let mut fragile = Vec::new();

        for (i, &ev) in current.eigenvalues.iter().enumerate() {
            if ev > EPS && ev < self.connectivity_threshold {
                fragile.push(i);
            }
        }

        fragile
    }

    /// Get early warning signal if any
    pub fn early_warning_signal(&self) -> Option<Warning> {
        if self.spectral_history.len() < 2 {
            return None;
        }

        let current = self.spectral_history.back()?;
        let prev = self.spectral_history.get(self.spectral_history.len() - 2)?;

        // Check for early signs of degradation
        let connectivity_drop = prev.algebraic_connectivity - current.algebraic_connectivity;
        let relative_drop = if prev.algebraic_connectivity > EPS {
            connectivity_drop / prev.algebraic_connectivity
        } else {
            0.0
        };

        if relative_drop > 0.1 {
            Some(Warning {
                level: WarningLevel::Low,
                message: "Early warning: Connectivity showing decline".to_string(),
                metric: "early_connectivity".to_string(),
                current_value: current.algebraic_connectivity,
                threshold: prev.algebraic_connectivity,
                rate_of_change: Some(relative_drop),
                recommendations: vec![
                    "Continue monitoring".to_string(),
                    "Review recent graph modifications".to_string(),
                ],
            })
        } else {
            None
        }
    }

    /// Get the spectral history
    pub fn history(&self) -> &VecDeque<SpectralSnapshot> {
        &self.spectral_history
    }

    /// Clear history
    pub fn clear_history(&mut self) {
        self.spectral_history.clear();
        self.current_timestamp = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_connected_graph(n: usize) -> Graph {
        let edges: Vec<(usize, usize, f64)> = (0..n - 1)
            .map(|i| (i, i + 1, 1.0))
            .collect();
        Graph::from_edges(n, &edges)
    }

    fn create_complete_graph(n: usize) -> Graph {
        let mut edges = Vec::new();
        for i in 0..n {
            for j in i + 1..n {
                edges.push((i, j, 1.0));
            }
        }
        Graph::from_edges(n, &edges)
    }

    #[test]
    fn test_collapse_predictor_stable() {
        let g = create_complete_graph(10);
        let mut predictor = CollapsePredictor::new();

        // Record several snapshots of the same stable graph
        for _ in 0..5 {
            predictor.record(&g);
        }

        let prediction = predictor.predict_collapse(&g);
        assert_eq!(prediction.warning_level, WarningLevel::None);
        assert!(prediction.risk_score < 0.3);
    }

    #[test]
    fn test_collapse_predictor_path_graph() {
        let g = create_connected_graph(20);
        let predictor = CollapsePredictor::new();

        // Path graph has low connectivity
        let prediction = predictor.predict_collapse(&g);

        // Should have some warnings due to low connectivity
        assert!(prediction.risk_score > 0.1);
    }

    #[test]
    fn test_warning_levels() {
        assert_eq!(WarningLevel::None.severity(), 0);
        assert_eq!(WarningLevel::Critical.severity(), 4);
        assert_eq!(WarningLevel::from_severity(2), WarningLevel::Medium);
    }

    #[test]
    fn test_trend_detection() {
        let mut predictor = CollapsePredictor::new();

        // Simulate degrading graph
        for i in 0..10 {
            let n = 20 - i; // Shrinking graph
            if n > 2 {
                let g = create_connected_graph(n);
                predictor.record(&g);
            }
        }

        // Check that we detect the degradation
        if predictor.spectral_history.len() >= 3 {
            let g = create_connected_graph(10);
            let prediction = predictor.predict_collapse(&g);

            // Should detect some instability
            assert!(prediction.trend.stability != StabilityAssessment::Stable);
        }
    }

    #[test]
    fn test_early_warning() {
        let mut predictor = CollapsePredictor::new();

        // Record a stable graph
        let stable = create_complete_graph(10);
        predictor.record(&stable);

        // Record a slightly degraded graph
        let mut degraded = create_complete_graph(10);
        // Remove some edges to degrade
        degraded.adj[0].retain(|(n, _)| *n < 5);
        degraded.adj[1].retain(|(n, _)| *n < 5);
        predictor.record(&degraded);

        // Check for early warning
        let warning = predictor.early_warning_signal();
        // May or may not trigger depending on magnitude of change
        if let Some(w) = warning {
            assert!(w.level == WarningLevel::Low);
        }
    }

    #[test]
    fn test_spectral_snapshot() {
        let g = create_complete_graph(5);
        let predictor = CollapsePredictor::new();
        let snapshot = predictor.create_snapshot(&g);

        assert_eq!(snapshot.num_nodes, 5);
        assert_eq!(snapshot.num_edges, 10); // C(5,2) = 10
        assert!(snapshot.algebraic_connectivity > 0.0);
        assert!(snapshot.eigenvalue_entropy >= 0.0);
    }
}
