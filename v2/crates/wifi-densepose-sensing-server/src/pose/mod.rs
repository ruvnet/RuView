//! Skeleton pose derivation and temporal smoothing algorithms.

pub mod synthesis;
pub mod temporal;

pub use synthesis::{derive_pose_from_sensing, derive_single_person_pose};
pub use temporal::{
    apply_temporal_smoothing_to_detection, clamp_bone_lengths_f64, dist_f64, MAX_BONE_CHANGE_RATIO,
    POSE_BONE_PAIRS,
};

