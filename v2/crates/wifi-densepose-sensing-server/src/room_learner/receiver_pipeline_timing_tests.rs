//! SYNTHETIC relative receiver timing; no captured RF or physical clock claim.
use super::*;
use crate::room_learner::{native::{canonicalize_esp32, NativePacketInput, CanonicalPacket}, observation::CaptureProvenance};
use wifi_densepose_hardware::{Adr018Flags, PpduType};

fn packet(source: u8, sequence: u32, stamp: u64, ppdu: PpduType, bins: usize) -> CanonicalPacket {
    canonicalize_esp32(NativePacketInput { node_id: source, n_antennas: 1, raw_bins: bins,
        frequency_hz: 2_437_000_000, ppdu_type: ppdu, sequence, received_at_ns: stamp,
        receiver_sample_rate_hz: 33.0, iq_bytes: &vec![4u8; bins * 2],
        flags: Adr018Flags::from_byte(0), provenance: CaptureProvenance::Synthetic }).unwrap()
}
const PRIMARY: &str = "temporal_observed_gap_or_rate: source=15 direction=below_min receiver_interval_ns=1000000 min_ns=27777777 max_ns=50000000";

#[test]
fn pipeline_failure_reports_relative_durations_without_changing_packet_identity_or_values() {
    let p = packet(15, 12, 2_000_000_000, PpduType::HtLegacy, 64);
    let before = serde_json::to_vec(&p.summary()).unwrap();
    let iq = p.native_frame.native_iq.clone();
    let magnitudes = p.amplitudes.clone();
    let id = p.native_frame.frame_id;
    // Distinct synthetic dequeue spacing and processing interval are NOT clocks.
    let timing = ReceiverPipelineTiming::for_packet(&p, Some(30_303), Some(42), Some(60_000), Some(60_050));
    let detail = timing.failure_detail(&p, PRIMARY).unwrap();
    assert_eq!(detail, format!("{PRIMARY} deq_us=30303 parse_us=42 lock_us=60000 stamp_age_us=60050"));
    assert_eq!(serde_json::to_vec(&p.summary()).unwrap(), before);
    assert_eq!(p.native_frame.native_iq, iq);
    assert_eq!(p.amplitudes, magnitudes);
    assert_eq!(p.native_frame.frame_id, id);
    assert!(!detail.contains("owner") && !detail.contains("token") && !detail.contains("timestamp"));
}

#[test]
fn receiver_timing_never_attaches_another_packet_or_phy_class() {
    let p = packet(15, 12, 2_000_000_000, PpduType::HtLegacy, 64);
    let timing = ReceiverPipelineTiming::for_packet(&p, Some(30_303), None, Some(12), None);
    for other in [packet(16, 12, 2_000_000_000, PpduType::HtLegacy, 64),
        packet(15, 13, 2_000_000_000, PpduType::HtLegacy, 64),
        packet(15, 12, 2_000_000_001, PpduType::HtLegacy, 64),
        packet(15, 12, 2_000_000_000, PpduType::HeSu, 256)] {
        assert!(timing.failure_detail(&other, PRIMARY).is_none());
    }
    assert!(timing.failure_detail(&p, "temporal_missing_segment_source").is_none());
}

#[test]
fn failure_primary_is_preserved_and_unknown_durations_are_not_fabricated() {
    let p = packet(15, 12, 2_000_000_000, PpduType::HtLegacy, 64);
    let timing = ReceiverPipelineTiming::for_packet(&p, None, Some(u32::MAX), None, Some(0));
    let detail = timing.failure_detail(&p, PRIMARY).unwrap();
    assert!(detail.starts_with(PRIMARY));
    assert!(detail.ends_with("deq_us=na parse_us=4294967295 lock_us=na stamp_age_us=0"));
    assert!(detail.len() <= 256);
    let long_primary = format!("{PRIMARY}{}", "x".repeat(256 - PRIMARY.len()));
    assert!(timing.failure_detail(&p, &long_primary).is_none(), "never truncate the primary to make metadata fit");
}
