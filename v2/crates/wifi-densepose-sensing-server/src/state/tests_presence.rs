use wifi_densepose_sensing_server::domain::classification::ClassificationInfo;
use wifi_densepose_sensing_server::dsp::raw_classify;
use crate::state::methods::smooth_and_classify;
use crate::state::AppStateInner;

const QUIET_ROOM_MOTION: f64 = 0.117;

#[test]
fn room_presence_matches_debounced_level() {
    for raw_motion in [0.0, QUIET_ROOM_MOTION, 0.2, 1.0] {
        let mut s = AppStateInner::minimal();
        for frame in 0..600 {
            let mut c = ClassificationInfo {
                motion_level: raw_classify(raw_motion),
                presence: raw_motion > 0.04,
                confidence: 0.5,
            };
            smooth_and_classify(&mut s, &mut c, raw_motion);
            assert_eq!(
                c.presence,
                c.motion_level != "absent",
                "raw_motion={raw_motion} frame={frame} level={} sm={}",
                c.motion_level,
                s.smoothed_motion
            );
        }
    }
}

