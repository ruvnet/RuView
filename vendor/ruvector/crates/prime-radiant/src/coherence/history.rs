//! Energy History Tracking
//!
//! This module provides time-series tracking of coherence energy for trend analysis,
//! anomaly detection, and adaptive threshold tuning.
//!
//! # Features
//!
//! - Rolling window of energy snapshots
//! - Trend detection (increasing, decreasing, stable)
//! - Anomaly detection using statistical methods
//! - Persistence tracking for threshold tuning
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::coherence::{EnergyHistory, EnergyHistoryConfig};
//!
//! let mut history = EnergyHistory::new(EnergyHistoryConfig::default());
//!
//! // Record energy values
//! history.record(1.0);
//! history.record(1.2);
//! history.record(1.5);
//!
//! // Get trend
//! let trend = history.trend();
//! println!("Energy is {:?}", trend.direction);
//! ```

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Configuration for energy history tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyHistoryConfig {
    /// Maximum number of entries to keep
    pub max_entries: usize,
    /// Window size for trend calculation
    pub trend_window: usize,
    /// Threshold for persistence detection (seconds)
    pub persistence_window_secs: u64,
    /// Number of standard deviations for anomaly detection
    pub anomaly_sigma: f32,
    /// Minimum entries before trend analysis
    pub min_entries: usize,
}

impl Default for EnergyHistoryConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            trend_window: 10,
            persistence_window_secs: 300, // 5 minutes
            anomaly_sigma: 3.0,
            min_entries: 5,
        }
    }
}

/// Direction of energy trend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    /// Energy is increasing
    Increasing,
    /// Energy is decreasing
    Decreasing,
    /// Energy is relatively stable
    Stable,
    /// Not enough data to determine trend
    Unknown,
}

/// Result of trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyTrend {
    /// Direction of the trend
    pub direction: TrendDirection,
    /// Slope of the trend line (energy units per second)
    pub slope: f32,
    /// R-squared value indicating trend fit quality
    pub r_squared: f32,
    /// Average energy in the window
    pub mean: f32,
    /// Standard deviation in the window
    pub std_dev: f32,
    /// Window size used
    pub window_size: usize,
}

impl EnergyTrend {
    /// Check if the trend is concerning (increasing significantly)
    pub fn is_concerning(&self, threshold: f32) -> bool {
        self.direction == TrendDirection::Increasing && self.slope > threshold
    }

    /// Check if the trend is improving
    pub fn is_improving(&self) -> bool {
        self.direction == TrendDirection::Decreasing && self.r_squared > 0.5
    }
}

/// An entry in the energy history
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryEntry {
    /// Energy value
    energy: f32,
    /// Timestamp
    timestamp: DateTime<Utc>,
    /// Whether this was an anomaly
    is_anomaly: bool,
}

/// Time-series tracker for coherence energy
#[derive(Debug)]
pub struct EnergyHistory {
    /// Configuration
    config: EnergyHistoryConfig,
    /// History entries
    entries: VecDeque<HistoryEntry>,
    /// Running sum for efficient mean calculation
    running_sum: f64,
    /// Running sum of squares for efficient variance
    running_sum_sq: f64,
    /// Last computed trend
    last_trend: Option<EnergyTrend>,
    /// Statistics
    total_entries: u64,
    anomaly_count: u64,
}

impl EnergyHistory {
    /// Create a new energy history tracker
    pub fn new(config: EnergyHistoryConfig) -> Self {
        Self {
            config,
            entries: VecDeque::new(),
            running_sum: 0.0,
            running_sum_sq: 0.0,
            last_trend: None,
            total_entries: 0,
            anomaly_count: 0,
        }
    }

    /// Record a new energy value
    pub fn record(&mut self, energy: f32) {
        self.record_at(energy, Utc::now());
    }

    /// Record an energy value at a specific time
    pub fn record_at(&mut self, energy: f32, timestamp: DateTime<Utc>) {
        // Check for anomaly before updating stats
        let is_anomaly = self.is_anomaly(energy);
        if is_anomaly {
            self.anomaly_count += 1;
        }

        // Create entry
        let entry = HistoryEntry {
            energy,
            timestamp,
            is_anomaly,
        };

        // Update running statistics
        self.running_sum += energy as f64;
        self.running_sum_sq += (energy as f64) * (energy as f64);

        // Add to history
        self.entries.push_back(entry);
        self.total_entries += 1;

        // Trim if necessary
        while self.entries.len() > self.config.max_entries {
            if let Some(old) = self.entries.pop_front() {
                self.running_sum -= old.energy as f64;
                self.running_sum_sq -= (old.energy as f64) * (old.energy as f64);
            }
        }

        // Invalidate cached trend
        self.last_trend = None;
    }

    /// Get the current energy value
    pub fn current(&self) -> Option<f32> {
        self.entries.back().map(|e| e.energy)
    }

