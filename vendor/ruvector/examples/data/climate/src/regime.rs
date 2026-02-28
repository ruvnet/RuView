//! Regime shift detection using RuVector's min-cut algorithms

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{ClimateObservation, SensorNetwork, SensorEdge, WeatherVariable};

/// A detected regime shift
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeShift {
    /// Shift identifier
    pub id: String,

    /// Timestamp when shift was detected
    pub timestamp: DateTime<Utc>,

    /// Shift type
    pub shift_type: ShiftType,

    /// Shift severity
    pub severity: ShiftSeverity,

    /// Min-cut value before shift
    pub mincut_before: f64,

    /// Min-cut value after shift
    pub mincut_after: f64,

    /// Change magnitude
    pub magnitude: f64,

    /// Affected sensor IDs
    pub affected_sensors: Vec<String>,

    /// Geographic center of shift (lat, lon)
    pub center: Option<(f64, f64)>,

    /// Radius of effect (km)
    pub radius_km: Option<f64>,

    /// Primary variable affected
    pub primary_variable: WeatherVariable,

    /// Confidence score (0-1)
    pub confidence: f64,

    /// Evidence supporting the detection
    pub evidence: Vec<ShiftEvidence>,

    /// Interpretation of the shift
    pub interpretation: String,
}

/// Type of regime shift
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShiftType {
    /// Network fragmentation (min-cut decreased significantly)
    Fragmentation,

    /// Network consolidation (min-cut increased)
    Consolidation,

    /// Localized disruption (subset of sensors)
    LocalizedDisruption,

    /// Global pattern change
    GlobalPatternChange,

    /// Seasonal transition
    SeasonalTransition,

    /// Unknown type
    Unknown,
}

/// Severity of regime shift
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub enum ShiftSeverity {
    /// Minor shift, might be noise
    Minor,

    /// Moderate shift, notable
    Moderate,

    /// Major shift, significant
    Major,

    /// Extreme shift, exceptional
    Extreme,
}

impl ShiftSeverity {
    /// Convert from magnitude
    pub fn from_magnitude(magnitude: f64) -> Self {
        if magnitude < 0.1 {
            ShiftSeverity::Minor
        } else if magnitude < 0.3 {
            ShiftSeverity::Moderate
        } else if magnitude < 0.5 {
            ShiftSeverity::Major
        } else {
            ShiftSeverity::Extreme
        }
    }
}

/// Evidence for a regime shift
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShiftEvidence {
    /// Evidence type
    pub evidence_type: String,

    /// Numeric value
    pub value: f64,

    /// Explanation
    pub explanation: String,
}

/// Regime shift detector using RuVector's min-cut
pub struct RegimeShiftDetector {
    /// Configuration
    config: RegimeDetectorConfig,

    /// Historical min-cut values
    mincut_history: Vec<(DateTime<Utc>, f64)>,

    /// Historical partition info
    partition_history: Vec<(DateTime<Utc>, Vec<String>, Vec<String>)>,

    /// Detected shifts
    detected_shifts: Vec<RegimeShift>,
}

/// Configuration for regime detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeDetectorConfig {
    /// Window size (hours)
    pub window_hours: u32,

    /// Slide step (hours)
    pub slide_hours: u32,

    /// Minimum change threshold for detection
    pub detection_threshold: f64,

    /// Use approximate min-cut
    pub approximate: bool,

    /// Approximation epsilon
    pub epsilon: f64,

    /// Minimum sensors for valid detection
    pub min_sensors: usize,

    /// Lookback windows for trend analysis
    pub lookback_windows: usize,
}

impl Default for RegimeDetectorConfig {
    fn default() -> Self {
        Self {
            window_hours: 168, // 1 week
            slide_hours: 24,   // 1 day
            detection_threshold: 0.15,
            approximate: true,
            epsilon: 0.1,
            min_sensors: 5,
            lookback_windows: 10,
        }
    }
}

impl RegimeShiftDetector {
    /// Create a new regime shift detector
    pub fn new(config: RegimeDetectorConfig) -> Self {
        Self {
            config,
            mincut_history: Vec::new(),
            partition_history: Vec::new(),
            detected_shifts: Vec::new(),
        }
    }

