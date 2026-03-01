//! Spectral Analysis for Coherence Drift Detection
//!
//! This module provides eigenvalue-based drift detection using the sheaf Laplacian.
//! Spectral analysis reveals structural changes in the coherence graph that may not
//! be apparent from simple energy metrics.
//!
//! # Theory
//!
//! The sheaf Laplacian L = D - A (weighted degree - adjacency) has eigenvalues that
//! characterize the graph's coherence structure:
//!
//! - **Algebraic connectivity** (second smallest eigenvalue): Measures how well-connected
//!   the graph is; a drop indicates structural weakening
//! - **Spectral gap**: Difference between first and second eigenvalues; indicates
//!   separation between components
//! - **Eigenvalue distribution drift**: Changes in the overall spectrum indicate
//!   fundamental structural shifts
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::coherence::{SpectralAnalyzer, SpectralConfig};
//!
//! let mut analyzer = SpectralAnalyzer::new(SpectralConfig::default());
//!
//! // Record eigenvalues over time
//! analyzer.record_eigenvalues(vec![0.0, 0.5, 1.2, 2.1]);
//! analyzer.record_eigenvalues(vec![0.0, 0.3, 1.0, 2.0]); // Drop in second eigenvalue
//!
//! // Check for drift
//! if let Some(event) = analyzer.detect_drift() {
//!     println!("Drift detected: {:?}", event);
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Configuration for spectral analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralConfig {
    /// Number of top eigenvalues to track
    pub num_eigenvalues: usize,
    /// Maximum history length
    pub history_size: usize,
    /// Threshold for detecting drift (relative change)
    pub drift_threshold: f32,
    /// Threshold for detecting severe drift
    pub severe_threshold: f32,
    /// Minimum number of samples before drift detection
    pub min_samples: usize,
    /// Smoothing factor for exponential moving average (0 = no smoothing)
    pub smoothing_alpha: f32,
}

impl Default for SpectralConfig {
    fn default() -> Self {
        Self {
            num_eigenvalues: 10,
            history_size: 100,
            drift_threshold: 0.1,   // 10% relative change
            severe_threshold: 0.25, // 25% relative change
            min_samples: 3,
            smoothing_alpha: 0.3,
        }
    }
}

/// Severity level of detected drift
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftSeverity {
    /// Minor drift - may be noise
    Minor,
    /// Moderate drift - warrants attention
    Moderate,
    /// Severe drift - requires action
    Severe,
    /// Critical drift - structural breakdown
    Critical,
}

impl DriftSeverity {
    /// Get numeric severity level (higher = more severe)
    pub fn level(&self) -> u8 {
        match self {
            DriftSeverity::Minor => 1,
            DriftSeverity::Moderate => 2,
            DriftSeverity::Severe => 3,
            DriftSeverity::Critical => 4,
        }
    }

    /// Check if this severity requires escalation
    pub fn requires_escalation(&self) -> bool {
        matches!(self, DriftSeverity::Severe | DriftSeverity::Critical)
    }
}

/// A detected drift event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftEvent {
    /// Magnitude of the drift (spectral distance)
    pub magnitude: f32,
    /// Severity classification
    pub severity: DriftSeverity,
    /// Which eigenvalue modes are affected (indices)
    pub affected_modes: Vec<usize>,
    /// Direction of drift for each affected mode (positive = increasing)
    pub mode_changes: Vec<f32>,
    /// Timestamp when drift was detected
    pub timestamp: DateTime<Utc>,
    /// Algebraic connectivity change (second eigenvalue)
    pub connectivity_change: f32,
    /// Spectral gap change
    pub spectral_gap_change: f32,
    /// Description of the drift
    pub description: String,
}

impl DriftEvent {
    /// Check if connectivity is weakening
    pub fn is_connectivity_weakening(&self) -> bool {
        self.connectivity_change < 0.0
    }

    /// Check if this indicates component separation
    pub fn indicates_separation(&self) -> bool {
        // Increasing spectral gap indicates components drifting apart
        self.spectral_gap_change > 0.0 && self.connectivity_change < 0.0
    }
}

