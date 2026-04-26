//! Scan zone entity for defining areas to monitor.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Unique identifier for a scan zone
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScanZoneId(Uuid);

impl ScanZoneId {
    /// Create a new random zone ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for ScanZoneId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ScanZoneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Bounds of a scan zone
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ZoneBounds {
    /// Rectangular zone
    Rectangle {
        /// Minimum X coordinate
        min_x: f64,
        /// Minimum Y coordinate
        min_y: f64,
        /// Maximum X coordinate
        max_x: f64,
        /// Maximum Y coordinate
        max_y: f64,
    },
    /// Circular zone
    Circle {
        /// Center X coordinate
        center_x: f64,
        /// Center Y coordinate
        center_y: f64,
        /// Radius in meters
        radius: f64,
    },
    /// Polygon zone (ordered vertices)
    Polygon {
        /// List of (x, y) vertices
        vertices: Vec<(f64, f64)>,
    },
}

impl ZoneBounds {
    /// Create a rectangular zone
    pub fn rectangle(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        ZoneBounds::Rectangle { min_x, min_y, max_x, max_y }
    }

    /// Create a circular zone
    pub fn circle(center_x: f64, center_y: f64, radius: f64) -> Self {
        ZoneBounds::Circle { center_x, center_y, radius }
    }

    /// Create a polygon zone
    pub fn polygon(vertices: Vec<(f64, f64)>) -> Self {
        ZoneBounds::Polygon { vertices }
    }

    /// Calculate the area of the zone in square meters
    pub fn area(&self) -> f64 {
        match self {
            ZoneBounds::Rectangle { min_x, min_y, max_x, max_y } => {
                (max_x - min_x) * (max_y - min_y)
            }
            ZoneBounds::Circle { radius, .. } => {
                std::f64::consts::PI * radius * radius
            }
            ZoneBounds::Polygon { vertices } => {
                // Shoelace formula
                if vertices.len() < 3 {
                    return 0.0;
                }
                let mut area = 0.0;
                let n = vertices.len();
                for i in 0..n {
                    let j = (i + 1) % n;
                    area += vertices[i].0 * vertices[j].1;
                    area -= vertices[j].0 * vertices[i].1;
                }
                (area / 2.0).abs()
            }
        }
    }

