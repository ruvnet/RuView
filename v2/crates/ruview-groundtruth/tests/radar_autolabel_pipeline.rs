//! End-to-end on SYNTHETIC data: recording + radar JSONL text through
//! labelling, a session-disjoint split and the baseline.

use std::collections::BTreeSet;

use ruview_groundtruth::{
    auto_label, majority_baseline, parse_radar_jsonl, parse_recording_jsonl,
    session_disjoint_split, AutoLabelConfig, LabelledWindow, Occupancy, RadarAdapterConfig,
    RadarLabels,
};

/// A session: 10 Hz CSI and one 1 s radar window per entry of `counts`. The
/// radar clock runs `skew_ms` behind the recording.
fn session(t0_ms: i64, skew_ms: i64, counts: &[u32]) -> (String, String) {
    let mut rec = String::new();
    for i in 0..counts.len() as i64 * 10 {
        let t = (t0_ms + i * 100) as f64 / 1000.0;
        rec.push_str(&format!(
            "{{\"timestamp\":{t},\"subcarriers\":[1.0,{}],\"rssi\":-55.0,\"noise_floor\":-90.0,\"features\":{{}}}}\n",
            1 + i % 3
        ));
    }
    let mut radar = String::new();
    for (i, c) in counts.iter().enumerate() {
        let start = t0_ms - skew_ms + i as i64 * 1000;
        let targets: Vec<String> = (0..*c)
            .map(|k| format!("{{\"x_m\":0.2,\"y_m\":{}}}", 1 + k))
            .collect();
        radar.push_str(&format!(
            "{{\"timestamp_ms\":{},\"window_start_ms\":{start},\"health\":\"ok\",\"source\":{{\"verified\":true}},\"target_count\":{c},\"targets\":[{}]}}\n",
            start + 1000,
            targets.join(",")
        ));
    }
    (rec, radar)
}

fn label(name: &str, t0_ms: i64, counts: &[u32]) -> Vec<LabelledWindow> {
    let skew = 2_500;
    let (rec, radar) = session(t0_ms, skew, counts);
    let frames = parse_recording_jsonl(&rec).unwrap();
    let windows = parse_radar_jsonl(&radar).unwrap();
    let labels =
        RadarLabels::from_windows(name, "ld2450", &windows, RadarAdapterConfig::default()).unwrap();
    let cfg = AutoLabelConfig {
        radar_offset_ms: skew,
        min_csi_frames: 5,
        empty_hold_ms: 0,
    };
    let out = auto_label(name, &frames, &labels, cfg).unwrap();
    assert_eq!(out.windows.len(), counts.len(), "{name}: {:?}", out.skipped);
    out.windows
}

#[test]
fn synthetic_sessions_label_split_and_baseline() {
    let mut all = label("a", 1_000_000, &[0, 1, 1, 2, 0]);
    all.extend(label("b", 2_000_000, &[0, 0, 0, 1]));
    all.extend(label("c", 3_000_000, &[1, 1, 0]));

    // The clock offset lands each label on its own second of CSI.
    assert!(all.iter().all(|w| w.csi.n_frames == 10));
    assert_eq!(all[3].person_count, Some(2));
    assert_eq!(all[0].occupancy, Occupancy::Empty);
    // Radar-derived labels are never human ground truth.
    assert!(all.iter().all(|w| w.evidence == ruview_groundtruth::EvidenceGrade::Claimed));

    let split = session_disjoint_split(&all, &["b"]).unwrap();
    let train: BTreeSet<_> = split.train.iter().map(|w| w.session.clone()).collect();
    let test: BTreeSet<_> = split.test.iter().map(|w| w.session.clone()).collect();
    assert!(train.is_disjoint(&test));

    // Train a+c: occupied 5 of 8 -> majority occupied; test b is 1 of 4 occupied.
    let b = majority_baseline(&split.train, &split.test).unwrap();
    assert_eq!(b.majority_occupancy, Occupancy::Occupied);
    assert!((b.majority_accuracy - 0.25).abs() < 1e-12);
    // Train a+c counts sorted: 0,0,0,1,1,1,1,2 -> median 1.
    assert!((b.median_count.unwrap() - 1.0).abs() < 1e-12);

    // Same inputs, same outputs.
    let again = majority_baseline(&split.train, &split.test).unwrap();
    assert_eq!(b, again);
}
