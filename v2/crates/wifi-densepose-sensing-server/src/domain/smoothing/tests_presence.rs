use crate::domain::classification::ClassificationInfo;
use crate::domain::fusion::corr_persons_to_score;
use crate::domain::node::{node_inference_for, update_node_person_count, NodeState};
use crate::domain::smoothing::presence::smooth_and_classify_node;
use crate::dsp::raw_classify;

const QUIET_ROOM_MOTION: f64 = 0.117;

#[test]
fn node_presence_matches_debounced_level() {
    for raw_motion in [0.0, QUIET_ROOM_MOTION, 0.2, 1.0] {
        let mut ns = NodeState::new();
        ns.csi_fps_ema = 10.0;
        for frame in 0..600 {
            let mut c = ClassificationInfo {
                motion_level: raw_classify(raw_motion),
                presence: raw_motion > 0.04,
                confidence: 0.5,
            };
            smooth_and_classify_node(&mut ns, &mut c, raw_motion);
            assert_eq!(
                c.presence,
                c.motion_level != "absent",
                "raw_motion={raw_motion} frame={frame} level={} sm={}",
                c.motion_level,
                ns.smoothed_motion
            );
        }
    }
}

#[test]
fn quiet_node_reports_absent_and_no_presence() {
    let mut ns = NodeState::new();
    ns.csi_fps_ema = 10.0;
    let mut c = ClassificationInfo {
        motion_level: "absent".into(),
        presence: false,
        confidence: 0.5,
    };
    for _ in 0..600 {
        smooth_and_classify_node(&mut ns, &mut c, QUIET_ROOM_MOTION);
    }
    assert!(ns.smoothed_motion > 0.03 && ns.smoothed_motion <= 0.04);
    assert_eq!(c.motion_level, "absent");
    assert!(
        !c.presence,
        "an absent node must not gate a person count of 1"
    );
}

#[test]
fn absent_node_count_is_zero_whatever_the_frame_label() {
    let mut ns = NodeState::new();
    ns.current_motion_level = "absent".into();
    ns.smoothed_person_score = corr_persons_to_score(1);
    ns.prev_person_count = 1;
    update_node_person_count(&mut ns);
    assert_eq!(ns.prev_person_count, 0);
    assert_eq!(
        node_inference_for(&ns, std::time::Instant::now()).classification,
        "absent"
    );
}

#[test]
fn present_node_count_is_at_least_one() {
    let mut ns = NodeState::new();
    ns.current_motion_level = "present_still".into();
    ns.smoothed_person_score = 0.0;
    update_node_person_count(&mut ns);
    assert_eq!(ns.prev_person_count, 1);
    ns.smoothed_person_score = corr_persons_to_score(2);
    update_node_person_count(&mut ns);
    assert_eq!(ns.prev_person_count, 2);
}

#[test]
fn sustained_motion_still_reports_presence() {
    let mut ns = NodeState::new();
    ns.csi_fps_ema = 10.0;
    let mut c = ClassificationInfo {
        motion_level: "absent".into(),
        presence: false,
        confidence: 0.5,
    };
    for _ in 0..600 {
        smooth_and_classify_node(&mut ns, &mut c, 1.0);
    }
    assert_ne!(c.motion_level, "absent");
    assert!(c.presence);
}