    /// Check if a point is within the zone bounds
    pub fn contains(&self, x: f64, y: f64) -> bool {
        match self {
            ZoneBounds::Rectangle { min_x, min_y, max_x, max_y } => {
                x >= *min_x && x <= *max_x && y >= *min_y && y <= *max_y
            }
            ZoneBounds::Circle { center_x, center_y, radius } => {
                let dx = x - center_x;
                let dy = y - center_y;
                (dx * dx + dy * dy).sqrt() <= *radius
            }
            ZoneBounds::Polygon { vertices } => {
                // Ray casting algorithm
                if vertices.len() < 3 {
                    return false;
                }
                let mut inside = false;
                let n = vertices.len();
                let mut j = n - 1;
                for i in 0..n {
                    let (xi, yi) = vertices[i];
                    let (xj, yj) = vertices[j];
                    if ((yi > y) != (yj > y))
                        && (x < (xj - xi) * (y - yi) / (yj - yi) + xi)
                    {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            }
        }
    }

    /// Get the center point of the zone
    pub fn center(&self) -> (f64, f64) {
        match self {
            ZoneBounds::Rectangle { min_x, min_y, max_x, max_y } => {
                ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0)
            }
            ZoneBounds::Circle { center_x, center_y, .. } => {
                (*center_x, *center_y)
            }
            ZoneBounds::Polygon { vertices } => {
                if vertices.is_empty() {
                    return (0.0, 0.0);
                }
                let sum_x: f64 = vertices.iter().map(|(x, _)| x).sum();
                let sum_y: f64 = vertices.iter().map(|(_, y)| y).sum();
                let n = vertices.len() as f64;
                (sum_x / n, sum_y / n)
            }
        }
    }
}

/// Status of a scan zone
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ZoneStatus {
    /// Zone is active and being scanned
    Active,
    /// Zone is paused (temporary)
    Paused,
    /// Zone scan is complete
    Complete,
    /// Zone is inaccessible
    Inaccessible,
    /// Zone is deactivated
    Deactivated,
}

/// Parameters for scanning a zone
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScanParameters {
    /// Scan sensitivity (0.0-1.0)
    pub sensitivity: f64,
    /// Maximum depth to scan (meters)
    pub max_depth: f64,
    /// Scan resolution (higher = more detailed but slower)
    pub resolution: ScanResolution,
    /// Whether to use enhanced breathing detection
    pub enhanced_breathing: bool,
    /// Whether to use heartbeat detection (more sensitive but slower)
    pub heartbeat_detection: bool,
}

impl Default for ScanParameters {
    fn default() -> Self {
        Self {
            sensitivity: 0.8,
            max_depth: 5.0,
            resolution: ScanResolution::Standard,
            enhanced_breathing: true,
            heartbeat_detection: false,
        }
    }
}

/// Scan resolution levels
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ScanResolution {
    /// Quick scan, lower accuracy
    Quick,
    /// Standard scan
    Standard,
    /// High resolution scan
    High,
    /// Maximum resolution (slowest)
    Maximum,
}

impl ScanResolution {
    /// Get scan time multiplier
    pub fn time_multiplier(&self) -> f64 {
        match self {
            ScanResolution::Quick => 0.5,
            ScanResolution::Standard => 1.0,
            ScanResolution::High => 2.0,
            ScanResolution::Maximum => 4.0,
        }
    }
}

/// Position of a sensor (WiFi transmitter/receiver)
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SensorPosition {
    /// Sensor identifier
    pub id: String,
    /// X coordinate (meters)
    pub x: f64,
    /// Y coordinate (meters)
    pub y: f64,
    /// Z coordinate (meters, height above ground)
    pub z: f64,
    /// Sensor type
    pub sensor_type: SensorType,
    /// Whether sensor is operational
    pub is_operational: bool,
}

/// Types of sensors
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SensorType {
    /// WiFi transmitter
    Transmitter,
    /// WiFi receiver
    Receiver,
    /// Combined transmitter/receiver
    Transceiver,
}

/// A defined geographic area being monitored for survivors
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScanZone {
    id: ScanZoneId,
    name: String,
    bounds: ZoneBounds,
    sensor_positions: Vec<SensorPosition>,
    parameters: ScanParameters,
    status: ZoneStatus,
    created_at: DateTime<Utc>,
    last_scan: Option<DateTime<Utc>>,
    scan_count: u32,
    detections_count: u32,
}

impl ScanZone {
    /// Create a new scan zone
    pub fn new(name: &str, bounds: ZoneBounds) -> Self {
        Self {
            id: ScanZoneId::new(),
            name: name.to_string(),
            bounds,
            sensor_positions: Vec::new(),
            parameters: ScanParameters::default(),
            status: ZoneStatus::Active,
            created_at: Utc::now(),
            last_scan: None,
            scan_count: 0,
            detections_count: 0,
        }
    }

    /// Create with custom parameters
    pub fn with_parameters(name: &str, bounds: ZoneBounds, parameters: ScanParameters) -> Self {
        let mut zone = Self::new(name, bounds);
        zone.parameters = parameters;
        zone
    }

    /// Get the zone ID
    pub fn id(&self) -> &ScanZoneId {
        &self.id
    }

    /// Get the zone name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the bounds
    pub fn bounds(&self) -> &ZoneBounds {
        &self.bounds
    }

    /// Get sensor positions
    pub fn sensor_positions(&self) -> &[SensorPosition] {
        &self.sensor_positions
    }

    /// Get scan parameters
    pub fn parameters(&self) -> &ScanParameters {
        &self.parameters
    }

    /// Get mutable scan parameters
    pub fn parameters_mut(&mut self) -> &mut ScanParameters {
        &mut self.parameters
    }

    /// Get the status
    pub fn status(&self) -> &ZoneStatus {
        &self.status
    }

    /// Get last scan time
    pub fn last_scan(&self) -> Option<&DateTime<Utc>> {
        self.last_scan.as_ref()
    }

    /// Get scan count
    pub fn scan_count(&self) -> u32 {
        self.scan_count
    }

