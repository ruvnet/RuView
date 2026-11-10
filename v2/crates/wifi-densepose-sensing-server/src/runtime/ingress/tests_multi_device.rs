//! MediaTek room update multi-device agreement and activity timeline tests.
//!
//! Complies with ELMAS.md: <= 350 lines, deterministic multi-device harness.

use std::time::Duration;
use super::mediatek_room::mediatek_room_update;
use super::tests_capacity::{seed_device, SETTLE_SAMPLES};
use crate::mediatek_activity;
use crate::mediatek_csi;
use crate::mediatek_heuristic;
use crate::AppStateInner;
use wifi_densepose_sensing_server::domain::node::constants::ESP32_OFFLINE_TIMEOUT;

#[test]
fn a_single_transient_dip_does_not_flip_the_room_to_absent() {
    let mut s = AppStateInner::minimal();
    let swinging: Vec<f64> = (0..SETTLE_SAMPLES)
        .map(|i| if i % 2 == 0 { 100.0 } else { 130.0 })
        .collect();
    let mut samples = swinging.clone();
    samples.push(115.0);
    let now = seed_device(
        &mut s,
        "a9be7e5bb1644d3a",
        std::time::Instant::now(),
        &samples,
        -50,
    );

    let update = mediatek_room_update(
        &mut s,
        now,
        ESP32_OFFLINE_TIMEOUT,
        1,
        "mediatek:physical-unvalidated".to_string(),
    );
    assert!(
        update.classification.presence,
        "a single dip must not flip the room to absent"
    );
}

#[test]
fn two_devices_disagreeing_lowers_room_confidence_vs_single_device() {
    let swinging: Vec<f64> = (0..SETTLE_SAMPLES)
        .map(|i| if i % 2 == 0 { 100.0 } else { 130.0 })
        .collect();
    let flat: Vec<f64> = std::iter::repeat_n(100.0, SETTLE_SAMPLES).collect();

    let mut solo = AppStateInner::minimal();
    let solo_now = seed_device(
        &mut solo,
        "a9be7e5bb1644d3a",
        std::time::Instant::now(),
        &swinging,
        -50,
    );
    let solo_update = mediatek_room_update(
        &mut solo,
        solo_now,
        ESP32_OFFLINE_TIMEOUT,
        1,
        "mediatek:physical-unvalidated".to_string(),
    );

    let mut pair = AppStateInner::minimal();
    let start = std::time::Instant::now();
    seed_device(&mut pair, "a9be7e5bb1644d3a", start, &swinging, -50);
    let pair_now = seed_device(&mut pair, "a9be7d5bb1644b87", start, &flat, -55);
    let pair_update = mediatek_room_update(
        &mut pair,
        pair_now,
        ESP32_OFFLINE_TIMEOUT,
        1,
        "mediatek:physical-unvalidated".to_string(),
    );

    assert_eq!(
        pair_update.nodes.len(),
        2,
        "both devices must vote, not overwrite each other"
    );
    let moving_node = pair_update
        .nodes
        .iter()
        .find(|n| n.device_id.as_deref() == Some("a9be7e5bb1644d3a"))
        .expect("moving device present");
    let moving_confidence = moving_node.node_inference.as_ref().unwrap().confidence;
    let solo_confidence = solo_update.classification.confidence;
    assert!(
        moving_confidence < solo_confidence,
        "disagreement should lower confidence: solo={solo_confidence} paired={moving_confidence}"
    );
}

#[test]
fn bursty_interleaved_two_device_timeline_stays_stable() {
    let mut s = AppStateInner::minimal();
    let start = std::time::Instant::now();
    let mut t = start;
    let mut hist_a = mediatek_heuristic::DeviceHistory::default();
    let mut hist_b = mediatek_heuristic::DeviceHistory::default();

    let gaps_ms = [300u64, 4000, 500, 3500, 300, 4000, 500, 3000, 300, 4000];
    for (i, &gap) in gaps_ms.iter().enumerate() {
        t += Duration::from_millis(gap);
        let sample_a = if i % 2 == 0 { 100.0 } else { 130.0 };
        hist_a.observe(t, sample_a, sample_a + 2.0);
        hist_b.observe(t, 1600.0, 1650.0);
    }
    let (t_a, t_b) = (t, t);

    let now = t;
    s.mediatek_heuristic_by_device
        .insert("dev-a".to_string(), hist_a);
    s.mediatek_heuristic_by_device
        .insert("dev-b".to_string(), hist_b);
    for (id, seen) in [("dev-a", t_a), ("dev-b", t_b)] {
        use wifi_densepose_hardware::mediatek_csi::{
            ChipsetProfile, CsiFlags, CsiPayload, PpduType, ReportKind,
        };
        let frame = wifi_densepose_hardware::mediatek_csi::CsiFrame {
            report_kind: ReportKind::Csi,
            sequence: 0,
            timestamp_us: 0,
            device_id: 0,
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
                rssi_dbm: vec![-50],
                values: vec![[0, 0]; 8],
            },
        };
        let snapshot = mediatek_csi::MediatekCsiSnapshot::from_frame(&frame);
        s.mediatek_csi_by_device
            .insert(id.to_string(), (snapshot, seen));
    }

    let update = mediatek_room_update(
        &mut s,
        now,
        ESP32_OFFLINE_TIMEOUT,
        1,
        "mediatek:physical-unvalidated".to_string(),
    );
    assert_eq!(update.nodes.len(), 2);
    let dev_a = update
        .nodes
        .iter()
        .find(|n| n.device_id.as_deref() == Some("dev-a"))
        .unwrap();
    let diag = dev_a.mediatek_diagnostics.as_ref().unwrap();
    assert!(
        diag.sample_count >= 3,
        "sample_count was {}",
        diag.sample_count
    );
}

#[test]
fn stale_device_is_excluded_from_activity_but_fresh_one_reports_age_ms() {
    let mut s = AppStateInner::minimal();
    let stale_seen = std::time::Instant::now();
    let now =
        stale_seen + mediatek_activity::ROOM_STALE_AFTER + std::time::Duration::from_secs(1);

    let mut fresh = mediatek_activity::ActivityTracker::default();
    fresh.observe(now, 0.5, 10);
    s.mediatek_activity_by_device
        .insert("dev-fresh".to_string(), fresh);

    let mut stale = mediatek_activity::ActivityTracker::default();
    stale.observe(stale_seen, 0.5, 10);
    s.mediatek_activity_by_device
        .insert("dev-stale".to_string(), stale);

    let update = mediatek_room_update(
        &mut s,
        now,
        ESP32_OFFLINE_TIMEOUT,
        1,
        "mediatek:physical-unvalidated".to_string(),
    );
    let activity = update.activity.expect("activity block present");
    let devices = activity["devices"].as_object().expect("devices object");
    assert!(
        devices.contains_key("dev-fresh"),
        "the fresh device must be listed"
    );
    assert!(
        !devices.contains_key("dev-stale"),
        "a device stale past ROOM_STALE_AFTER must be dropped"
    );
    assert!(
        devices["dev-fresh"]["age_ms"].as_u64().is_some(),
        "a listed device must report age_ms"
    );
}

