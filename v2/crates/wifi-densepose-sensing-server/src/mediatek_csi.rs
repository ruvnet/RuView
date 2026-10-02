//! Bounded summaries for ADR-267 MediaTek MIMO CSI frames.

use serde::Serialize;
use wifi_densepose_hardware::mediatek_csi::{CsiFlags, CsiFrame, CsiPayload, ReportKind};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct MediatekCsiSnapshot {
    pub event_type: &'static str,
    pub source: &'static str,
    pub report_kind: &'static str,
    pub sequence: u32,
    pub timestamp_us: u64,
    pub device_id: String,
    pub chipset: &'static str,
    pub center_freq_khz: u32,
    pub bandwidth_mhz: u16,
    pub tx_count: u8,
    pub rx_count: u8,
    pub subcarrier_count: u16,
    pub element_count: usize,
    pub ppdu_type: String,
    pub rssi_dbm: Vec<i8>,
    pub noise_floor_dbm: i8,
    pub calibrated: bool,
    pub synthetic: bool,
    pub saturated: bool,
    pub time_synchronized: bool,
    pub dropped_predecessor: bool,
    pub calibration_id: u32,
    pub subcarrier_spacing_hz: f32,
    pub mean_amplitude: Option<f32>,
    pub peak_amplitude: Option<f32>,
}

impl MediatekCsiSnapshot {
    pub(crate) fn from_frame(frame: &CsiFrame) -> Self {
        let synthetic = frame.flags.contains(CsiFlags::SYNTHETIC);
        let calibrated = frame.flags.contains(CsiFlags::CALIBRATED);
        let (mean_amplitude, peak_amplitude) = amplitude_summary(frame);
        Self {
            event_type: "mediatek_csi",
            // Provenance is decided by the frame's own flags, in this order, and
            // never by which ingest path delivered it (ADR-267). SYNTHETIC is
            // checked first and is not clearable by anything downstream: a
            // simulated frame stays labelled simulated even when it also claims
            // CALIBRATED, which the ADR-266 simulator does.
            source: if synthetic {
                "mediatek:simulated"
            } else if calibrated {
                "mediatek"
            } else {
                // Real silicon, but nothing has validated this capture against a
                // calibration. `wifi-densepose-mtk-bridge` emits exactly this class.
                "mediatek:physical-unvalidated"
            },
            report_kind: match frame.report_kind {
                ReportKind::Csi => "csi",
                ReportKind::Capabilities => "capabilities",
            },
            sequence: frame.sequence,
            timestamp_us: frame.timestamp_us,
            device_id: format!("{:016x}", frame.device_id),
            chipset: frame.chipset.name(),
            center_freq_khz: frame.center_freq_khz,
            bandwidth_mhz: frame.bandwidth_mhz,
            tx_count: frame.tx_count,
            rx_count: frame.rx_count,
            subcarrier_count: frame.subcarrier_count,
            element_count: frame.payload.len(),
            ppdu_type: format!("{:?}", frame.ppdu_type).to_ascii_lowercase(),
            rssi_dbm: frame.payload.rssi_dbm().to_vec(),
            noise_floor_dbm: frame.noise_floor_dbm,
            calibrated,
            synthetic,
            saturated: frame.flags.contains(CsiFlags::SATURATED),
            time_synchronized: frame.flags.contains(CsiFlags::TIME_SYNCHRONIZED),
            dropped_predecessor: frame.flags.contains(CsiFlags::DROPPED_PREDECESSOR),
            calibration_id: frame.calibration_id,
            subcarrier_spacing_hz: frame.subcarrier_spacing_hz,
            mean_amplitude,
            peak_amplitude,
        }
    }
}