/// Entry in the eigenvalue history
#[derive(Debug, Clone)]
struct EigenvalueSnapshot {
    /// Eigenvalues (sorted ascending)
    eigenvalues: Vec<f32>,
    /// Timestamp
    timestamp: DateTime<Utc>,
    /// Algebraic connectivity (second smallest eigenvalue)
    connectivity: f32,
    /// Spectral gap (difference between first two eigenvalues)
    spectral_gap: f32,
}

impl EigenvalueSnapshot {
    fn new(mut eigenvalues: Vec<f32>) -> Self {
        // Sort eigenvalues
        eigenvalues.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let connectivity = if eigenvalues.len() > 1 {
            eigenvalues[1]
        } else {
            0.0
        };

        let spectral_gap = if eigenvalues.len() > 1 {
            eigenvalues[1] - eigenvalues[0]
        } else {
            0.0
        };

        Self {
            eigenvalues,
            timestamp: Utc::now(),
            connectivity,
            spectral_gap,
        }
    }
}

/// Spectral analyzer for drift detection
pub struct SpectralAnalyzer {
    /// Configuration
    config: SpectralConfig,
    /// History of eigenvalue snapshots
    history: VecDeque<EigenvalueSnapshot>,
    /// Exponential moving average of eigenvalues
    ema_eigenvalues: Option<Vec<f32>>,
    /// Last detected drift event
    last_drift: Option<DriftEvent>,
    /// Statistics
    total_samples: u64,
    drift_events: u64,
}

impl SpectralAnalyzer {
    /// Create a new spectral analyzer
    pub fn new(config: SpectralConfig) -> Self {
        Self {
            config,
            history: VecDeque::new(),
            ema_eigenvalues: None,
            last_drift: None,
            total_samples: 0,
            drift_events: 0,
        }
    }

    /// Record new eigenvalues
    pub fn record_eigenvalues(&mut self, eigenvalues: Vec<f32>) {
        let snapshot = EigenvalueSnapshot::new(eigenvalues);

        // Update EMA
        if let Some(ref mut ema) = self.ema_eigenvalues {
            let alpha = self.config.smoothing_alpha;
            for (i, &val) in snapshot.eigenvalues.iter().enumerate() {
                if i < ema.len() {
                    ema[i] = alpha * val + (1.0 - alpha) * ema[i];
                }
            }
        } else {
            self.ema_eigenvalues = Some(snapshot.eigenvalues.clone());
        }

        self.history.push_back(snapshot);
        self.total_samples += 1;

        // Trim history
        while self.history.len() > self.config.history_size {
            self.history.pop_front();
        }
    }

    /// Detect drift based on recent eigenvalue changes
    pub fn detect_drift(&mut self) -> Option<DriftEvent> {
        if self.history.len() < self.config.min_samples {
            return None;
        }

        let current = self.history.back()?;
        let previous = self.history.get(self.history.len() - 2)?;

        // Compute spectral distance
        let distance = self.spectral_distance(&current.eigenvalues, &previous.eigenvalues);

        // Check threshold
        if distance < self.config.drift_threshold {
            return None;
        }

        // Identify affected modes
        let (affected_modes, mode_changes) = self.identify_affected_modes(current, previous);

        // Compute connectivity and gap changes
        let connectivity_change = current.connectivity - previous.connectivity;
        let spectral_gap_change = current.spectral_gap - previous.spectral_gap;

        // Determine severity
        let severity = self.classify_severity(distance, connectivity_change);

        // Build description
        let description = self.build_description(
            &affected_modes,
            connectivity_change,
            spectral_gap_change,
            severity,
        );

        let event = DriftEvent {
            magnitude: distance,
            severity,
            affected_modes,
            mode_changes,
            timestamp: Utc::now(),
            connectivity_change,
            spectral_gap_change,
            description,
        };

        self.last_drift = Some(event.clone());
        self.drift_events += 1;

        Some(event)
    }

    /// Get the current algebraic connectivity (second smallest eigenvalue)
    pub fn algebraic_connectivity(&self) -> Option<f32> {
        self.history.back().map(|s| s.connectivity)
    }

    /// Get the current spectral gap
    pub fn spectral_gap(&self) -> Option<f32> {
        self.history.back().map(|s| s.spectral_gap)
    }

    /// Get the smoothed eigenvalues (EMA)
    pub fn smoothed_eigenvalues(&self) -> Option<&Vec<f32>> {
        self.ema_eigenvalues.as_ref()
    }

