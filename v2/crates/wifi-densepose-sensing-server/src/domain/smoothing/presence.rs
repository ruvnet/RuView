use crate::domain::types::ClassificationInfo;
use crate::dsp::raw_classify;

pub const DEBOUNCE_FRAMES: u32 = 4;
pub const MOTION_EMA_ALPHA: f64 = 0.15;
pub const BASELINE_EMA_ALPHA: f64 = 0.003;
pub const BASELINE_WARMUP: u64 = 50;

pub const NODE_DEBOUNCE_DURATION_SECS: f64 = 0.4;
pub const NODE_MOTION_TIME_CONSTANT_SECS: f64 = 0.6154;
pub const NODE_BASELINE_TIME_CONSTANT_SECS: f64 = 33.28;
pub const NODE_BASELINE_WARMUP_SECS: f64 = 5.0;

/// Smooth motion score and debounce classification state transitions.
///
/// Invariant to CSI frame arrival rate: derives time step from measured `fps`.
pub fn smooth_and_classify_parts(
    baseline_frames: &mut u64,
    baseline_motion: &mut f64,
    smoothed_motion: &mut f64,
    current_motion_level: &mut String,
    debounce_counter: &mut u32,
    debounce_candidate: &mut String,
    fps: f64,
    raw: &mut ClassificationInfo,
    raw_motion: f64,
) {
    let effective_fps = fps.max(1.0);
    let dt = 1.0 / effective_fps;
    let motion_alpha = 1.0 - (-dt / NODE_MOTION_TIME_CONSTANT_SECS).exp();
    let baseline_alpha = 1.0 - (-dt / NODE_BASELINE_TIME_CONSTANT_SECS).exp();
    let debounce_frames_needed = ((NODE_DEBOUNCE_DURATION_SECS * effective_fps).round() as u32).max(1);
    let warmup_frames_needed = ((NODE_BASELINE_WARMUP_SECS * effective_fps).round() as u64).max(1);

    // 1. Adaptive baseline: track quiet room floor during calm periods.
    *baseline_frames += 1;
    if *baseline_frames < warmup_frames_needed {
        *baseline_motion = *baseline_motion * 0.9 + raw_motion * 0.1;
    } else if raw_motion < *smoothed_motion + 0.05 {
        *baseline_motion = *baseline_motion * (1.0 - baseline_alpha) + raw_motion * baseline_alpha;
    }

    // 2. Subtract baseline and clamp.
    let adjusted = (raw_motion - *baseline_motion * 0.7).max(0.0);

    // 3. EMA smooth the adjusted score.
    *smoothed_motion = *smoothed_motion * (1.0 - motion_alpha) + adjusted * motion_alpha;
    let sm = *smoothed_motion;

    // 4. Classify from smoothed score.
    let candidate = raw_classify(sm);

    // 5. Hysteresis debounce: require N consecutive frames agreeing on a new state.
    if candidate == *current_motion_level {
        *debounce_counter = 0;
        *debounce_candidate = candidate;
    } else if candidate == *debounce_candidate {
        *debounce_counter += 1;
        if *debounce_counter >= debounce_frames_needed {
            *current_motion_level = candidate;
            *debounce_counter = 0;
        }
    } else {
        *debounce_candidate = candidate;
        *debounce_counter = 1;
    }

    // 6. Write smoothed result back into classification.
    raw.motion_level = current_motion_level.clone();
    raw.presence = raw.motion_level != "absent";
    raw.confidence = (0.4 + sm * 0.6).clamp(0.0, 1.0);
}

pub fn smooth_and_classify_node(
    ns: &mut crate::domain::node::NodeState,
    raw: &mut ClassificationInfo,
    raw_motion: f64,
) {
    smooth_and_classify_parts(
        &mut ns.baseline_frames,
        &mut ns.baseline_motion,
        &mut ns.smoothed_motion,
        &mut ns.current_motion_level,
        &mut ns.debounce_counter,
        &mut ns.debounce_candidate,
        ns.csi_fps_ema.max(1.0),
        raw,
        raw_motion,
    );
}

#[derive(Debug, Clone)]
pub struct PresenceFilter {
    pub baseline_frames: u64,
    pub baseline_motion: f64,
    pub smoothed_motion: f64,
    pub current_motion_level: String,
    pub debounce_counter: u32,
    pub debounce_candidate: String,
}

impl Default for PresenceFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl PresenceFilter {
    pub fn new() -> Self {
        Self {
            baseline_frames: 0,
            baseline_motion: 0.0,
            smoothed_motion: 0.0,
            current_motion_level: "absent".into(),
            debounce_counter: 0,
            debounce_candidate: "absent".into(),
        }
    }

    pub fn smooth(&mut self, raw: &mut ClassificationInfo, raw_motion: f64, fps: f64) {
        smooth_and_classify_parts(
            &mut self.baseline_frames,
            &mut self.baseline_motion,
            &mut self.smoothed_motion,
            &mut self.current_motion_level,
            &mut self.debounce_counter,
            &mut self.debounce_candidate,
            fps,
            raw,
            raw_motion,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUIET_ROOM_MOTION: f64 = 0.117;

    #[test]
    fn presence_matches_debounced_level() {
        for raw_motion in [0.0, QUIET_ROOM_MOTION, 0.2, 1.0] {
            let mut filter = PresenceFilter::new();
            for _ in 0..600 {
                let mut c = ClassificationInfo {
                    motion_level: raw_classify(raw_motion),
                    presence: raw_motion > 0.04,
                    confidence: 0.5,
                };
                filter.smooth(&mut c, raw_motion, 10.0);
                assert_eq!(
                    c.presence,
                    c.motion_level != "absent",
                    "raw_motion={raw_motion} level={} sm={}",
                    c.motion_level,
                    filter.smoothed_motion
                );
            }
        }
    }

    #[test]
    fn quiet_reports_absent_and_no_presence() {
        let mut filter = PresenceFilter::new();
        let mut c = ClassificationInfo {
            motion_level: "absent".into(),
            presence: false,
            confidence: 0.5,
        };
        for _ in 0..600 {
            filter.smooth(&mut c, QUIET_ROOM_MOTION, 10.0);
        }
        assert!(filter.smoothed_motion > 0.03 && filter.smoothed_motion <= 0.04);
        assert_eq!(c.motion_level, "absent");
        assert!(!c.presence);
    }

    #[test]
    fn sustained_motion_reports_presence() {
        let mut filter = PresenceFilter::new();
        let mut c = ClassificationInfo {
            motion_level: "absent".into(),
            presence: false,
            confidence: 0.5,
        };
        for _ in 0..600 {
            filter.smooth(&mut c, 1.0, 10.0);
        }
        assert_ne!(c.motion_level, "absent");
        assert!(c.presence);
    }
}

