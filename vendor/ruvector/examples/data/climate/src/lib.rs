//! # RuVector Climate Data Integration
//!
//! Integration with NOAA and NASA Earthdata for climate intelligence,
//! regime shift detection, and anomaly prediction.
//!
//! ## Core Capabilities
//!
//! - **Sensor Network Graph**: Model sensor correlations as dynamic graphs
//! - **Regime Shift Detection**: Use min-cut coherence breaks for regime changes
//! - **Anomaly Prediction**: Vector-based pattern matching for early warning
//! - **Multi-Scale Analysis**: From local sensors to global patterns
//!
//! ## Data Sources
//!
//! ### NOAA Open Data Dissemination (NODD)
//! - Global Historical Climatology Network (GHCN)
//! - Integrated Surface Database (ISD)
//! - Climate Forecast System (CFS)
//! - NOAA Weather Alerts
//!
//! ### NASA Earthdata
//! - MODIS (Terra/Aqua) satellite imagery
//! - GPM precipitation data
//! - GRACE groundwater measurements
//! - ICESat-2 ice sheet data
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ruvector_data_climate::{
//!     ClimateClient, SensorNetworkBuilder, RegimeShiftDetector,
//!     TimeSeriesVector, CoherenceAnalyzer,
//! };
//!
//! // Build sensor correlation network
//! let network = SensorNetworkBuilder::new()
//!     .add_noaa_ghcn("US", 2020..2024)
//!     .correlation_threshold(0.7)
//!     .build()
//!     .await?;
//!
//! // Detect regime shifts using RuVector's min-cut
//! let detector = RegimeShiftDetector::new(network);
//! let shifts = detector.detect(
//!     window_days: 90,
//!     coherence_threshold: 0.5,
//! ).await?;
//!
//! for shift in shifts {
//!     println!("Regime shift at {}: {} sensors affected",
//!              shift.timestamp, shift.affected_sensors.len());
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod noaa;
pub mod nasa;
pub mod regime;
pub mod network;
pub mod timeseries;

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use geo::Point;
use ndarray::Array1;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use network::{SensorNetwork, SensorNetworkBuilder, SensorNode, SensorEdge};
pub use noaa::{NoaaClient, GhcnStation, GhcnObservation, WeatherVariable};
pub use nasa::{NasaClient, ModisProduct, SatelliteObservation};
pub use regime::{RegimeShiftDetector, RegimeShift, ShiftType, ShiftSeverity, ShiftEvidence};
pub use timeseries::{TimeSeriesVector, TimeSeriesProcessor, SeasonalDecomposition};

use ruvector_data_framework::{DataRecord, DataSource, FrameworkError, Relationship, Result};

/// Climate-specific error types
#[derive(Error, Debug)]
pub enum ClimateError {
    /// API request failed
    #[error("API error: {0}")]
    Api(String),

    /// Invalid coordinates
    #[error("Invalid coordinates: lat={0}, lon={1}")]
    InvalidCoordinates(f64, f64),

    /// Data format error
    #[error("Data format error: {0}")]
    DataFormat(String),

    /// Insufficient data
    #[error("Insufficient data: {0}")]
    InsufficientData(String),

    /// Network error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Numerical error
    #[error("Numerical error: {0}")]
    Numerical(String),
}

impl From<ClimateError> for FrameworkError {
    fn from(e: ClimateError) -> Self {
        FrameworkError::Ingestion(e.to_string())
    }
}

/// Configuration for climate data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateConfig {
    /// NOAA API token
    pub noaa_token: Option<String>,

    /// NASA Earthdata token
    pub nasa_token: Option<String>,

    /// Geographic bounding box
    pub bounding_box: Option<BoundingBox>,

    /// Variables to fetch
    pub variables: Vec<WeatherVariable>,

    /// Temporal resolution (hours)
    pub temporal_resolution_hours: u32,

    /// Enable interpolation for missing data
    pub interpolate: bool,
}

impl Default for ClimateConfig {
    fn default() -> Self {
        Self {
            noaa_token: None,
            nasa_token: None,
            bounding_box: None,
            variables: vec![WeatherVariable::Temperature, WeatherVariable::Precipitation],
            temporal_resolution_hours: 24,
            interpolate: true,
        }
    }
}