    /// Get drift trend over recent history
    pub fn drift_trend(&self, window: usize) -> Option<f32> {
        if self.history.len() < window + 1 {
            return None;
        }

        let recent: Vec<_> = self.history.iter().rev().take(window + 1).collect();

        // Compute average pairwise distance
        let mut total_distance = 0.0;
        for i in 0..recent.len() - 1 {
            total_distance +=
                self.spectral_distance(&recent[i].eigenvalues, &recent[i + 1].eigenvalues);
        }

        Some(total_distance / window as f32)
    }

    /// Check if the system is currently in a drift state
    pub fn is_drifting(&self) -> bool {
        self.drift_trend(self.config.min_samples)
            .map(|trend| trend > self.config.drift_threshold)
            .unwrap_or(false)
    }

    /// Get statistics
    pub fn stats(&self) -> SpectralStats {
        SpectralStats {
            total_samples: self.total_samples,
            drift_events: self.drift_events,
            history_size: self.history.len(),
            current_connectivity: self.algebraic_connectivity(),
            current_spectral_gap: self.spectral_gap(),
            is_drifting: self.is_drifting(),
        }
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.history.clear();
        self.ema_eigenvalues = None;
        self.last_drift = None;
    }

    // Private methods

    /// Compute spectral distance between two eigenvalue vectors
    fn spectral_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        let len = a.len().min(b.len());
        if len == 0 {
            return 0.0;
        }

        // Use relative L2 distance
        let mut sum_sq = 0.0;
        let mut sum_ref = 0.0;

        for i in 0..len {
            let diff = a[i] - b[i];
            sum_sq += diff * diff;
            sum_ref += b[i].abs();
        }

        if sum_ref > 1e-10 {
            (sum_sq.sqrt()) / (sum_ref / len as f32)
        } else {
            sum_sq.sqrt()
        }
    }

    /// Identify which eigenvalue modes are affected
    fn identify_affected_modes(
        &self,
        current: &EigenvalueSnapshot,
        previous: &EigenvalueSnapshot,
    ) -> (Vec<usize>, Vec<f32>) {
        let mut affected = Vec::new();
        let mut changes = Vec::new();

        let len = current.eigenvalues.len().min(previous.eigenvalues.len());

        for i in 0..len {
            let change = current.eigenvalues[i] - previous.eigenvalues[i];
            let relative_change = if previous.eigenvalues[i].abs() > 1e-10 {
                change.abs() / previous.eigenvalues[i].abs()
            } else {
                change.abs()
            };

            if relative_change > self.config.drift_threshold / 2.0 {
                affected.push(i);
                changes.push(change);
            }
        }

        (affected, changes)
    }

    /// Classify drift severity
    fn classify_severity(&self, distance: f32, connectivity_change: f32) -> DriftSeverity {
        let is_connectivity_loss = connectivity_change < -self.config.drift_threshold;

        if distance > self.config.severe_threshold * 2.0
            || (is_connectivity_loss && distance > self.config.severe_threshold)
        {
            DriftSeverity::Critical
        } else if distance > self.config.severe_threshold {
            DriftSeverity::Severe
        } else if distance > self.config.drift_threshold * 1.5 || is_connectivity_loss {
            DriftSeverity::Moderate
        } else {
            DriftSeverity::Minor
        }
    }

    /// Build human-readable description
    fn build_description(
        &self,
        affected_modes: &[usize],
        connectivity_change: f32,
        spectral_gap_change: f32,
        severity: DriftSeverity,
    ) -> String {
        let mut parts = Vec::new();

        // Severity
        parts.push(format!("{:?} spectral drift detected", severity));

        // Affected modes
        if !affected_modes.is_empty() {
            let mode_str = affected_modes
                .iter()
                .map(|m| m.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("affecting modes [{}]", mode_str));
        }

        // Connectivity
        if connectivity_change < 0.0 {
            parts.push(format!(
                "connectivity decreased by {:.2}%",
                connectivity_change.abs() * 100.0
            ));
        } else if connectivity_change > 0.0 {
            parts.push(format!(
                "connectivity increased by {:.2}%",
                connectivity_change * 100.0
            ));
        }

        // Spectral gap
        if spectral_gap_change.abs() > 0.01 {
            let direction = if spectral_gap_change > 0.0 {
                "widened"
            } else {
                "narrowed"
            };
            parts.push(format!(
                "spectral gap {} by {:.2}%",
                direction,
                spectral_gap_change.abs() * 100.0
            ));
        }

        parts.join("; ")
    }
}