    /// Detect regime shifts in a sensor network over time
    ///
    /// This integrates with RuVector's min-cut algorithms to:
    /// 1. Build dynamic correlation graphs from observations
    /// 2. Compute min-cut values over sliding windows
    /// 3. Detect significant changes indicating regime shifts
    pub fn detect(
        &mut self,
        base_network: &SensorNetwork,
        observations: &[ClimateObservation],
    ) -> Vec<RegimeShift> {
        if observations.is_empty() || base_network.nodes.len() < self.config.min_sensors {
            return vec![];
        }

        // Sort observations by time
        let mut sorted_obs = observations.to_vec();
        sorted_obs.sort_by_key(|o| o.timestamp);

        // Slide window over time
        let window_duration = chrono::Duration::hours(self.config.window_hours as i64);
        let slide_duration = chrono::Duration::hours(self.config.slide_hours as i64);

        let start_time = sorted_obs.first().unwrap().timestamp;
        let end_time = sorted_obs.last().unwrap().timestamp;

        let mut current_start = start_time;
        let mut shift_counter = 0;

        while current_start + window_duration <= end_time {
            let window_end = current_start + window_duration;

            // Get observations in window
            let window_obs: Vec<_> = sorted_obs
                .iter()
                .filter(|o| o.timestamp >= current_start && o.timestamp < window_end)
                .cloned()
                .collect();

            if window_obs.len() >= self.config.min_sensors * 10 {
                // Build network from window observations
                let window_network = self.build_window_network(base_network, &window_obs);

                // Compute min-cut
                let (mincut_value, partition) = self.compute_mincut(&window_network);

                self.mincut_history.push((current_start, mincut_value));
                if let Some((side_a, side_b)) = partition {
                    self.partition_history.push((current_start, side_a, side_b));
                }

                // Check for regime shift
                if self.mincut_history.len() >= 2 {
                    let prev_mincut = self.mincut_history[self.mincut_history.len() - 2].1;
                    let delta = (mincut_value - prev_mincut) / prev_mincut.max(0.01);

                    if delta.abs() > self.config.detection_threshold {
                        let shift = self.create_shift_record(
                            &format!("shift_{}", shift_counter),
                            current_start,
                            prev_mincut,
                            mincut_value,
                            delta,
                            &window_network,
                            &window_obs,
                        );
                        self.detected_shifts.push(shift);
                        shift_counter += 1;
                    }
                }
            }

            current_start = current_start + slide_duration;
        }

        self.detected_shifts.clone()
    }

    /// Build network from window observations
    fn build_window_network(
        &self,
        base_network: &SensorNetwork,
        observations: &[ClimateObservation],
    ) -> SensorNetwork {
        let mut network = base_network.clone();

        // Update edge weights based on observation correlations
        let mut station_values: HashMap<&str, Vec<(DateTime<Utc>, f64)>> = HashMap::new();

        for obs in observations {
            station_values
                .entry(&obs.station_id)
                .or_default()
                .push((obs.timestamp, obs.value));
        }

        // Recompute correlations
        network.edges.clear();

        let station_ids: Vec<_> = station_values.keys().cloned().collect();

        for i in 0..station_ids.len() {
            for j in (i + 1)..station_ids.len() {
                let id_i = station_ids[i];
                let id_j = station_ids[j];

                let vals_i = &station_values[id_i];
                let vals_j = &station_values[id_j];

                let correlation = self.compute_correlation(vals_i, vals_j);

                if correlation.abs() > 0.3 {
                    network.add_edge(SensorEdge {
                        source: id_i.to_string(),
                        target: id_j.to_string(),
                        correlation,
                        distance_km: 0.0, // Would compute from locations
                        weight: correlation.abs(),
                        variables: vec![],
                        overlap_count: vals_i.len().min(vals_j.len()),
                    });
                }
            }
        }

        network
    }

    /// Compute correlation between two time series
    fn compute_correlation(&self, a: &[(DateTime<Utc>, f64)], b: &[(DateTime<Utc>, f64)]) -> f64 {
        // Build time-indexed maps (daily resolution)
        let mut map_a: HashMap<i64, f64> = HashMap::new();
        let mut map_b: HashMap<i64, f64> = HashMap::new();

        for (ts, val) in a {
            let day = ts.timestamp() / 86400;
            map_a.insert(day, *val);
        }

        for (ts, val) in b {
            let day = ts.timestamp() / 86400;
            map_b.insert(day, *val);
        }

        // Find overlapping days
        let mut vals_a = Vec::new();
        let mut vals_b = Vec::new();

        for (day, val_a) in &map_a {
            if let Some(&val_b) = map_b.get(day) {
                vals_a.push(*val_a);
                vals_b.push(val_b);
            }
        }

        if vals_a.len() < 3 {
            return 0.0;
        }

        // Pearson correlation
        let n = vals_a.len();
        let mean_a = vals_a.iter().sum::<f64>() / n as f64;
        let mean_b = vals_b.iter().sum::<f64>() / n as f64;

        let mut cov = 0.0;
        let mut var_a = 0.0;
        let mut var_b = 0.0;

        for i in 0..n {
            let da = vals_a[i] - mean_a;
            let db = vals_b[i] - mean_b;
            cov += da * db;
            var_a += da * da;
            var_b += db * db;
        }

        if var_a * var_b > 0.0 {
            cov / (var_a.sqrt() * var_b.sqrt())
        } else {
            0.0
        }
    }