    /// Get the previous energy value
    pub fn previous(&self) -> Option<f32> {
        if self.entries.len() >= 2 {
            self.entries.get(self.entries.len() - 2).map(|e| e.energy)
        } else {
            None
        }
    }

    /// Get the change from previous to current
    pub fn delta(&self) -> Option<f32> {
        match (self.current(), self.previous()) {
            (Some(curr), Some(prev)) => Some(curr - prev),
            _ => None,
        }
    }

    /// Get the mean energy
    pub fn mean(&self) -> f32 {
        if self.entries.is_empty() {
            0.0
        } else {
            (self.running_sum / self.entries.len() as f64) as f32
        }
    }

    /// Get the standard deviation
    pub fn std_dev(&self) -> f32 {
        let n = self.entries.len();
        if n < 2 {
            return 0.0;
        }

        let mean = self.running_sum / n as f64;
        let variance = (self.running_sum_sq / n as f64) - (mean * mean);

        if variance > 0.0 {
            (variance.sqrt()) as f32
        } else {
            0.0
        }
    }

    /// Get the minimum energy value
    pub fn min(&self) -> Option<f32> {
        self.entries
            .iter()
            .map(|e| e.energy)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get the maximum energy value
    pub fn max(&self) -> Option<f32> {
        self.entries
            .iter()
            .map(|e| e.energy)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Compute the current trend
    pub fn trend(&mut self) -> EnergyTrend {
        if let Some(ref trend) = self.last_trend {
            return trend.clone();
        }

        let trend = self.compute_trend();
        self.last_trend = Some(trend.clone());
        trend
    }

    /// Check if energy has been above threshold for persistence window
    pub fn is_above_threshold_persistent(&self, threshold: f32) -> bool {
        let window = Duration::seconds(self.config.persistence_window_secs as i64);
        let cutoff = Utc::now() - window;

        // Check all entries within the persistence window
        let recent: Vec<_> = self
            .entries
            .iter()
            .rev()
            .take_while(|e| e.timestamp >= cutoff)
            .collect();

        if recent.is_empty() {
            return false;
        }

        // All entries must be above threshold
        recent.iter().all(|e| e.energy > threshold)
    }

    /// Check if energy has been below threshold for persistence window
    pub fn is_below_threshold_persistent(&self, threshold: f32) -> bool {
        let window = Duration::seconds(self.config.persistence_window_secs as i64);
        let cutoff = Utc::now() - window;

        let recent: Vec<_> = self
            .entries
            .iter()
            .rev()
            .take_while(|e| e.timestamp >= cutoff)
            .collect();

        if recent.is_empty() {
            return false;
        }

        recent.iter().all(|e| e.energy < threshold)
    }

    /// Get entries in the persistence window
    pub fn recent_entries(&self, seconds: u64) -> Vec<(f32, DateTime<Utc>)> {
        let window = Duration::seconds(seconds as i64);
        let cutoff = Utc::now() - window;

        self.entries
            .iter()
            .rev()
            .take_while(|e| e.timestamp >= cutoff)
            .map(|e| (e.energy, e.timestamp))
            .collect()
    }

    /// Get the number of entries
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if history is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get total entries ever recorded
    #[inline]
    pub fn total_entries(&self) -> u64 {
        self.total_entries
    }

    /// Get anomaly count
    #[inline]
    pub fn anomaly_count(&self) -> u64 {
        self.anomaly_count
    }

    /// Get anomaly rate
    pub fn anomaly_rate(&self) -> f32 {
        if self.total_entries > 0 {
            self.anomaly_count as f32 / self.total_entries as f32
        } else {
            0.0
        }
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.entries.clear();
        self.running_sum = 0.0;
        self.running_sum_sq = 0.0;
        self.last_trend = None;
    }

    // Private methods

    fn is_anomaly(&self, energy: f32) -> bool {
        if self.entries.len() < self.config.min_entries {
            return false;
        }

        let mean = self.mean();
        let std_dev = self.std_dev();

        if std_dev < 1e-10 {
            return false;
        }

        let z_score = ((energy - mean) / std_dev).abs();
        z_score > self.config.anomaly_sigma
    }

    fn compute_trend(&self) -> EnergyTrend {
        let window_size = self.config.trend_window.min(self.entries.len());

        if window_size < self.config.min_entries {
            return EnergyTrend {
                direction: TrendDirection::Unknown,
                slope: 0.0,
                r_squared: 0.0,
                mean: self.mean(),
                std_dev: self.std_dev(),
                window_size,
            };
        }

        // Get recent entries
        let recent: Vec<_> = self.entries.iter().rev().take(window_size).collect();

        // Linear regression: y = mx + b
        // x is the index, y is the energy value
        let n = recent.len() as f64;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_xx = 0.0;

        for (i, entry) in recent.iter().rev().enumerate() {
            let x = i as f64;
            let y = entry.energy as f64;
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_xx += x * x;
        }

        // Compute slope
        let slope = if (n * sum_xx - sum_x * sum_x).abs() > 1e-10 {
            ((n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x)) as f32
        } else {
            0.0
        };

        // Compute R-squared
        let mean_y = sum_y / n;
        let mut ss_tot = 0.0;
        let mut ss_res = 0.0;

        let b = (sum_y - slope as f64 * sum_x) / n;

        for (i, entry) in recent.iter().rev().enumerate() {
            let x = i as f64;
            let y = entry.energy as f64;
            let y_pred = slope as f64 * x + b;

            ss_tot += (y - mean_y).powi(2);
            ss_res += (y - y_pred).powi(2);
        }

        let r_squared = if ss_tot > 1e-10 {
            (1.0 - ss_res / ss_tot) as f32
        } else {
            0.0
        };

        // Determine direction
        let direction = if slope.abs() < 0.001 {
            TrendDirection::Stable
        } else if slope > 0.0 {
            TrendDirection::Increasing
        } else {
            TrendDirection::Decreasing
        };

        // Compute window stats
        let window_sum: f64 = recent.iter().map(|e| e.energy as f64).sum();
        let window_mean = (window_sum / n) as f32;

        let window_var: f64 = recent
            .iter()
            .map(|e| {
                let diff = e.energy as f64 - window_sum / n;
                diff * diff
            })
            .sum::<f64>()
            / n;
        let window_std_dev = (window_var.sqrt()) as f32;

        EnergyTrend {
            direction,
            slope,
            r_squared,
            mean: window_mean,
            std_dev: window_std_dev,
            window_size,
        }
    }
}

impl Default for EnergyHistory {
    fn default() -> Self {
        Self::new(EnergyHistoryConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_creation() {
        let history = EnergyHistory::default();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_record_energy() {
        let mut history = EnergyHistory::default();

        history.record(1.0);
        history.record(2.0);
        history.record(3.0);

        assert_eq!(history.len(), 3);
        assert_eq!(history.current(), Some(3.0));
        assert_eq!(history.previous(), Some(2.0));
        assert_eq!(history.delta(), Some(1.0));
    }

    #[test]
    fn test_statistics() {
        let mut history = EnergyHistory::default();

        history.record(1.0);
        history.record(2.0);
        history.record(3.0);
        history.record(4.0);
        history.record(5.0);

        assert_eq!(history.mean(), 3.0);
        assert_eq!(history.min(), Some(1.0));
        assert_eq!(history.max(), Some(5.0));
    }

    #[test]
    fn test_trend_increasing() {
        let mut history = EnergyHistory::new(EnergyHistoryConfig {
            min_entries: 3,
            trend_window: 5,
            ..Default::default()
        });

        for i in 0..10 {
            history.record(i as f32);
        }

        let trend = history.trend();
        assert_eq!(trend.direction, TrendDirection::Increasing);
        assert!(trend.slope > 0.0);
    }

    #[test]
    fn test_trend_decreasing() {
        let mut history = EnergyHistory::new(EnergyHistoryConfig {
            min_entries: 3,
            trend_window: 5,
            ..Default::default()
        });

        for i in (0..10).rev() {
            history.record(i as f32);
        }

        let trend = history.trend();
        assert_eq!(trend.direction, TrendDirection::Decreasing);
        assert!(trend.slope < 0.0);
    }

    #[test]
    fn test_trend_stable() {
        let mut history = EnergyHistory::new(EnergyHistoryConfig {
            min_entries: 3,
            trend_window: 5,
            ..Default::default()
        });

        for _ in 0..10 {
            history.record(5.0);
        }

        let trend = history.trend();
        assert_eq!(trend.direction, TrendDirection::Stable);
        assert!(trend.slope.abs() < 0.01);
    }

    #[test]
    fn test_anomaly_detection() {
        let config = EnergyHistoryConfig {
            anomaly_sigma: 2.0,
            min_entries: 5,
            ..Default::default()
        };
        let mut history = EnergyHistory::new(config);

        // Add normal values
        for _ in 0..10 {
            history.record(5.0);
        }

        // Add anomaly
        history.record(100.0);

        assert!(history.anomaly_count() > 0);
    }

    #[test]
    fn test_history_trimming() {
        let config = EnergyHistoryConfig {
            max_entries: 5,
            ..Default::default()
        };
        let mut history = EnergyHistory::new(config);

        for i in 0..10 {
            history.record(i as f32);
        }

        assert_eq!(history.len(), 5);
        assert_eq!(history.total_entries(), 10);
        // Oldest entries should be trimmed
        assert_eq!(history.min(), Some(5.0));
    }

    #[test]
    fn test_clear() {
        let mut history = EnergyHistory::default();

        history.record(1.0);
        history.record(2.0);
        history.clear();

        assert!(history.is_empty());
        assert_eq!(history.current(), None);
    }
}
