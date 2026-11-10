//! Field position attachment and temporal smoothing.
//!
//! Complies with ELMAS.md: <= 350 lines per file, clean abstraction.

use crate::field_localize;
use crate::domain::node::NodeState;
use crate::domain::types::{PersonDetection, SensingUpdate};

/// Attach real, field-derived per-person world positions to a `SensingUpdate`'s
/// `persons` (issue #1050).
pub fn attach_field_positions(update: &mut SensingUpdate) {
    let Some(persons) = update.persons.as_mut() else {
        return;
    };
    if persons.is_empty() {
        return;
    }

    let [nx, _ny, nz] = update.signal_field.grid_size;
    let peaks = field_localize::extract_peaks(
        &update.signal_field.values,
        nx,
        nz,
        persons.len().max(1),
        3.0,
    );

    let motion_score = field_localize::motion_score_from_power(update.features.motion_band_power);
    let pose_label = update.posture.clone();

    for (i, person) in persons.iter_mut().enumerate() {
        if let Some(peak) = peaks.get(i).or_else(|| peaks.first()) {
            person.position = peak.position;
        }
        person.motion_score = motion_score;
        person.pose = pose_label.clone();
    }
}

pub fn apply_temporal_smoothing(persons: &mut [PersonDetection], ns: &mut NodeState) {
    if persons.is_empty() {
        return;
    }
    let alpha = ns.ema_alpha();
    crate::pose::apply_temporal_smoothing_to_detection(
        &mut persons[0],
        &mut ns.prev_keypoints,
        alpha,
    );
}