    /// Compute min-cut for network
    ///
    /// Uses RuVector's min-cut algorithms when available
    fn compute_mincut(&self, network: &SensorNetwork) -> (f64, Option<(Vec<String>, Vec<String>)>) {
        // Convert to min-cut format
        let edges = network.to_mincut_edges();
        let node_mapping = network.node_id_mapping();

        if edges.is_empty() {
            return (0.0, None);
        }

        // Simplified min-cut computation for demo
        // In production, use ruvector_mincut::MinCutBuilder
        let total_weight: f64 = edges.iter().map(|(_, _, w)| w).sum();
        let avg_degree = (2.0 * edges.len() as f64) / node_mapping.len() as f64;

        let approx_mincut = if edges.is_empty() {
            0.0
        } else {
            total_weight / avg_degree.max(1.0)
        };

        // Simple partition (would use actual min-cut partition)
        let all_nodes: Vec<String> = node_mapping.values().cloned().collect();
        let mid = all_nodes.len() / 2;
        let side_a = all_nodes[..mid].to_vec();
        let side_b = all_nodes[mid..].to_vec();

        (approx_mincut, Some((side_a, side_b)))
    }

    /// Create a regime shift record
    fn create_shift_record(
        &self,
        id: &str,
        timestamp: DateTime<Utc>,
        mincut_before: f64,
        mincut_after: f64,
        delta: f64,
        network: &SensorNetwork,
        observations: &[ClimateObservation],
    ) -> RegimeShift {
        let magnitude = delta.abs();
        let severity = ShiftSeverity::from_magnitude(magnitude);

        let shift_type = if delta < -0.3 {
            ShiftType::Fragmentation
        } else if delta > 0.3 {
            ShiftType::Consolidation
        } else if network.nodes.len() < 10 {
            ShiftType::LocalizedDisruption
        } else {
            ShiftType::GlobalPatternChange
        };

        // Find affected sensors (those with high observation variance)
        let affected_sensors = self.find_affected_sensors(network, observations);

        // Compute center
        let center = self.compute_geographic_center(&affected_sensors, network);

        // Primary variable
        let primary_variable = observations
            .first()
            .map(|o| o.variable)
            .unwrap_or(WeatherVariable::Temperature);

        // Compute confidence based on evidence
        let confidence = self.compute_confidence(magnitude, network.nodes.len(), observations.len());

        // Build evidence
        let evidence = vec![
            ShiftEvidence {
                evidence_type: "mincut_change".to_string(),
                value: delta,
                explanation: format!(
                    "Min-cut {} by {:.1}%",
                    if delta > 0.0 { "increased" } else { "decreased" },
                    delta.abs() * 100.0
                ),
            },
            ShiftEvidence {
                evidence_type: "affected_sensors".to_string(),
                value: affected_sensors.len() as f64,
                explanation: format!("{} sensors significantly affected", affected_sensors.len()),
            },
            ShiftEvidence {
                evidence_type: "network_size".to_string(),
                value: network.nodes.len() as f64,
                explanation: format!("Network has {} sensors", network.nodes.len()),
            },
        ];

        let interpretation = self.interpret_shift(shift_type, severity, &affected_sensors);

        RegimeShift {
            id: id.to_string(),
            timestamp,
            shift_type,
            severity,
            mincut_before,
            mincut_after,
            magnitude,
            affected_sensors,
            center,
            radius_km: Some(100.0), // Would compute from sensor positions
            primary_variable,
            confidence,
            evidence,
            interpretation,
        }
    }