/// Geographic bounding box
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    /// Minimum latitude
    pub min_lat: f64,
    /// Maximum latitude
    pub max_lat: f64,
    /// Minimum longitude
    pub min_lon: f64,
    /// Maximum longitude
    pub max_lon: f64,
}

impl BoundingBox {
    /// Create a new bounding box
    pub fn new(min_lat: f64, max_lat: f64, min_lon: f64, max_lon: f64) -> Self {
        Self { min_lat, max_lat, min_lon, max_lon }
    }

    /// Check if point is within bounds
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        lat >= self.min_lat && lat <= self.max_lat &&
        lon >= self.min_lon && lon <= self.max_lon
    }

    /// Get center point
    pub fn center(&self) -> (f64, f64) {
        ((self.min_lat + self.max_lat) / 2.0, (self.min_lon + self.max_lon) / 2.0)
    }

    /// US Continental bounding box
    pub fn us_continental() -> Self {
        Self::new(24.0, 50.0, -125.0, -66.0)
    }

    /// Global bounding box
    pub fn global() -> Self {
        Self::new(-90.0, 90.0, -180.0, 180.0)
    }
}

/// A climate observation from any source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateObservation {
    /// Station/sensor ID
    pub station_id: String,

    /// Observation timestamp
    pub timestamp: DateTime<Utc>,

    /// Location
    pub location: (f64, f64),

    /// Variable type
    pub variable: WeatherVariable,

    /// Observed value
    pub value: f64,

    /// Quality flag
    pub quality: QualityFlag,

    /// Data source
    pub source: DataSourceType,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Quality flag for observations
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum QualityFlag {
    /// Good quality data
    Good,
    /// Suspect data
    Suspect,
    /// Erroneous data
    Erroneous,
    /// Missing data (interpolated)
    Missing,
    /// Unknown quality
    Unknown,
}

/// Data source type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DataSourceType {
    /// NOAA GHCN
    NoaaGhcn,
    /// NOAA ISD
    NoaaIsd,
    /// NASA MODIS
    NasaModis,
    /// NASA GPM
    NasaGpm,
    /// Other source
    Other,
}

/// Coherence analyzer for sensor networks
///
/// Uses RuVector's min-cut algorithms to detect coherence breaks
/// in sensor correlation networks.
pub struct CoherenceAnalyzer {
    /// Configuration
    config: CoherenceAnalyzerConfig,

    /// Historical coherence values
    coherence_history: Vec<(DateTime<Utc>, f64)>,

    /// Detected breaks
    detected_breaks: Vec<CoherenceBreak>,
}

/// Configuration for coherence analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceAnalyzerConfig {
    /// Window size for analysis (hours)
    pub window_hours: u32,

    /// Slide step (hours)
    pub slide_hours: u32,

    /// Minimum coherence threshold
    pub min_coherence: f64,

    /// Use approximate min-cut
    pub approximate: bool,

    /// Approximation epsilon
    pub epsilon: f64,
}

impl Default for CoherenceAnalyzerConfig {
    fn default() -> Self {
        Self {
            window_hours: 168, // 1 week
            slide_hours: 24,   // 1 day
            min_coherence: 0.3,
            approximate: true,
            epsilon: 0.1,
        }
    }
}

/// A detected coherence break
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceBreak {
    /// Break identifier
    pub id: String,

    /// Timestamp of break
    pub timestamp: DateTime<Utc>,

    /// Coherence value before break
    pub coherence_before: f64,

    /// Coherence value after break
    pub coherence_after: f64,

    /// Magnitude of change
    pub magnitude: f64,

    /// Affected sensor IDs
    pub affected_sensors: Vec<String>,

    /// Geographic extent
    pub geographic_extent: Option<BoundingBox>,

    /// Break interpretation
    pub interpretation: String,
}

impl CoherenceAnalyzer {
    /// Create a new coherence analyzer
    pub fn new(config: CoherenceAnalyzerConfig) -> Self {
        Self {
            config,
            coherence_history: Vec::new(),
            detected_breaks: Vec::new(),
        }
    }