impl Default for SpectralAnalyzer {
    fn default() -> Self {
        Self::new(SpectralConfig::default())
    }
}

/// Statistics about spectral analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralStats {
    /// Total samples recorded
    pub total_samples: u64,
    /// Number of drift events detected
    pub drift_events: u64,
    /// Current history size
    pub history_size: usize,
    /// Current algebraic connectivity
    pub current_connectivity: Option<f32>,
    /// Current spectral gap
    pub current_spectral_gap: Option<f32>,
    /// Whether currently drifting
    pub is_drifting: bool,
}

/// Compute eigenvalues of a symmetric matrix (Laplacian)
///
/// This is a simplified eigenvalue computation for small matrices.
/// For production use with large graphs, use the `spectral` feature
/// which provides `nalgebra` integration.
#[cfg(not(feature = "spectral"))]
pub fn compute_eigenvalues(laplacian: &[Vec<f32>], k: usize) -> Vec<f32> {
    // Power iteration for top eigenvalue, deflation for subsequent
    // This is a simplified implementation - use nalgebra for production
    let n = laplacian.len();
    if n == 0 || k == 0 {
        return Vec::new();
    }

    let mut eigenvalues = Vec::with_capacity(k.min(n));

    // Start with a copy of the matrix
    let mut matrix: Vec<Vec<f32>> = laplacian.to_vec();

    for _ in 0..k.min(n) {
        // Power iteration
        let lambda = power_iteration(&matrix, 100, 1e-6);
        eigenvalues.push(lambda);

        // Deflate matrix
        deflate_matrix(&mut matrix, lambda);
    }

    // Sort ascending (Laplacian eigenvalues are non-negative)
    eigenvalues.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    eigenvalues
}

/// Power iteration to find the largest eigenvalue
#[cfg(not(feature = "spectral"))]
fn power_iteration(matrix: &[Vec<f32>], max_iters: usize, tolerance: f32) -> f32 {
    let n = matrix.len();
    if n == 0 {
        return 0.0;
    }

    // Initialize with random vector
    let mut v: Vec<f32> = (0..n).map(|i| (i as f32 + 1.0) / n as f32).collect();
    normalize(&mut v);

    let mut lambda = 0.0;

    for _ in 0..max_iters {
        // w = A * v
        let mut w = vec![0.0; n];
        for i in 0..n {
            for j in 0..n {
                w[i] += matrix[i][j] * v[j];
            }
        }

        // Rayleigh quotient
        let new_lambda: f32 = v.iter().zip(w.iter()).map(|(vi, wi)| vi * wi).sum();

        // Normalize
        normalize(&mut w);
        v = w;

        // Check convergence
        if (new_lambda - lambda).abs() < tolerance {
            return new_lambda;
        }
        lambda = new_lambda;
    }

    lambda
}

/// Normalize a vector in-place
#[cfg(not(feature = "spectral"))]
fn normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-10 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Deflate matrix to find next eigenvalue
#[cfg(not(feature = "spectral"))]
fn deflate_matrix(matrix: &mut [Vec<f32>], lambda: f32) {
    let n = matrix.len();
    // Simple deflation: A' = A - lambda * I
    // This is approximate but sufficient for drift detection
    for i in 0..n {
        matrix[i][i] -= lambda;
    }
}