fn amplitude_summary(frame: &CsiFrame) -> (Option<f32>, Option<f32>) {
    let amplitudes: Vec<f32> = match &frame.payload {
        CsiPayload::ComplexI16 { values, .. } => values
            .iter()
            .map(|[i, q]| (*i as f32).hypot(*q as f32) * frame.scale)
            .collect(),
        CsiPayload::ComplexF32 { values, .. } => values
            .iter()
            .map(|[i, q]| i.hypot(*q) * frame.scale)
            .collect(),
        CsiPayload::Bytes(_) => return (None, None),
    };
    if amplitudes.is_empty() {
        return (None, None);
    }
    let mean = amplitudes.iter().sum::<f32>() / amplitudes.len() as f32;
    let peak = amplitudes.into_iter().max_by(f32::total_cmp);
    (Some(mean), peak)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_densepose_hardware::mediatek_csi::simulator::{MediatekCsiSimulator, SimulatorConfig};

    #[test]
    fn simulator_summary_preserves_dimensions_and_provenance() {
        let mut sim = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let snapshot = MediatekCsiSnapshot::from_frame(&sim.next_frame());
        assert_eq!(snapshot.source, "mediatek:simulated");
        assert_eq!(
            (
                snapshot.tx_count,
                snapshot.rx_count,
                snapshot.subcarrier_count
            ),
            (2, 3, 256)
        );
        assert_eq!(snapshot.element_count, 1536);
        assert!(snapshot.mean_amplitude.unwrap() > 0.0);
        assert!(snapshot.peak_amplitude.unwrap() >= snapshot.mean_amplitude.unwrap());
    }

    /// A frame off real silicon that has not been calibrated must be
    /// distinguishable from both a simulation and a validated capture.
    /// `wifi-densepose-mtk-bridge` produces exactly this shape.
    #[test]
    fn physical_uncalibrated_frame_is_labelled_physical_unvalidated() {
        let mut sim = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let mut frame = sim.next_frame();
        frame.flags = CsiFlags(0);
        let snapshot = MediatekCsiSnapshot::from_frame(&frame);
        assert_eq!(snapshot.source, "mediatek:physical-unvalidated");
        assert!(!snapshot.synthetic);
        assert!(!snapshot.calibrated);
    }

    /// Plain `"mediatek"` is reserved for calibrated physical frames.
    #[test]
    fn calibrated_physical_frame_keeps_the_plain_mediatek_label() {
        let mut sim = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let mut frame = sim.next_frame();
        frame.flags = CsiFlags(CsiFlags::CALIBRATED);
        let snapshot = MediatekCsiSnapshot::from_frame(&frame);
        assert_eq!(snapshot.source, "mediatek");
        assert!(snapshot.calibrated);
        assert!(!snapshot.synthetic);
    }

    /// ADR-267: SYNTHETIC is not clearable by an ingest path. No combination of
    /// other flags may promote a simulated frame out of `mediatek:simulated`.
    #[test]
    fn synthetic_frame_can_never_be_relabelled_as_physical() {
        let mut sim = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let base = sim.next_frame();
        for extra in [
            0,
            CsiFlags::CALIBRATED,
            CsiFlags::TIME_SYNCHRONIZED,
            CsiFlags::SATURATED | CsiFlags::DROPPED_PREDECESSOR,
            CsiFlags::CALIBRATED | CsiFlags::TIME_SYNCHRONIZED | CsiFlags::SATURATED,
        ] {
            let mut frame = base.clone();
            frame.flags = CsiFlags(CsiFlags::SYNTHETIC | extra);
            let snapshot = MediatekCsiSnapshot::from_frame(&frame);
            assert_eq!(
                snapshot.source, "mediatek:simulated",
                "flags {:#06x} must stay simulated",
                frame.flags.0
            );
            assert!(snapshot.synthetic);
        }
    }

    /// The three labels are distinct, and all three still start with
    /// `"mediatek"`, which is what the server's staleness check keys on.
    #[test]
    fn the_three_provenance_labels_are_distinct_and_all_prefixed_mediatek() {
        let mut sim = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let base = sim.next_frame();
        let label = |flags: u16| {
            let mut f = base.clone();
            f.flags = CsiFlags(flags);
            MediatekCsiSnapshot::from_frame(&f).source
        };
        let simulated = label(CsiFlags::SYNTHETIC | CsiFlags::CALIBRATED);
        let calibrated = label(CsiFlags::CALIBRATED);
        let physical = label(0);
        assert_eq!(simulated, "mediatek:simulated");
        assert_eq!(calibrated, "mediatek");
        assert_eq!(physical, "mediatek:physical-unvalidated");
        for l in [simulated, calibrated, physical] {
            assert!(l.starts_with("mediatek"), "{l}");
        }
    }

    #[test]
    fn capability_summary_does_not_invent_signal_statistics() {
        let sim = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let snapshot = MediatekCsiSnapshot::from_frame(&sim.capabilities_frame());
        assert_eq!(snapshot.report_kind, "capabilities");
        assert_eq!(snapshot.mean_amplitude, None);
        assert!(snapshot.rssi_dbm.is_empty());
    }
}
