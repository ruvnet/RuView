//! Contract: `scripts/radar_window_logger.py` output parses and labels here.
//!
//! The fixture is real logger output from a SYNTHETIC ESPHome log replayed with
//! `--unverified`, so as recorded every window must label Unknown. The test then
//! marks the same windows verified in memory to check the presence mapping:
//! carried-forward ON/OFF reports label present/absent, and the logger's
//! `health: "stale"` windows (no presence field) stay Unknown instead of
//! becoming "empty".

use ruview_groundtruth::{
    label_window, parse_radar_jsonl, LabelStatus, RadarAdapterConfig, RadarWindowSource,
    UnknownReason,
};

const FIXTURE: &str = include_str!("fixtures/radar_window_logger.jsonl");

#[test]
fn logger_output_parses_and_unverified_windows_are_unknown() {
    let windows = parse_radar_jsonl(FIXTURE).expect("logger JSONL must parse");
    assert_eq!(windows.len(), 17);
    let cfg = RadarAdapterConfig::default();
    for (i, w) in windows.iter().enumerate() {
        let label = label_window(w, i, &cfg);
        assert_eq!(label.status, LabelStatus::Unknown(UnknownReason::Unverified), "window {i}");
    }
}

#[test]
fn verified_logger_windows_map_presence_and_keep_stale_unknown() {
    let mut windows = parse_radar_jsonl(FIXTURE).expect("logger JSONL must parse");
    let cfg = RadarAdapterConfig::default();
    let mut present = 0;
    let mut absent = 0;
    let mut unknown = 0;
    for (i, w) in windows.iter_mut().enumerate() {
        w.source = Some(RadarWindowSource { verified: true, simulated: false });
        let label = label_window(w, i, &cfg);
        match (label.status, label.presence) {
            (LabelStatus::Labelled, Some(true)) => present += 1,
            (LabelStatus::Labelled, Some(false)) => absent += 1,
            (LabelStatus::Unknown(_), None) => unknown += 1,
            other => panic!("window {i}: unexpected {other:?}"),
        }
    }
    // 0-2 s present, 2-7 s absent (carried forward), then 10 stale windows.
    assert_eq!((present, absent, unknown), (2, 5, 10));
}