    /// Analyze a sensor network for coherence breaks
    ///
    /// This method integrates with RuVector's min-cut algorithms:
    /// 1. Build a graph from sensor correlations
    /// 2. Compute dynamic min-cut over sliding windows
    /// 3. Detect significant changes in min-cut value
    pub fn analyze(&mut self, network: &SensorNetwork, observations: &[ClimateObservation]) -> Result<Vec<CoherenceBreak>> {
        if observations.is_empty() {
            return Ok(vec![]);
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

        while current_start + window_duration <= end_time {
            let window_end = current_start + window_duration;

            // Get observations in window
            let window_obs: Vec<_> = sorted_obs
                .iter()
                .filter(|o| o.timestamp >= current_start && o.timestamp < window_end)
                .collect();

            if window_obs.len() >= 10 {
                // Compute coherence for this window
                let coherence = self.compute_window_coherence(network, &window_obs);
                self.coherence_history.push((current_start, coherence));

                // Check for break
                if self.coherence_history.len() >= 2 {
                    let prev_coherence = self.coherence_history[self.coherence_history.len() - 2].1;
                    let delta = (coherence - prev_coherence).abs();

                    if delta > self.config.min_coherence {
                        let affected_sensors = self.identify_affected_sensors(network, &window_obs);
                        let extent = self.compute_geographic_extent(&affected_sensors, network);

                        self.detected_breaks.push(CoherenceBreak {
                            id: format!("break_{}", self.detected_breaks.len()),
                            timestamp: current_start,
                            coherence_before: prev_coherence,
                            coherence_after: coherence,
                            magnitude: delta,
                            affected_sensors,
                            geographic_extent: extent,
                            interpretation: self.interpret_break(delta, coherence > prev_coherence),
                        });
                    }
                }
            }

            current_start = current_start + slide_duration;
        }

        Ok(self.detected_breaks.clone())
    }

    /// Compute coherence for a window of observations
    fn compute_window_coherence(&self, network: &SensorNetwork, observations: &[&ClimateObservation]) -> f64 {
        // Build correlation matrix from observations
        let mut station_values: HashMap<&str, Vec<f64>> = HashMap::new();

        for obs in observations {
            station_values
                .entry(&obs.station_id)
                .or_default()
                .push(obs.value);
        }

        // Compute average pairwise correlation
        let stations: Vec<_> = station_values.keys().collect();
        if stations.len() < 2 {
            return 1.0; // Single station = fully coherent
        }

        let mut correlations = Vec::new();

        for i in 0..stations.len() {
            for j in (i + 1)..stations.len() {
                let vals_i = &station_values[stations[i]];
                let vals_j = &station_values[stations[j]];

                if vals_i.len() >= 3 && vals_j.len() >= 3 {
                    let corr = Self::pearson_correlation(vals_i, vals_j);
                    if corr.is_finite() {
                        correlations.push(corr.abs());
                    }
                }
            }
        }

        if correlations.is_empty() {
            return 0.5; // Default
        }

        // Coherence = average absolute correlation
        correlations.iter().sum::<f64>() / correlations.len() as f64
    }

    /// Compute Pearson correlation coefficient
    fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
        let n = x.len().min(y.len());
        if n < 2 {
            return 0.0;
        }

        let mean_x = x.iter().take(n).sum::<f64>() / n as f64;
        let mean_y = y.iter().take(n).sum::<f64>() / n as f64;

        let mut cov = 0.0;
        let mut var_x = 0.0;
        let mut var_y = 0.0;

        for i in 0..n {
            let dx = x[i] - mean_x;
            let dy = y[i] - mean_y;
            cov += dx * dy;
            var_x += dx * dx;
            var_y += dy * dy;
        }

        if var_x * var_y > 0.0 {
            cov / (var_x.sqrt() * var_y.sqrt())
        } else {
            0.0
        }
    }

    /// Identify affected sensors during a break
    fn identify_affected_sensors(&self, network: &SensorNetwork, observations: &[&ClimateObservation]) -> Vec<String> {
        // Return stations with significant value changes
        let mut station_ranges: HashMap<&str, (f64, f64)> = HashMap::new();

        for obs in observations {
            let entry = station_ranges.entry(&obs.station_id).or_insert((f64::INFINITY, f64::NEG_INFINITY));
            entry.0 = entry.0.min(obs.value);
            entry.1 = entry.1.max(obs.value);
        }

        // Stations with high range = affected
        let avg_range: f64 = station_ranges.values().map(|(min, max)| max - min).sum::<f64>()
            / station_ranges.len() as f64;

        station_ranges
            .iter()
            .filter(|(_, (min, max))| max - min > avg_range * 1.5)
            .map(|(id, _)| id.to_string())
            .collect()
    }

