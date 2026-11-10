//! Temporal smoothing and filtering algorithms for presence and vitals.

pub mod presence;
pub mod vitals;

#[cfg(test)]
mod tests_presence;

pub use presence::{
    smooth_and_classify_node, smooth_and_classify_parts, PresenceFilter,
    BASELINE_EMA_ALPHA, BASELINE_WARMUP, DEBOUNCE_FRAMES, MOTION_EMA_ALPHA,
    NODE_BASELINE_TIME_CONSTANT_SECS, NODE_BASELINE_WARMUP_SECS, NODE_DEBOUNCE_DURATION_SECS,
    NODE_MOTION_TIME_CONSTANT_SECS,
};
pub use vitals::{
    smooth_vitals_node, smooth_vitals_parts, trimmed_mean, VitalsFilter,
    BR_DEAD_BAND, BR_MAX_JUMP, HR_DEAD_BAND, HR_MAX_JUMP, VITAL_EMA_ALPHA, VITAL_MEDIAN_WINDOW,
};

