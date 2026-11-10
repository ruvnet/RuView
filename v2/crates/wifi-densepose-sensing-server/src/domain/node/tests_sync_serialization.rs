use crate::domain::types::{NodeInfo, NodeSyncSnapshot};

fn sample_sync() -> NodeSyncSnapshot {
    NodeSyncSnapshot {
        offset_us: 1_163_565,
        is_leader: false,
        is_valid: true,
        smoothed: true,
        sequence: 20,
        csi_fps_ema: 10.0,
        csi_fps_samples: 47,
        staleness_ms: Some(120),
    }
}

fn sample_node(sync: Option<NodeSyncSnapshot>) -> NodeInfo {
    NodeInfo {
        node_id: 9,
        device_id: None,
        rssi_dbm: -38.0,
        position: [2.0, 0.0, 1.5],
        amplitude: vec![],
        subcarrier_count: 0,
        sync,
        node_inference: None,
        mediatek_diagnostics: None,
    }
}

#[test]
fn sync_present_serializes_all_seven_fields() {
    let v = serde_json::to_value(sample_node(Some(sample_sync()))).unwrap();
    let s = v.get("sync").expect("sync key must be present");
    for key in [
        "offset_us",
        "is_leader",
        "is_valid",
        "smoothed",
        "sequence",
        "csi_fps_ema",
        "csi_fps_samples",
        "staleness_ms",
    ] {
        assert!(
            s.get(key).is_some(),
            "sync object missing field `{key}` — UI contract broken"
        );
    }
    assert_eq!(s["offset_us"], 1_163_565);
    assert_eq!(s["is_leader"], false);
    assert_eq!(s["sequence"], 20);
    assert_eq!(s["csi_fps_samples"], 47);
}

#[test]
fn sync_absent_omits_the_key_entirely() {
    let v = serde_json::to_value(sample_node(None)).unwrap();
    assert!(
        v.get("sync").is_none(),
        "expected `sync` key omitted when None, got {:?}",
        v.get("sync")
    );
    assert_eq!(v["node_id"], 9);
    assert_eq!(v["rssi_dbm"], -38.0);
}

#[test]
fn sync_round_trips_through_serde() {
    let original = sample_node(Some(sample_sync()));
    let json = serde_json::to_string(&original).unwrap();
    let parsed: NodeInfo = serde_json::from_str(&json).unwrap();
    let s_orig = original.sync.unwrap();
    let s_parsed = parsed.sync.expect("sync should survive round-trip");
    assert_eq!(s_parsed.offset_us, s_orig.offset_us);
    assert_eq!(s_parsed.is_leader, s_orig.is_leader);
    assert_eq!(s_parsed.is_valid, s_orig.is_valid);
    assert_eq!(s_parsed.smoothed, s_orig.smoothed);
    assert_eq!(s_parsed.sequence, s_orig.sequence);
    assert!((s_parsed.csi_fps_ema - s_orig.csi_fps_ema).abs() < 1e-9);
    assert_eq!(s_parsed.csi_fps_samples, s_orig.csi_fps_samples);
}