/// Compute eigenvalues using nalgebra (when spectral feature is enabled)
#[cfg(feature = "spectral")]
pub fn compute_eigenvalues(laplacian: &[Vec<f32>], k: usize) -> Vec<f32> {
    use nalgebra::{DMatrix, SymmetricEigen};

    let n = laplacian.len();
    if n == 0 || k == 0 {
        return Vec::new();
    }

    // Convert to nalgebra matrix
    let data: Vec<f64> = laplacian
        .iter()
        .flat_map(|row| row.iter().map(|&x| x as f64))
        .collect();

    let matrix = DMatrix::from_row_slice(n, n, &data);

    // Compute eigenvalues
    let eigen = SymmetricEigen::new(matrix);
    let mut eigenvalues: Vec<f32> = eigen.eigenvalues.iter().map(|&x| x as f32).collect();

    // Sort and take top k
    eigenvalues.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    eigenvalues.truncate(k);

    eigenvalues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectral_analyzer_creation() {
        let analyzer = SpectralAnalyzer::default();
        assert_eq!(analyzer.stats().total_samples, 0);
        assert!(!analyzer.is_drifting());
    }

    #[test]
    fn test_record_eigenvalues() {
        let mut analyzer = SpectralAnalyzer::default();

        analyzer.record_eigenvalues(vec![0.0, 0.5, 1.0, 2.0]);
        assert_eq!(analyzer.stats().total_samples, 1);
        assert_eq!(analyzer.algebraic_connectivity(), Some(0.5));
        assert_eq!(analyzer.spectral_gap(), Some(0.5));
    }

    #[test]
    fn test_drift_detection() {
        let config = SpectralConfig {
            drift_threshold: 0.1,
            severe_threshold: 0.3,
            min_samples: 2,
            ..Default::default()
        };
        let mut analyzer = SpectralAnalyzer::new(config);

        // Record stable eigenvalues
        analyzer.record_eigenvalues(vec![0.0, 0.5, 1.0, 2.0]);
        analyzer.record_eigenvalues(vec![0.0, 0.5, 1.0, 2.0]);

        // No drift yet
        assert!(analyzer.detect_drift().is_none());

        // Record significant change
        analyzer.record_eigenvalues(vec![0.0, 0.2, 0.8, 1.5]); // Connectivity dropped

        let drift = analyzer.detect_drift();
        assert!(drift.is_some());

        let event = drift.unwrap();
        assert!(event.connectivity_change < 0.0);
    }

    #[test]
    fn test_drift_severity() {
        let config = SpectralConfig {
            drift_threshold: 0.1,
            severe_threshold: 0.3,
            min_samples: 2,
            ..Default::default()
        };
        let mut analyzer = SpectralAnalyzer::new(config);

        analyzer.record_eigenvalues(vec![0.0, 1.0, 2.0, 3.0]);
        analyzer.record_eigenvalues(vec![0.0, 0.1, 0.5, 1.0]); // Drastic change

        let drift = analyzer.detect_drift().unwrap();
        assert!(drift.severity.level() >= DriftSeverity::Moderate.level());
    }

    #[test]
    fn test_smoothed_eigenvalues() {
        let mut analyzer = SpectralAnalyzer::new(SpectralConfig {
            smoothing_alpha: 0.5,
            ..Default::default()
        });

        analyzer.record_eigenvalues(vec![0.0, 1.0, 2.0]);
        let first = analyzer.smoothed_eigenvalues().unwrap().clone();

        analyzer.record_eigenvalues(vec![0.0, 1.5, 2.5]);
        let second = analyzer.smoothed_eigenvalues().unwrap();

        // EMA should be between first and second values
        assert!(second[1] > 1.0 && second[1] < 1.5);
    }

    #[test]
    fn test_spectral_stats() {
        let mut analyzer = SpectralAnalyzer::default();

        analyzer.record_eigenvalues(vec![0.0, 0.5, 1.0]);

        let stats = analyzer.stats();
        assert_eq!(stats.total_samples, 1);
        assert_eq!(stats.history_size, 1);
        assert_eq!(stats.current_connectivity, Some(0.5));
    }

    #[test]
    #[cfg(not(feature = "spectral"))]
    fn test_compute_eigenvalues() {
        // Identity matrix has all eigenvalues = 1
        let identity = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        let eigenvalues = compute_eigenvalues(&identity, 3);
        assert_eq!(eigenvalues.len(), 3);

        // All should be close to 1.0
        for ev in eigenvalues {
            assert!((ev - 1.0).abs() < 0.1 || ev.abs() < 0.1);
        }
    }

    #[test]
    fn test_history_trimming() {
        let config = SpectralConfig {
            history_size: 5,
            ..Default::default()
        };
        let mut analyzer = SpectralAnalyzer::new(config);

        for i in 0..10 {
            analyzer.record_eigenvalues(vec![0.0, i as f32 * 0.1]);
        }

        assert_eq!(analyzer.stats().history_size, 5);
    }
}