    /// Get detection count
    pub fn detections_count(&self) -> u32 {
        self.detections_count
    }

    /// Add a sensor to the zone
    pub fn add_sensor(&mut self, sensor: SensorPosition) {
        self.sensor_positions.push(sensor);
    }

    /// Remove a sensor
    pub fn remove_sensor(&mut self, sensor_id: &str) {
        self.sensor_positions.retain(|s| s.id != sensor_id);
    }

    /// Set zone status
    pub fn set_status(&mut self, status: ZoneStatus) {
        self.status = status;
    }

    /// Pause the zone
    pub fn pause(&mut self) {
        self.status = ZoneStatus::Paused;
    }

    /// Resume the zone
    pub fn resume(&mut self) {
        if self.status == ZoneStatus::Paused {
            self.status = ZoneStatus::Active;
        }
    }

    /// Mark zone as complete
    pub fn complete(&mut self) {
        self.status = ZoneStatus::Complete;
    }

    /// Record a scan
    pub fn record_scan(&mut self, found_detections: u32) {
        self.last_scan = Some(Utc::now());
        self.scan_count += 1;
        self.detections_count += found_detections;
    }

    /// Check if a point is within this zone
    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        self.bounds.contains(x, y)
    }

    /// Get the area of the zone
    pub fn area(&self) -> f64 {
        self.bounds.area()
    }

    /// Check if zone has enough sensors for localization
    pub fn has_sufficient_sensors(&self) -> bool {
        // Need at least 3 sensors for 2D localization
        self.sensor_positions.iter()
            .filter(|s| s.is_operational)
            .count() >= 3
    }

    /// Time since last scan
    pub fn time_since_scan(&self) -> Option<chrono::Duration> {
        self.last_scan.map(|t| Utc::now() - t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rectangle_bounds() {
        let bounds = ZoneBounds::rectangle(0.0, 0.0, 10.0, 10.0);

        assert!((bounds.area() - 100.0).abs() < 0.001);
        assert!(bounds.contains(5.0, 5.0));
        assert!(!bounds.contains(15.0, 5.0));
        assert_eq!(bounds.center(), (5.0, 5.0));
    }

    #[test]
    fn test_circle_bounds() {
        let bounds = ZoneBounds::circle(0.0, 0.0, 10.0);

        assert!((bounds.area() - std::f64::consts::PI * 100.0).abs() < 0.001);
        assert!(bounds.contains(0.0, 0.0));
        assert!(bounds.contains(5.0, 5.0));
        assert!(!bounds.contains(10.0, 10.0));
    }

    #[test]
    fn test_scan_zone_creation() {
        let zone = ScanZone::new(
            "Test Zone",
            ZoneBounds::rectangle(0.0, 0.0, 50.0, 30.0),
        );

        assert_eq!(zone.name(), "Test Zone");
        assert!(matches!(zone.status(), ZoneStatus::Active));
        assert_eq!(zone.scan_count(), 0);
    }

    #[test]
    fn test_scan_zone_sensors() {
        let mut zone = ScanZone::new(
            "Test Zone",
            ZoneBounds::rectangle(0.0, 0.0, 50.0, 30.0),
        );

        assert!(!zone.has_sufficient_sensors());

        for i in 0..3 {
            zone.add_sensor(SensorPosition {
                id: format!("sensor-{}", i),
                x: i as f64 * 10.0,
                y: 0.0,
                z: 1.5,
                sensor_type: SensorType::Transceiver,
                is_operational: true,
            });
        }

        assert!(zone.has_sufficient_sensors());
    }

    #[test]
    fn test_scan_zone_status_transitions() {
        let mut zone = ScanZone::new(
            "Test",
            ZoneBounds::rectangle(0.0, 0.0, 10.0, 10.0),
        );

        assert!(matches!(zone.status(), ZoneStatus::Active));

        zone.pause();
        assert!(matches!(zone.status(), ZoneStatus::Paused));

        zone.resume();
        assert!(matches!(zone.status(), ZoneStatus::Active));

        zone.complete();
        assert!(matches!(zone.status(), ZoneStatus::Complete));
    }
}
