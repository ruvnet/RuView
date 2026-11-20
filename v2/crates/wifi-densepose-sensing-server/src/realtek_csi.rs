//! Bounded summaries for ADR-323 Realtek RTL8721Dx (AmebaDplus) 1x1 CSI frames.

use serde::Serialize;
use wifi_densepose_hardware::realtek_csi::{CsiFlags, CsiFrame};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct RealtekCsiSnapshot {
    pub event_type: &'static str,
    pub source: &'static str,
    pub csi_mode: &'static str,
    pub node_id: u8,
    pub sequence: u32,
    pub timestamp_us: u32,
    pub peer_mac: String,
    pub trig_mac: String,
    pub channel: u8,
    pub bandwidth_mhz: u16,
    pub rx_rate: u8,
    pub protocol_mode: String,
    pub num_sub_carrier: u16,
    pub num_bit_per_tone: u8,
    pub decimation: u8,
    pub rssi_dbm: i8,
    pub rxsc: u8,
    pub csi_valid: bool,
    pub synthetic: bool,
    pub mean_amplitude: Option<f32>,
    pub peak_amplitude: Option<f32>,
}

impl RealtekCsiSnapshot {
    pub(crate) fn from_frame(frame: &CsiFrame) -> Self {
        let synthetic = frame.flags.contains(CsiFlags::SYNTHETIC);
        let (mean_amplitude, peak_amplitude) = amplitude_summary(frame);
        Self {
            event_type: "realtek_csi",
            source: if synthetic {
                "realtek_csi:simulated"
            } else {
                "realtek_csi"
            },
            csi_mode: match frame.csi_mode {
                wifi_densepose_hardware::realtek_csi::CsiMode::Passive => "passive",
                wifi_densepose_hardware::realtek_csi::CsiMode::Active => "active",
                wifi_densepose_hardware::realtek_csi::CsiMode::BoardToBoard => "board_to_board",
            },
            node_id: frame.node_id,
            sequence: frame.sequence,
            timestamp_us: frame.timestamp_us,
            peer_mac: mac_string(&frame.peer_mac),
            trig_mac: mac_string(&frame.trig_mac),
            channel: frame.channel,
            bandwidth_mhz: if frame.bandwidth == 1 { 40 } else { 20 },
            rx_rate: frame.rx_rate,
            protocol_mode: format!("{:?}", frame.protocol_mode).to_ascii_lowercase(),
            num_sub_carrier: frame.num_sub_carrier,
            num_bit_per_tone: frame.num_bit_per_tone,
            decimation: frame.decimation,
            rssi_dbm: frame.rssi_dbm,
            rxsc: frame.rxsc,
            csi_valid: frame.csi_valid,
            synthetic,
            mean_amplitude,
            peak_amplitude,
        }
    }
}

fn mac_string(mac: &[u8; 6]) -> String {
    mac.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

fn amplitude_summary(frame: &CsiFrame) -> (Option<f32>, Option<f32>) {
    if frame.payload.is_empty() {
        return (None, None);
    }
    let amplitudes: Vec<f32> = frame
        .payload
        .iter()
        .map(|(i, q)| (*i as f32).hypot(*q as f32))
        .collect();
    let mean = amplitudes.iter().sum::<f32>() / amplitudes.len() as f32;
    let peak = amplitudes.into_iter().max_by(f32::total_cmp);
    (Some(mean), peak)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_densepose_hardware::realtek_csi::simulator::{RealtekCsiSimulator, SimulatorConfig};

    #[test]
    fn simulator_summary_preserves_dimensions_and_provenance() {
        let mut sim = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let snapshot = RealtekCsiSnapshot::from_frame(&sim.next_frame());
        assert_eq!(snapshot.source, "realtek_csi:simulated");
        assert_eq!(snapshot.num_sub_carrier, 52);
        assert_eq!(snapshot.csi_mode, "passive");
        assert!(snapshot.mean_amplitude.unwrap() > 0.0);
        assert!(snapshot.peak_amplitude.unwrap() >= snapshot.mean_amplitude.unwrap());
    }

    #[test]
    fn source_string_is_distinct_from_realtek_radar() {
        let mut sim = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let snapshot = RealtekCsiSnapshot::from_frame(&sim.next_frame());
        // Must never collapse to the bare "realtek"/"realtek:simulated" source
        // strings already owned by the RTL8720F radar path (realtek_radar.rs) —
        // effective_source()'s prefix matching in main.rs depends on this.
        assert_ne!(snapshot.source, "realtek");
        assert_ne!(snapshot.source, "realtek:simulated");
        assert!(snapshot.source.starts_with("realtek_csi"));
    }

    #[test]
    fn mac_addresses_are_formatted_colon_separated() {
        let mut sim = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let snapshot = RealtekCsiSnapshot::from_frame(&sim.next_frame());
        assert_eq!(snapshot.peer_mac, "a4:39:b3:a4:be:2d");
    }
}
