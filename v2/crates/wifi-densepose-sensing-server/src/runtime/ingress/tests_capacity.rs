//! MediaTek room update capacity and single-device tests.
//!
//! Complies with ELMAS.md: <= 350 lines, deterministic test harness.

use std::time::Duration;
use super::mediatek_room::{
    clamp_csi_ring, evict_mediatek_device_if_full, mediatek_room_update,
    MAX_CSI_RING_CAPACITY, MAX_MEDIATEK_DEVICES,
};
use crate::mediatek_activity;
use crate::mediatek_csi;
use crate::mediatek_heuristic;
use crate::AppStateInner;
use wifi_densepose_sensing_server::domain::node::constants::ESP32_OFFLINE_TIMEOUT;

pub fn seed_device(
    s: &mut AppStateInner,
    device_id: &str,
    start: std::time::Instant,
    samples: &[f64],
    rssi: i8,
) -> std::time::Instant {
    use wifi_densepose_hardware::mediatek_csi::{
        ChipsetProfile, CsiFlags, CsiPayload, PpduType, ReportKind,
    };
    let mut hist = mediatek_heuristic::DeviceHistory::default();
    let mut t = start;
    for (i, &sample) in samples.iter().enumerate() {
        if i > 0 {
            t += Duration::from_millis(500);
        }
        hist.observe(t, sample, sample);
    }
    s.mediatek_heuristic_by_device
        .insert(device_id.to_string(), hist);
    let frame = wifi_densepose_hardware::mediatek_csi::CsiFrame {
        report_kind: ReportKind::Csi,
        sequence: samples.len() as u32,
        timestamp_us: 0,
        device_id: u64::from_str_radix(device_id, 16).unwrap_or(0),
        chipset: ChipsetProfile::Mt7981Mt7976,
        bandwidth_mhz: 80,
        center_freq_khz: 5_210_000,
        flags: CsiFlags(CsiFlags::CALIBRATED),
        tx_count: 2,
        rx_count: 1,
        ppdu_type: PpduType::HeSu,
        subcarrier_count: 4,
        noise_floor_dbm: -95,
        scale: 1.0,
        subcarrier_spacing_hz: 312_500.0,
        calibration_id: 1,
        payload: CsiPayload::ComplexI16 {
            rssi_dbm: vec![rssi],
            values: vec![[0, 0]; 8],
        },
    };
    let snapshot = mediatek_csi::MediatekCsiSnapshot::from_frame(&frame);
    s.mediatek_csi_by_device
        .insert(device_id.to_string(), (snapshot, t));
    t
}

pub const SETTLE_SAMPLES: usize = 40;

#[test]
fn device_cap_evicts_oldest_updated_device() {
    let mut s = AppStateInner::minimal();
    let start = std::time::Instant::now();
    for i in 0..MAX_MEDIATEK_DEVICES {
        let id = format!("{:016x}", i + 1);
        let t = start + Duration::from_millis(i as u64);
        seed_device(&mut s, &id, t, &[1.0], -50);
        s.mediatek_activity_by_device
            .insert(id, mediatek_activity::ActivityTracker::default());
    }
    let oldest = format!("{:016x}", 1);
    evict_mediatek_device_if_full(&mut s, &oldest);
    assert_eq!(s.mediatek_csi_by_device.len(), MAX_MEDIATEK_DEVICES);
    evict_mediatek_device_if_full(&mut s, "ffffffffffffffff");
    assert_eq!(s.mediatek_csi_by_device.len(), MAX_MEDIATEK_DEVICES - 1);
    assert!(!s.mediatek_csi_by_device.contains_key(&oldest));
    assert!(!s.mediatek_heuristic_by_device.contains_key(&oldest));
    assert!(!s.mediatek_activity_by_device.contains_key(&oldest));
    assert!(!s.mediatek_csi_ring_by_device.contains_key(&oldest));
}

#[test]
fn csi_ring_is_clamped() {
    assert_eq!(clamp_csi_ring(0), 1);
    assert_eq!(clamp_csi_ring(256), 256);
    assert_eq!(clamp_csi_ring(usize::MAX), MAX_CSI_RING_CAPACITY);
}

#[test]
fn empty_room_flat_amplitudes_is_absent() {
    let mut s = AppStateInner::minimal();
    let flat: Vec<f64> = std::iter::repeat_n(100.0, SETTLE_SAMPLES).collect();
    let now = seed_device(
        &mut s,
        "a9be7e5bb1644d3a",
        std::time::Instant::now(),
        &flat,
        -50,
    );

    let update = mediatek_room_update(
        &mut s,
        now,
        ESP32_OFFLINE_TIMEOUT,
        1,
        "mediatek:physical-unvalidated".to_string(),
    );
    assert!(!update.classification.presence);
    assert_eq!(update.classification.motion_level, "absent");
    assert_eq!(
        update.classifier.as_deref(),
        Some(mediatek_heuristic::CLASSIFIER_ID)
    );
    assert_eq!(update.nodes.len(), 1);
    assert_eq!(
        update.nodes[0].device_id.as_deref(),
        Some("a9be7e5bb1644d3a")
    );
    assert_eq!(
        update.nodes[0]
            .mediatek_diagnostics
            .as_ref()
            .map(|d| d.presence_state.as_str()),
        Some("absent")
    );
}

#[test]
fn injected_variance_is_present_moving() {
    let mut s = AppStateInner::minimal();
    let swinging: Vec<f64> = (0..SETTLE_SAMPLES)
        .map(|i| if i % 2 == 0 { 100.0 } else { 130.0 })
        .collect();
    let now = seed_device(
        &mut s,
        "a9be7e5bb1644d3a",
        std::time::Instant::now(),
        &swinging,
        -50,
    );

    let update = mediatek_room_update(
        &mut s,
        now,
        ESP32_OFFLINE_TIMEOUT,
        1,
        "mediatek:physical-unvalidated".to_string(),
    );
    assert!(update.classification.presence);
    assert_eq!(update.classification.motion_level, "present_moving");
}

#[test]
fn low_confidence_device_abstains_instead_of_asserting_absent() {
    let mut s = AppStateInner::minimal();
    let now = seed_device(
        &mut s,
        "a9be7e5bb1644d3a",
        std::time::Instant::now(),
        &[1800.0],
        -49,
    );

    let update = mediatek_room_update(
        &mut s,
        now,
        ESP32_OFFLINE_TIMEOUT,
        1,
        "mediatek:physical-unvalidated".to_string(),
    );
    assert_eq!(update.nodes.len(), 1);
    assert_eq!(
        update.nodes[0]
            .mediatek_diagnostics
            .as_ref()
            .map(|d| d.presence_state.as_str()),
        Some("unknown")
    );
    assert!(
        update.nodes[0].node_inference.is_none(),
        "an unknown device must abstain, not vote"
    );
    assert_eq!(update.classification.motion_level, "absent");
    assert_eq!(update.classification.confidence, 0.0);
}

