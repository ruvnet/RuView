//! Adapter for wifi-densepose-hardware crate.

use super::AdapterError;
use crate::domain::{SensorPosition, SensorType};

/// Hardware adapter for sensor communication
pub struct HardwareAdapter {
    /// Connected sensors
    sensors: Vec<SensorInfo>,
    /// Whether hardware is initialized
    initialized: bool,
}

/// Information about a connected sensor
#[derive(Debug, Clone)]
pub struct SensorInfo {
    /// Unique sensor ID
    pub id: String,
    /// Sensor position
    pub position: SensorPosition,
    /// Current status
    pub status: SensorStatus,
    /// Last RSSI reading (if available)
    pub last_rssi: Option<f64>,
    /// Battery level (0-100, if applicable)
    pub battery_level: Option<u8>,
}

/// Status of a sensor
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SensorStatus {
    /// Sensor is connected and operational
    Connected,
    /// Sensor is disconnected
    Disconnected,
    /// Sensor is in error state
    Error,
    /// Sensor is initializing
    Initializing,
    /// Sensor battery is low
    LowBattery,
}

impl HardwareAdapter {
    /// Create a new hardware adapter
    pub fn new() -> Self {
        Self {
            sensors: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize hardware communication
    pub fn initialize(&mut self) -> Result<(), AdapterError> {
        // In production, this would initialize actual hardware
        // using wifi-densepose-hardware crate
        self.initialized = true;
        Ok(())
    }

    /// Discover available sensors
    pub fn discover_sensors(&mut self) -> Result<Vec<SensorInfo>, AdapterError> {
        if !self.initialized {
            return Err(AdapterError::Hardware("Hardware not initialized".into()));
        }

        // In production, this would scan for WiFi devices
        // For now, return empty list (would be populated by real hardware)
        Ok(Vec::new())
    }

    /// Add a sensor
    pub fn add_sensor(&mut self, sensor: SensorInfo) -> Result<(), AdapterError> {
        if self.sensors.iter().any(|s| s.id == sensor.id) {
            return Err(AdapterError::Hardware(format!(
                "Sensor {} already registered",
                sensor.id
            )));
        }

        self.sensors.push(sensor);
        Ok(())
    }

    /// Remove a sensor
    pub fn remove_sensor(&mut self, sensor_id: &str) -> Result<(), AdapterError> {
        let initial_len = self.sensors.len();
        self.sensors.retain(|s| s.id != sensor_id);

        if self.sensors.len() == initial_len {
            return Err(AdapterError::Hardware(format!(
                "Sensor {} not found",
                sensor_id
            )));
        }

        Ok(())
    }

    /// Get all sensors
    pub fn sensors(&self) -> &[SensorInfo] {
        &self.sensors
    }

    /// Get operational sensors
    pub fn operational_sensors(&self) -> Vec<&SensorInfo> {
        self.sensors
            .iter()
            .filter(|s| s.status == SensorStatus::Connected)
            .collect()
    }

    /// Get sensor positions for localization
    pub fn sensor_positions(&self) -> Vec<SensorPosition> {
        self.sensors
            .iter()
            .filter(|s| s.status == SensorStatus::Connected)
            .map(|s| s.position.clone())
            .collect()
    }

    /// Read CSI data from sensors
    pub fn read_csi(&self) -> Result<CsiReadings, AdapterError> {
        if !self.initialized {
            return Err(AdapterError::Hardware("Hardware not initialized".into()));
        }

        // In production, this would read actual CSI data
        // For now, return empty readings
        Ok(CsiReadings {
            timestamp: chrono::Utc::now(),
            readings: Vec::new(),
        })
    }

    /// Read RSSI from all sensors
    pub fn read_rssi(&self) -> Result<Vec<(String, f64)>, AdapterError> {
        if !self.initialized {
            return Err(AdapterError::Hardware("Hardware not initialized".into()));
        }

        // Return last known RSSI values
        Ok(self
            .sensors
            .iter()
            .filter_map(|s| s.last_rssi.map(|rssi| (s.id.clone(), rssi)))
            .collect())
    }

    /// Update sensor position
    pub fn update_sensor_position(
        &mut self,
        sensor_id: &str,
        position: SensorPosition,
    ) -> Result<(), AdapterError> {
        let sensor = self
            .sensors
            .iter_mut()
            .find(|s| s.id == sensor_id)
            .ok_or_else(|| AdapterError::Hardware(format!("Sensor {} not found", sensor_id)))?;

        sensor.position = position;
        Ok(())
    }

    /// Check hardware health
    pub fn health_check(&self) -> HardwareHealth {
        let total = self.sensors.len();
        let connected = self
            .sensors
            .iter()
            .filter(|s| s.status == SensorStatus::Connected)
            .count();
        let low_battery = self
            .sensors
            .iter()
            .filter(|s| matches!(s.battery_level, Some(b) if b < 20))
            .count();

        let status = if connected == 0 && total > 0 {
            HealthStatus::Critical
        } else if connected < total / 2 {
            HealthStatus::Degraded
        } else if low_battery > 0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        HardwareHealth {
            status,
            total_sensors: total,
            connected_sensors: connected,
            low_battery_sensors: low_battery,
        }
    }
}

impl Default for HardwareAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// CSI readings from sensors
#[derive(Debug, Clone)]
pub struct CsiReadings {
    /// Timestamp of readings
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Individual sensor readings
    pub readings: Vec<SensorCsiReading>,
}

/// CSI reading from a single sensor
#[derive(Debug, Clone)]
pub struct SensorCsiReading {
    /// Sensor ID
    pub sensor_id: String,
    /// CSI amplitudes (per subcarrier)
    pub amplitudes: Vec<f64>,
    /// CSI phases (per subcarrier)
    pub phases: Vec<f64>,
    /// RSSI value
    pub rssi: f64,
    /// Noise floor
    pub noise_floor: f64,
}

/// Hardware health status
#[derive(Debug, Clone)]
pub struct HardwareHealth {
    /// Overall status
    pub status: HealthStatus,
    /// Total number of sensors
    pub total_sensors: usize,
    /// Number of connected sensors
    pub connected_sensors: usize,
    /// Number of sensors with low battery
    pub low_battery_sensors: usize,
}

/// Health status levels
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// All systems operational
    Healthy,
    /// Minor issues, still functional
    Warning,
    /// Significant issues, reduced capability
    Degraded,
    /// System not functional
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_sensor(id: &str) -> SensorInfo {
        SensorInfo {
            id: id.to_string(),
            position: SensorPosition {
                id: id.to_string(),
                x: 0.0,
                y: 0.0,
                z: 1.5,
                sensor_type: SensorType::Transceiver,
                is_operational: true,
            },
            status: SensorStatus::Connected,
            last_rssi: Some(-45.0),
            battery_level: Some(80),
        }
    }

    #[test]
    fn test_add_sensor() {
        let mut adapter = HardwareAdapter::new();
        adapter.initialize().unwrap();

        let sensor = create_test_sensor("s1");
        assert!(adapter.add_sensor(sensor).is_ok());
        assert_eq!(adapter.sensors().len(), 1);
    }

    #[test]
    fn test_duplicate_sensor_error() {
        let mut adapter = HardwareAdapter::new();
        adapter.initialize().unwrap();

        let sensor1 = create_test_sensor("s1");
        let sensor2 = create_test_sensor("s1");

        adapter.add_sensor(sensor1).unwrap();
        assert!(adapter.add_sensor(sensor2).is_err());
    }

    #[test]
    fn test_health_check() {
        let mut adapter = HardwareAdapter::new();
        adapter.initialize().unwrap();

        // No sensors - should be healthy (nothing to fail)
        let health = adapter.health_check();
        assert!(matches!(health.status, HealthStatus::Healthy));

        // Add connected sensor
        adapter.add_sensor(create_test_sensor("s1")).unwrap();
        let health = adapter.health_check();
        assert!(matches!(health.status, HealthStatus::Healthy));
    }

    #[test]
    fn test_sensor_positions() {
        let mut adapter = HardwareAdapter::new();
        adapter.initialize().unwrap();

        adapter.add_sensor(create_test_sensor("s1")).unwrap();
        adapter.add_sensor(create_test_sensor("s2")).unwrap();

        let positions = adapter.sensor_positions();
        assert_eq!(positions.len(), 2);
    }
}