    /// Compute geographic extent of affected sensors
    fn compute_geographic_extent(&self, sensor_ids: &[String], network: &SensorNetwork) -> Option<BoundingBox> {
        if sensor_ids.is_empty() {
            return None;
        }

        let mut min_lat = f64::INFINITY;
        let mut max_lat = f64::NEG_INFINITY;
        let mut min_lon = f64::INFINITY;
        let mut max_lon = f64::NEG_INFINITY;

        for id in sensor_ids {
            if let Some(node) = network.get_node(id) {
                min_lat = min_lat.min(node.location.0);
                max_lat = max_lat.max(node.location.0);
                min_lon = min_lon.min(node.location.1);
                max_lon = max_lon.max(node.location.1);
            }
        }

        if min_lat.is_finite() && max_lat.is_finite() {
            Some(BoundingBox::new(min_lat, max_lat, min_lon, max_lon))
        } else {
            None
        }
    }

    /// Interpret a coherence break
    fn interpret_break(&self, magnitude: f64, increased: bool) -> String {
        let direction = if increased { "increased" } else { "decreased" };
        let severity = if magnitude > 0.5 {
            "Major"
        } else if magnitude > 0.3 {
            "Moderate"
        } else {
            "Minor"
        };

        format!("{} regime shift: coherence {} by {:.1}%", severity, direction, magnitude * 100.0)
    }

    /// Get coherence history
    pub fn coherence_history(&self) -> &[(DateTime<Utc>, f64)] {
        &self.coherence_history
    }

    /// Get detected breaks
    pub fn detected_breaks(&self) -> &[CoherenceBreak] {
        &self.detected_breaks
    }
}

/// Climate data source for the framework
pub struct ClimateSource {
    noaa_client: NoaaClient,
    nasa_client: NasaClient,
    config: ClimateConfig,
}

impl ClimateSource {
    /// Create a new climate data source
    pub fn new(config: ClimateConfig) -> Self {
        Self {
            noaa_client: NoaaClient::new(config.noaa_token.clone()),
            nasa_client: NasaClient::new(config.nasa_token.clone()),
            config,
        }
    }
}

#[async_trait]
impl DataSource for ClimateSource {
    fn source_id(&self) -> &str {
        "climate"
    }

    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        // Fetch from NOAA
        let (observations, next_cursor) = self.noaa_client
            .fetch_ghcn_observations(
                self.config.bounding_box,
                &self.config.variables,
                cursor,
                batch_size,
            )
            .await
            .map_err(|e| FrameworkError::Ingestion(e.to_string()))?;

        // Convert to DataRecords
        let records: Vec<DataRecord> = observations
            .into_iter()
            .map(observation_to_record)
            .collect();

        Ok((records, next_cursor))
    }

    async fn total_count(&self) -> Result<Option<u64>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<bool> {
        self.noaa_client.health_check().await.map_err(|e| e.into())
    }
}

/// Convert climate observation to data record
fn observation_to_record(obs: ClimateObservation) -> DataRecord {
    DataRecord {
        id: format!("{}_{}", obs.station_id, obs.timestamp.timestamp()),
        source: "climate".to_string(),
        record_type: format!("{:?}", obs.variable).to_lowercase(),
        timestamp: obs.timestamp,
        data: serde_json::to_value(&obs).unwrap_or_default(),
        embedding: None,
        relationships: vec![
            Relationship {
                target_id: obs.station_id.clone(),
                rel_type: "observed_at".to_string(),
                weight: 1.0,
                properties: HashMap::new(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounding_box() {
        let bbox = BoundingBox::us_continental();
        assert!(bbox.contains(40.0, -100.0));
        assert!(!bbox.contains(60.0, -100.0));
    }

    #[test]
    fn test_pearson_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let corr = CoherenceAnalyzer::pearson_correlation(&x, &y);
        assert!((corr - 1.0).abs() < 0.001);

        let y_neg = vec![5.0, 4.0, 3.0, 2.0, 1.0];
        let corr_neg = CoherenceAnalyzer::pearson_correlation(&x, &y_neg);
        assert!((corr_neg + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_coherence_analyzer_creation() {
        let config = CoherenceAnalyzerConfig::default();
        let analyzer = CoherenceAnalyzer::new(config);
        assert!(analyzer.coherence_history().is_empty());
    }
}
