//! CsiFrame → CsiData bridge (ADR-018 Layer 3).
//!
//! Converts hardware-level `CsiFrame` (I/Q pairs) into the pipeline-ready
//! `CsiData` format (amplitude/phase vectors). No ndarray dependency —
//! uses plain `Vec<f64>`.

use crate::csi_frame::CsiFrame;

/// Pipeline-ready CSI data with amplitude and phase vectors (ADR-018).
#[derive(Debug, Clone)]
pub struct CsiData {
    /// Unix timestamp in milliseconds when the frame was received.
    pub timestamp_unix_ms: u64,
    /// Node identifier (0-255).
    pub node_id: u8,
    /// Number of antennas.
    pub n_antennas: usize,
    /// Number of subcarriers per antenna.
    pub n_subcarriers: usize,
    /// Amplitude values: sqrt(I² + Q²) for each (antenna, subcarrier).
    /// Length = n_antennas * n_subcarriers, laid out antenna-major.
    pub amplitude: Vec<f64>,
    /// Phase values: atan2(Q, I) for each (antenna, subcarrier).
    /// Length = n_antennas * n_subcarriers.
    pub phase: Vec<f64>,
    /// RSSI in dBm.
    pub rssi_dbm: i8,
    /// Noise floor in dBm.
    pub noise_floor_dbm: i8,
    /// Channel center frequency in MHz.
    pub channel_freq_mhz: u32,
    /// Sequence number.
    pub sequence: u32,
}

impl CsiData {
    /// Compute SNR as RSSI - noise floor (in dB).
    pub fn snr_db(&self) -> f64 {
        self.rssi_dbm as f64 - self.noise_floor_dbm as f64
    }
}

impl From<CsiFrame> for CsiData {
    fn from(frame: CsiFrame) -> Self {
        let n_antennas = frame.metadata.n_antennas as usize;
        let n_subcarriers = frame.metadata.n_subcarriers as usize;
        let total = frame.subcarriers.len();

        let mut amplitude = Vec::with_capacity(total);
        let mut phase = Vec::with_capacity(total);

        for sc in &frame.subcarriers {
            let i = sc.i as f64;
            let q = sc.q as f64;
            amplitude.push((i * i + q * q).sqrt());
            phase.push(q.atan2(i));
        }

        let timestamp_unix_ms = frame.metadata.timestamp.timestamp_millis() as u64;

        CsiData {
            timestamp_unix_ms,
            node_id: frame.metadata.node_id,
            n_antennas,
            n_subcarriers,
            amplitude,
            phase,
            rssi_dbm: frame.metadata.rssi_dbm,
            noise_floor_dbm: frame.metadata.noise_floor_dbm,
            channel_freq_mhz: frame.metadata.channel_freq_mhz,
            sequence: frame.metadata.sequence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csi_frame::{AntennaConfig, Bandwidth, CsiMetadata, SubcarrierData};
    use chrono::Utc;

    fn make_frame(
        node_id: u8,
        n_antennas: u8,
        subcarriers: Vec<SubcarrierData>,
    ) -> CsiFrame {
        let n_subcarriers = if n_antennas == 0 {
            subcarriers.len()
        } else {
            subcarriers.len() / n_antennas as usize
        };

        CsiFrame {
            metadata: CsiMetadata {
                timestamp: Utc::now(),
                node_id,
                n_antennas,
                n_subcarriers: n_subcarriers as u16,
                channel_freq_mhz: 2437,
                rssi_dbm: -45,
                noise_floor_dbm: -90,
                bandwidth: Bandwidth::Bw20,
                antenna_config: AntennaConfig {
                    tx_antennas: 1,
                    rx_antennas: n_antennas,
                },
                sequence: 42,
            },
            subcarriers,
        }
    }

    #[test]
    fn test_bridge_from_known_iq() {
        let subs = vec![
            SubcarrierData { i: 3, q: 4, index: -1 },  // amp = 5.0
            SubcarrierData { i: 0, q: 10, index: 1 },   // amp = 10.0
        ];
        let frame = make_frame(1, 1, subs);
        let data: CsiData = frame.into();

        assert_eq!(data.amplitude.len(), 2);
        assert!((data.amplitude[0] - 5.0).abs() < 0.001);
        assert!((data.amplitude[1] - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_bridge_multi_antenna() {
        // 2 antennas, 3 subcarriers each = 6 total
        let subs = vec![
            SubcarrierData { i: 1, q: 0, index: -1 },
            SubcarrierData { i: 2, q: 0, index: 0 },
            SubcarrierData { i: 3, q: 0, index: 1 },
            SubcarrierData { i: 4, q: 0, index: -1 },
            SubcarrierData { i: 5, q: 0, index: 0 },
            SubcarrierData { i: 6, q: 0, index: 1 },
        ];
        let frame = make_frame(1, 2, subs);
        let data: CsiData = frame.into();

        assert_eq!(data.n_antennas, 2);
        assert_eq!(data.n_subcarriers, 3);
        assert_eq!(data.amplitude.len(), 6);
        assert_eq!(data.phase.len(), 6);
    }

    #[test]
    fn test_bridge_snr_computation() {
        let subs = vec![SubcarrierData { i: 1, q: 0, index: 0 }];
        let frame = make_frame(1, 1, subs);
        let data: CsiData = frame.into();

        // rssi=-45, noise=-90, SNR=45
        assert!((data.snr_db() - 45.0).abs() < 0.001);
    }

    #[test]
    fn test_bridge_preserves_metadata() {
        let subs = vec![SubcarrierData { i: 10, q: 20, index: 0 }];
        let frame = make_frame(7, 1, subs);
        let data: CsiData = frame.into();

        assert_eq!(data.node_id, 7);
        assert_eq!(data.channel_freq_mhz, 2437);
        assert_eq!(data.sequence, 42);
        assert_eq!(data.rssi_dbm, -45);
        assert_eq!(data.noise_floor_dbm, -90);
    }
}