    /// Find affected sensors
    fn find_affected_sensors(
        &self,
        network: &SensorNetwork,
        observations: &[ClimateObservation],
    ) -> Vec<String> {
        let mut station_stats: HashMap<&str, (f64, f64, usize)> = HashMap::new(); // (sum, sum_sq, count)

        for obs in observations {
            let entry = station_stats
                .entry(&obs.station_id)
                .or_insert((0.0, 0.0, 0));
            entry.0 += obs.value;
            entry.1 += obs.value * obs.value;
            entry.2 += 1;
        }

        // Compute variance for each station
        let mut variances: Vec<(&str, f64)> = station_stats
            .iter()
            .filter(|(_, (_, _, count))| *count >= 3)
            .map(|(id, (sum, sum_sq, count))| {
                let mean = sum / *count as f64;
                let variance = sum_sq / *count as f64 - mean * mean;
                (*id, variance)
            })
            .collect();

        // Return stations with above-average variance
        let avg_variance: f64 = variances.iter().map(|(_, v)| v).sum::<f64>()
            / variances.len().max(1) as f64;

        variances
            .iter()
            .filter(|(_, v)| *v > avg_variance * 1.5)
            .map(|(id, _)| id.to_string())
            .collect()
    }

    /// Compute geographic center
    fn compute_geographic_center(
        &self,
        sensor_ids: &[String],
        network: &SensorNetwork,
    ) -> Option<(f64, f64)> {
        if sensor_ids.is_empty() {
            return None;
        }

        let mut sum_lat = 0.0;
        let mut sum_lon = 0.0;
        let mut count = 0;

        for id in sensor_ids {
            if let Some(node) = network.get_node(id) {
                sum_lat += node.location.0;
                sum_lon += node.location.1;
                count += 1;
            }
        }

        if count > 0 {
            Some((sum_lat / count as f64, sum_lon / count as f64))
        } else {
            None
        }
    }

    /// Compute confidence score
    fn compute_confidence(&self, magnitude: f64, sensor_count: usize, obs_count: usize) -> f64 {
        let magnitude_score = (magnitude.min(1.0)).max(0.0);
        let sensor_score = (sensor_count as f64 / 50.0).min(1.0);
        let obs_score = (obs_count as f64 / 1000.0).min(1.0);

        (magnitude_score * 0.4 + sensor_score * 0.3 + obs_score * 0.3).min(1.0)
    }

    /// Interpret the shift
    fn interpret_shift(
        &self,
        shift_type: ShiftType,
        severity: ShiftSeverity,
        affected_sensors: &[String],
    ) -> String {
        let severity_str = match severity {
            ShiftSeverity::Minor => "Minor",
            ShiftSeverity::Moderate => "Moderate",
            ShiftSeverity::Major => "Major",
            ShiftSeverity::Extreme => "Extreme",
        };

        let type_str = match shift_type {
            ShiftType::Fragmentation => "network fragmentation (decreased correlation)",
            ShiftType::Consolidation => "network consolidation (increased correlation)",
            ShiftType::LocalizedDisruption => "localized weather pattern disruption",
            ShiftType::GlobalPatternChange => "large-scale pattern change",
            ShiftType::SeasonalTransition => "seasonal transition",
            ShiftType::Unknown => "undetermined regime change",
        };

        format!(
            "{} {} detected affecting {} sensors",
            severity_str,
            type_str,
            affected_sensors.len()
        )
    }

    /// Get min-cut history
    pub fn mincut_history(&self) -> &[(DateTime<Utc>, f64)] {
        &self.mincut_history
    }

    /// Get detected shifts
    pub fn detected_shifts(&self) -> &[RegimeShift] {
        &self.detected_shifts
    }

    /// Get shifts by severity
    pub fn shifts_by_severity(&self, min_severity: ShiftSeverity) -> Vec<&RegimeShift> {
        self.detected_shifts
            .iter()
            .filter(|s| s.severity >= min_severity)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shift_severity() {
        assert_eq!(ShiftSeverity::from_magnitude(0.05), ShiftSeverity::Minor);
        assert_eq!(ShiftSeverity::from_magnitude(0.2), ShiftSeverity::Moderate);
        assert_eq!(ShiftSeverity::from_magnitude(0.4), ShiftSeverity::Major);
        assert_eq!(ShiftSeverity::from_magnitude(0.6), ShiftSeverity::Extreme);
    }

    #[test]
    fn test_detector_creation() {
        let config = RegimeDetectorConfig::default();
        let detector = RegimeShiftDetector::new(config);
        assert!(detector.detected_shifts().is_empty());
    }
}
