use std::time::{Duration, Instant};

use wifi_densepose_hardware::{SyncPacket, SyncPacketFlags};
use crate::api::nodes::{bool_metric, fleet_role_counts};
use wifi_densepose_sensing_server::domain::node::NodeState;
use wifi_densepose_sensing_server::domain::types::NodeSyncSnapshot;

fn populated_sync(node_id: u8) -> SyncPacket {
    SyncPacket {
        node_id,
        proto_ver: 1,
        flags: SyncPacketFlags {
            is_leader: false,
            is_valid: true,
            smoothed_used: true,
        },
        local_us: 28_798_450,
        epoch_us: 27_634_885,
        sequence: 20,
    }
}

#[test]
fn fresh_node_with_no_sync_returns_none() {
    let ns = NodeState::new();
    assert!(ns.sync_snapshot().is_none());
}

#[test]
fn node_with_latest_sync_produces_correct_snapshot() {
    let mut ns = NodeState::new();
    ns.latest_sync = Some(populated_sync(9));
    ns.latest_sync_at = Some(Instant::now());
    ns.csi_fps_ema = 10.5;
    ns.csi_fps_samples = 42;

    let snap = ns.sync_snapshot().expect("populated state must produce a snapshot");
    assert_eq!(snap.offset_us, 1_163_565);
    assert!(!snap.is_leader);
    assert!(snap.is_valid);
    assert!(snap.smoothed);
    assert_eq!(snap.sequence, 20);
    assert!((snap.csi_fps_ema - 10.5).abs() < 1e-9);
    assert_eq!(snap.csi_fps_samples, 42);
}

#[test]
fn exported_node_rate_is_mature_and_physically_bounded() {
    let mut ns = NodeState::new();
    assert_eq!(ns.measured_sample_rate_hz(), None);

    ns.csi_fps_ema = 74.0;
    ns.csi_fps_samples = 5;
    assert_eq!(ns.measured_sample_rate_hz(), Some(50.0));

    ns.latest_sync = Some(populated_sync(9));
    ns.latest_sync_at = Some(Instant::now());
    assert_eq!(ns.sync_snapshot().unwrap().csi_fps_ema, 50.0);
}

#[test]
fn observe_csi_frame_arrival_recovers_rate_through_udp_bursts() {
    let base = Instant::now();
    let mut ns = NodeState::new();
    ns.csi_fps_ema = 40.0;
    ns.csi_fps_samples = 10;

    for g in 0..200u64 {
        let group_t = base + Duration::from_millis(50 * g);
        ns.observe_csi_frame_arrival(group_t);
        ns.observe_csi_frame_arrival(group_t + Duration::from_micros(40));
    }

    assert!(
        (ns.csi_fps_ema - 40.0).abs() < 2.0,
        "csi_fps_ema must recover the 40 fps ground truth through burst delivery, got {}",
        ns.csi_fps_ema
    );
}

#[test]
fn apply_sync_packet_populates_a_fresh_node() {
    let mut ns = NodeState::new();
    assert!(ns.latest_sync.is_none());
    assert!(ns.latest_sync_at.is_none());

    let now = Instant::now();
    ns.apply_sync_packet(populated_sync(9), now);

    let sync = ns.latest_sync.as_ref().expect("must be populated");
    assert_eq!(sync.node_id, 9);
    assert_eq!(sync.sequence, 20);
    assert_eq!(ns.latest_sync_at, Some(now));
    assert!(ns.sync_snapshot().is_some());
}

#[test]
fn sync_before_first_csi_still_marks_csi_as_first_sensing_frame() {
    let mut ns = NodeState::new();
    let now = Instant::now();
    ns.apply_sync_packet(populated_sync(9), now);

    assert!(
        ns.observe_csi_frame_arrival(now + Duration::from_millis(20)),
        "a sync packet must not consume the first sensing-frame transition"
    );
    assert!(
        !ns.observe_csi_frame_arrival(now + Duration::from_millis(40)),
        "subsequent CSI frames must not re-emit node.online"
    );
}

