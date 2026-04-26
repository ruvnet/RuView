//! CSI frame types representing parsed WiFi Channel State Information.
//!
//! These types are hardware-agnostic representations of CSI data that
//! can be produced by any parser (ESP32, Intel 5300, etc.) and consumed
//! by the detection pipeline.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A parsed CSI frame containing subcarrier data and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiFrame {
    /// Frame metadata (RSSI, channel, timestamps, etc.)
    pub metadata: CsiMetadata,
    /// Per-subcarrier I/Q data
    pub subcarriers: Vec<SubcarrierData>,
}

impl CsiFrame {
    /// Number of subcarriers in this frame.
    pub fn subcarrier_count(&self) -> usize {
        self.subcarriers.len()
    }

    /// Convert to amplitude and phase arrays for the detection pipeline.
    ///
    /// Returns (amplitudes, phases) where:
    /// - amplitude = sqrt(I^2 + Q^2)
    /// - phase = atan2(Q, I)
    pub fn to_amplitude_phase(&self) -> (Vec<f64>, Vec<f64>) {
        let amplitudes: Vec<f64> = self.subcarriers.iter()
            .map(|sc| (sc.i as f64 * sc.i as f64 + sc.q as f64 * sc.q as f64).sqrt())
            .collect();

        let phases: Vec<f64> = self.subcarriers.iter()
            .map(|sc| (sc.q as f64).atan2(sc.i as f64))
            .collect();

        (amplitudes, phases)
    }

    /// Get the average amplitude across all subcarriers.
    pub fn mean_amplitude(&self) -> f64 {
        if self.subcarriers.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.subcarriers.iter()
            .map(|sc| (sc.i as f64 * sc.i as f64 + sc.q as f64 * sc.q as f64).sqrt())
            .sum();
        sum / self.subcarriers.len() as f64
    }

    /// Check if this frame has valid data (non-zero subcarriers with non-zero I/Q).
    pub fn is_valid(&self) -> bool {
        !self.subcarriers.is_empty()
            && self.subcarriers.iter().any(|sc| sc.i != 0 || sc.q != 0)
    }
}

/// Metadata associated with a CSI frame (ADR-018 format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiMetadata {
    /// Timestamp when frame was received
    pub timestamp: DateTime<Utc>,
    /// Node identifier (0-255)
    pub node_id: u8,
    /// Number of antennas
    pub n_antennas: u8,
    /// Number of subcarriers
    pub n_subcarriers: u16,
    /// Channel center frequency in MHz
    pub channel_freq_mhz: u32,
    /// RSSI in dBm (signed byte, typically -100 to 0)
    pub rssi_dbm: i8,
    /// Noise floor in dBm (signed byte)
    pub noise_floor_dbm: i8,
    /// Channel bandwidth (derived from n_subcarriers)
    pub bandwidth: Bandwidth,
    /// Antenna configuration (populated from n_antennas)
    pub antenna_config: AntennaConfig,
    /// Sequence number for ordering
    pub sequence: u32,
}

/// WiFi channel bandwidth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bandwidth {
    /// 20 MHz (standard)
    Bw20,
    /// 40 MHz (HT)
    Bw40,
    /// 80 MHz (VHT)
    Bw80,
    /// 160 MHz (VHT)
    Bw160,
}

impl Bandwidth {
    /// Expected number of subcarriers for this bandwidth.
    pub fn expected_subcarriers(&self) -> usize {
        match self {
            Bandwidth::Bw20 => 56,
            Bandwidth::Bw40 => 114,
            Bandwidth::Bw80 => 242,
            Bandwidth::Bw160 => 484,
        }
    }
}

/// Antenna configuration for MIMO.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AntennaConfig {
    /// Number of transmit antennas
    pub tx_antennas: u8,
    /// Number of receive antennas
    pub rx_antennas: u8,
}

impl Default for AntennaConfig {
    fn default() -> Self {
        Self {
            tx_antennas: 1,
            rx_antennas: 1,
        }
    }
}

/// A single subcarrier's I/Q data.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SubcarrierData {
    /// In-phase component
    pub i: i16,
    /// Quadrature component
    pub q: i16,
    /// Subcarrier index (-28..28 for 20MHz, etc.)
    pub index: i16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn make_test_frame() -> CsiFrame {
        CsiFrame {
            metadata: CsiMetadata {
                timestamp: Utc::now(),
                node_id: 1,
                n_antennas: 1,
                n_subcarriers: 3,
                channel_freq_mhz: 2437,
                rssi_dbm: -50,
                noise_floor_dbm: -95,
                bandwidth: Bandwidth::Bw20,
                antenna_config: AntennaConfig::default(),
                sequence: 1,
            },
            subcarriers: vec![
                SubcarrierData { i: 100, q: 0, index: -28 },
                SubcarrierData { i: 0, q: 50, index: -27 },
                SubcarrierData { i: 30, q: 40, index: -26 },
            ],
        }
    }

    #[test]
    fn test_amplitude_phase_conversion() {
        let frame = make_test_frame();
        let (amps, phases) = frame.to_amplitude_phase();

        assert_eq!(amps.len(), 3);
        assert_eq!(phases.len(), 3);

        // First subcarrier: I=100, Q=0 -> amplitude=100, phase=0
        assert_relative_eq!(amps[0], 100.0, epsilon = 0.01);
        assert_relative_eq!(phases[0], 0.0, epsilon = 0.01);

        // Second: I=0, Q=50 -> amplitude=50, phase=pi/2
        assert_relative_eq!(amps[1], 50.0, epsilon = 0.01);
        assert_relative_eq!(phases[1], std::f64::consts::FRAC_PI_2, epsilon = 0.01);

        // Third: I=30, Q=40 -> amplitude=50, phase=atan2(40,30)
        assert_relative_eq!(amps[2], 50.0, epsilon = 0.01);
    }

    #[test]
    fn test_mean_amplitude() {
        let frame = make_test_frame();
        let mean = frame.mean_amplitude();
        // (100 + 50 + 50) / 3 = 66.67
        assert_relative_eq!(mean, 200.0 / 3.0, epsilon = 0.1);
    }

    #[test]
    fn test_is_valid() {
        let frame = make_test_frame();
        assert!(frame.is_valid());

        let empty = CsiFrame {
            metadata: frame.metadata.clone(),
            subcarriers: vec![],
        };
        assert!(!empty.is_valid());
    }

    #[test]
    fn test_bandwidth_subcarriers() {
        assert_eq!(Bandwidth::Bw20.expected_subcarriers(), 56);
        assert_eq!(Bandwidth::Bw40.expected_subcarriers(), 114);
    }
}
