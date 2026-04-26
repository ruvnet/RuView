//! Sensor types for brain signal acquisition.

use serde::{Deserialize, Serialize};

/// Sensor technology type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SensorType {
    /// Nitrogen-vacancy diamond magnetometer.
    NvDiamond,
    /// Optically pumped magnetometer.
    Opm,
    /// Electroencephalography.
    Eeg,
    /// Superconducting quantum interference device MEG.
    SquidMeg,
    /// Atom interferometer for gravitational neural sensing.
    AtomInterferometer,
}

impl SensorType {
    /// Typical sensitivity in fT/sqrt(Hz) for this sensor technology.
    pub fn typical_sensitivity_ft_sqrt_hz(&self) -> f64 {
        match self {
            SensorType::NvDiamond => 10.0,
            SensorType::Opm => 7.0,
            SensorType::Eeg => 1000.0,
            SensorType::SquidMeg => 3.0,
            SensorType::AtomInterferometer => 1.0,
        }
    }
}

/// Sensor channel metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorChannel {
    /// Channel index.
    pub id: usize,
    /// Type of sensor.
    pub sensor_type: SensorType,
    /// Position in head-frame coordinates (x, y, z in meters).
    pub position: [f64; 3],
    /// Orientation unit normal vector.
    pub orientation: [f64; 3],
    /// Sensitivity in fT/sqrt(Hz).
    pub sensitivity_ft_sqrt_hz: f64,
    /// Sampling rate in Hz.
    pub sample_rate_hz: f64,
    /// Human-readable label (e.g., "Fz", "OPM-L01").
    pub label: String,
}

/// Sensor array configuration (a collection of channels of one type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorArray {
    /// All channels in the array.
    pub channels: Vec<SensorChannel>,
    /// Sensor technology used by this array.
    pub sensor_type: SensorType,
    /// Human-readable name for the array.
    pub name: String,
}

impl SensorArray {
    /// Number of channels in the array.
    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    /// Returns true if the array has no channels.
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// Get a channel by its index within this array.
    pub fn get_channel(&self, index: usize) -> Option<&SensorChannel> {
        self.channels.get(index)
    }

    /// Get the bounding box of channel positions as ([min_x, min_y, min_z], [max_x, max_y, max_z]).
    pub fn bounding_box(&self) -> Option<([f64; 3], [f64; 3])> {
        if self.channels.is_empty() {
            return None;
        }
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for ch in &self.channels {
            for i in 0..3 {
                if ch.position[i] < min[i] {
                    min[i] = ch.position[i];
                }
                if ch.position[i] > max[i] {
                    max[i] = ch.position[i];
                }
            }
        }
        Some((min, max))
    }
}