#[test]
fn apply_sync_packet_overwrites_older_data() {
    let mut ns = NodeState::new();
    let t0 = Instant::now();
    ns.apply_sync_packet(populated_sync(9), t0);

    let mut second = populated_sync(9);
    second.sequence = 40;
    second.local_us = 30_000_000;
    second.epoch_us = 28_834_900;
    let t1 = t0 + Duration::from_secs(2);
    ns.apply_sync_packet(second, t1);

    let cur = ns.latest_sync.as_ref().unwrap();
    assert_eq!(cur.sequence, 40);
    assert_eq!(cur.local_us, 30_000_000);
    assert_eq!(ns.latest_sync_at, Some(t1));
}

#[test]
fn snapshot_staleness_ms_tracks_apply_time() {
    let mut ns = NodeState::new();
    ns.latest_sync = Some(populated_sync(9));
    ns.latest_sync_at = Instant::now().checked_sub(Duration::from_millis(750));

    let snap = ns.sync_snapshot().unwrap();
    let st = snap.staleness_ms.expect("staleness_ms must be present");
    assert!(
        st >= 740 && st < 1250,
        "expected ~750 ms staleness, got {} ms",
        st
    );
}

#[test]
fn fleet_role_counts_classifies_correctly() {
    fn snap(is_leader: bool) -> NodeSyncSnapshot {
        NodeSyncSnapshot {
            offset_us: 0,
            is_leader,
            is_valid: true,
            smoothed: true,
            sequence: 0,
            csi_fps_ema: 10.0,
            csi_fps_samples: 10,
            staleness_ms: Some(0),
        }
    }
    assert_eq!(fleet_role_counts(&[]), (0, 0));
    let snaps = vec![(12u8, snap(true)), (9, snap(false)), (3, snap(false))];
    assert_eq!(fleet_role_counts(&snaps), (1, 2));
    assert_eq!(
        fleet_role_counts(&[(1u8, snap(true)), (2, snap(true))]),
        (2, 0)
    );
}

#[test]
fn bool_metric_returns_zero_or_one_as_text() {
    assert_eq!(bool_metric(true), "1");
    assert_eq!(bool_metric(false), "0");
}

#[test]
fn mesh_aligned_us_honors_9s_staleness_gate() {
    let mut ns = NodeState::new();
    let now = Instant::now();
    ns.latest_sync = Some(populated_sync(9));

    ns.latest_sync_at = now.checked_sub(Duration::from_secs(1));
    assert!(
        ns.mesh_aligned_us_for_csi_frame(20).is_some(),
        "1 s old sync must produce a mesh-aligned timestamp"
    );

    ns.latest_sync_at = now.checked_sub(Duration::from_secs(8));
    assert!(
        ns.mesh_aligned_us_for_csi_frame(20).is_some(),
        "8 s old sync must still be inside the 9 s gate"
    );

    ns.latest_sync_at = now.checked_sub(Duration::from_secs(10));
    assert!(
        ns.mesh_aligned_us_for_csi_frame(20).is_none(),
        "10 s old sync must trigger the 9 s staleness gate"
    );
}

#[test]
fn latest_csi_mesh_time_requires_both_validity_signals() {
    let now = Instant::now();
    let mut ns = NodeState::new();
    ns.apply_sync_packet(populated_sync(9), now);

    ns.observe_accepted_csi_frame(21, false, now);
    assert!(
        ns.mesh_aligned_us_for_latest_csi_frame().is_none(),
        "an unsynchronized CSI capture must use the host-time fallback"
    );

    ns.observe_accepted_csi_frame(21, true, now + Duration::from_millis(50));
    assert_eq!(
        ns.mesh_aligned_us_for_latest_csi_frame(),
        Some(27_684_885)
    );

    ns.latest_sync.as_mut().unwrap().flags.is_valid = false;
    assert!(
        ns.mesh_aligned_us_for_latest_csi_frame().is_none(),
        "an invalid sync packet must not timestamp even a flagged CSI frame"
    );
}

#[test]
fn snapshot_reflects_leader_state() {
    let mut ns = NodeState::new();
    let mut s = populated_sync(12);
    s.flags = SyncPacketFlags {
        is_leader: true,
        is_valid: true,
        smoothed_used: false,
    };
    s.local_us = 28_864_932;
    s.epoch_us = 28_864_939;
    ns.latest_sync = Some(s);
    ns.latest_sync_at = Some(Instant::now());

    let snap = ns.sync_snapshot().unwrap();
    assert!(snap.is_leader);
    assert_eq!(snap.offset_us, -7);
    assert!(!snap.smoothed);
}

