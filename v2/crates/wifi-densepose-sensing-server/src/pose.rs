//! Skeleton derivation, pose estimation, and temporal smoothing.

use crate::types::*;

/// Expected bone lengths in pixel-space for the COCO-17 skeleton.
pub const POSE_BONE_PAIRS: &[(usize, usize)] = &[
    (5, 7),
    (7, 9),
    (6, 8),
    (8, 10),
    (5, 11),
    (6, 12),
    (11, 13),
    (13, 15),
    (12, 14),
    (14, 16),
    (5, 6),
    (11, 12),
];

const TORSO_KP: [usize; 4] = [5, 6, 11, 12];
const EXTREMITY_KP: [usize; 4] = [9, 10, 15, 16];

pub fn derive_single_person_pose(
    update: &SensingUpdate,
    person_idx: usize,
    total_persons: usize,
) -> PersonDetection {
    let cls = &update.classification;
    let feat = &update.features;

    let conf_decay = 1.0 - person_idx as f64 * 0.15;
    let base_confidence = cls.confidence * (0.6 + 0.4 * ((feat.variance - 0.5) / 10.0).clamp(0.0, 1.0)) * conf_decay;

    let motion_score = (feat.motion_band_power / 15.0).clamp(0.0, 1.0);
    let is_walking = motion_score > 0.55;
    let breath_amp = (feat.breathing_band_power * 4.0).clamp(0.0, 0.15);

    let breath_phase = if let Some(ref vs) = update.vital_signs {
        let bpm = vs.breathing_rate_bpm.unwrap_or(15.0);
        let freq = (bpm / 60.0).clamp(0.1, 0.5);
        (update.tick as f64 * freq * 0.02 * std::f64::consts::TAU + person_idx as f64 * 2.094).sin()
    } else {
        (update.tick as f64 * 0.02 + person_idx as f64 * 2.094).sin()
    };

    let pose_label = update.posture.as_deref().unwrap_or(if is_walking { "walking" } else { "standing" });

    // Spread persons across the room using person_idx as an offset
    let person_spread = (person_idx as f64 - (total_persons as f64 - 1.0) / 2.0) * 0.5;

    let kp_names = [
        "nose", "left_eye", "right_eye", "left_ear", "right_ear",
        "left_shoulder", "right_shoulder", "left_elbow", "right_elbow",
        "left_wrist", "right_wrist", "left_hip", "right_hip",
        "left_knee", "right_knee", "left_ankle", "right_ankle",
    ];

    // Person center in room coordinates — position will be updated by attach_field_positions
    let center_x = person_spread;
    let center_z = 0.0;

    let (kp_offsets, kp_confs) = pose_offsets(pose_label, motion_score, breath_phase, breath_amp);

    let keypoints: Vec<PoseKeypoint> = kp_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let (dx, dy, dz) = kp_offsets[i];
            let conf = kp_confs[i].min(base_confidence).max(0.1);
            let final_y = dy + breath_amp * breath_phase * 0.1;

            PoseKeypoint {
                name: name.to_string(),
                x: center_x + dx,
                y: final_y,
                z: center_z + dz,
                confidence: conf,
            }
        })
        .collect();

    let xs: Vec<f64> = keypoints.iter().map(|k| k.x).collect();
    let ys: Vec<f64> = keypoints.iter().map(|k| k.y).collect();
    let zs: Vec<f64> = keypoints.iter().map(|k| k.z).collect();
    let min_x = xs.iter().cloned().fold(f64::MAX, f64::min) - 0.1;
    let min_y = ys.iter().cloned().fold(f64::MAX, f64::min) - 0.1;
    let min_z = zs.iter().cloned().fold(f64::MAX, f64::min) - 0.1;
    let max_x = xs.iter().cloned().fold(f64::MIN, f64::max) + 0.1;
    let max_y = ys.iter().cloned().fold(f64::MIN, f64::max) + 0.1;
    let max_z = zs.iter().cloned().fold(f64::MIN, f64::max) + 0.1;

    PersonDetection {
        id: (person_idx + 1) as u32,
        confidence: cls.confidence * conf_decay,
        keypoints,
        bbox: BoundingBox {
            x: min_x,
            y: min_y,
            width: (max_x - min_x).max(0.3),
            height: (max_y - min_y).max(0.5),
        },
        zone: format!("zone_{}", person_idx + 1),
        position: [center_x, 0.0, center_z],
        motion_score,
        pose: update.posture.clone(),
    }
}

/// Return (dx, dy, dz) offsets and per-keypoint confidence multipliers for a given pose label.
/// Room coordinates in meters. Y axis is up.
fn pose_offsets(
    pose: &str,
    motion_score: f64,
    breath_phase: f64,
    breath_amp: f64,
) -> ([(f64, f64, f64); 17], [f64; 17]) {
    // Standing: arms at sides, feet together
    // All positions relative to hip center at (0, 0, 0)
    let standing = [
        (0.0, 1.65, 0.0),          // 0 nose
        (-0.03, 1.67, -0.02),      // 1 left_eye
        (0.03, 1.67, -0.02),       // 2 right_eye
        (-0.07, 1.65, 0.0),        // 3 left_ear
        (0.07, 1.65, 0.0),         // 4 right_ear
        (-0.18, 1.45, -0.05),      // 5 left_shoulder
        (0.18, 1.45, -0.05),       // 6 right_shoulder
        (-0.28, 1.15, -0.05),      // 7 left_elbow
        (0.28, 1.15, -0.05),       // 8 right_elbow
        (-0.35, 0.85, -0.05),      // 9 left_wrist
        (0.35, 0.85, -0.05),       // 10 right_wrist
        (-0.10, 1.05, -0.05),      // 11 left_hip
        (0.10, 1.05, -0.05),       // 12 right_hip
        (-0.12, 0.65, -0.05),      // 13 left_knee
        (0.12, 0.65, -0.05),       // 14 right_knee
        (-0.12, 0.05, -0.05),      // 15 left_ankle
        (0.12, 0.05, -0.05),       // 16 right_ankle
    ];
    let standing_conf = [0.9, 0.8, 0.8, 0.7, 0.7, 0.9, 0.9, 0.85, 0.85, 0.8, 0.8, 0.9, 0.9, 0.85, 0.85, 0.8, 0.8];

    // Walking: legs alternating, arms swing
    let walking = [
        (0.0, 1.65, 0.0),          // 0 nose
        (-0.03, 1.67, -0.02),      // 1 left_eye
        (0.03, 1.67, -0.02),       // 2 right_eye
        (-0.07, 1.65, 0.0),        // 3 left_ear
        (0.07, 1.65, 0.0),         // 4 right_ear
        (-0.18, 1.45, -0.05),      // 5 left_shoulder
        (0.18, 1.45, -0.05),       // 6 right_shoulder
        (-0.30, 1.15, -0.05),      // 7 left_elbow
        (0.25, 1.20, -0.05),       // 8 right_elbow
        (-0.40, 0.80, -0.05),      // 9 left_wrist
        (0.30, 0.90, -0.05),       // 10 right_wrist
        (-0.10, 1.05, -0.05),      // 11 left_hip
        (0.10, 1.05, -0.05),       // 12 right_hip
        (-0.12, 0.35, -0.05),      // 13 left_knee
        (0.15, 0.40, -0.05),       // 14 right_knee
        (-0.12, -0.05, -0.05),     // 15 left_ankle
        (0.15, -0.05, -0.05),      // 16 right_ankle
    ];
    let walking_conf = [0.8, 0.7, 0.7, 0.6, 0.6, 0.8, 0.8, 0.75, 0.75, 0.7, 0.7, 0.8, 0.8, 0.7, 0.7, 0.6, 0.6];

    // Sitting: lower hips, knees bent
    let sitting = [
        (0.0, 1.35, 0.0),          // 0 nose
        (-0.03, 1.37, -0.02),      // 1 left_eye
        (0.03, 1.37, -0.02),       // 2 right_eye
        (-0.07, 1.35, 0.0),        // 3 left_ear
        (0.07, 1.35, 0.0),         // 4 right_ear
        (-0.18, 1.25, -0.05),      // 5 left_shoulder
        (0.18, 1.25, -0.05),       // 6 right_shoulder
        (-0.25, 1.05, -0.05),      // 7 left_elbow
        (0.25, 1.05, -0.05),       // 8 right_elbow
        (-0.30, 0.85, -0.05),      // 9 left_wrist
        (0.30, 0.85, -0.05),       // 10 right_wrist
        (-0.12, 0.75, -0.05),      // 11 left_hip
        (0.12, 0.75, -0.05),       // 12 right_hip
        (-0.15, 0.45, -0.05),      // 13 left_knee
        (0.15, 0.45, -0.05),       // 14 right_knee
        (-0.12, 0.05, -0.05),      // 15 left_ankle
        (0.12, 0.05, -0.05),       // 16 right_ankle
    ];
    let sitting_conf = [0.8, 0.7, 0.7, 0.6, 0.6, 0.8, 0.8, 0.75, 0.75, 0.7, 0.7, 0.8, 0.8, 0.7, 0.7, 0.6, 0.6];

    let (offsets, confs) = match pose {
        "walking" => (walking, walking_conf),
        "sitting" => (sitting, sitting_conf),
        _ => (standing, standing_conf),
    };

    // Apply walking leg animation
    if pose == "walking" && motion_score > 0.5 {
        let stride = motion_score * 0.08;
        let mut offsets = offsets;
        offsets[13] = (offsets[13].0 - stride, offsets[13].1, offsets[13].2);
        offsets[14] = (offsets[14].0 + stride, offsets[14].1, offsets[14].2);
        offsets[15] = (offsets[15].0 - stride, offsets[15].1, offsets[15].2);
        offsets[16] = (offsets[16].0 + stride, offsets[16].1, offsets[16].2);
        (offsets, confs)
    } else {
        (offsets, confs)
    }
}

pub fn derive_pose_from_sensing(update: &SensingUpdate) -> Vec<PersonDetection> {
    let cls = &update.classification;
    if !cls.presence {
        return vec![];
    }
    let person_count = update.estimated_persons.unwrap_or(1).max(1);
    (0..person_count)
        .map(|idx| derive_single_person_pose(update, idx, person_count))
        .collect()
}

/// Apply temporal EMA smoothing and bone-length clamping to person detections.
pub fn apply_temporal_smoothing(persons: &mut [PersonDetection], ns: &mut NodeState) {
    if persons.is_empty() {
        return;
    }

    let alpha = ns.ema_alpha();
    let person = &mut persons[0];

    let current_kps: Vec<[f64; 3]> = person
        .keypoints
        .iter()
        .map(|kp| [kp.x, kp.y, kp.z])
        .collect();

    let smoothed = if let Some(ref prev) = ns.prev_keypoints {
        let mut out = Vec::with_capacity(current_kps.len());
        for (cur, prv) in current_kps.iter().zip(prev.iter()) {
            out.push([
                alpha * cur[0] + (1.0 - alpha) * prv[0],
                alpha * cur[1] + (1.0 - alpha) * prv[1],
                alpha * cur[2] + (1.0 - alpha) * prv[2],
            ]);
        }
        clamp_bone_lengths_f64(&mut out, prev);
        out
    } else {
        current_kps.clone()
    };

    for (kp, s) in person.keypoints.iter_mut().zip(smoothed.iter()) {
        kp.x = s[0];
        kp.y = s[1];
        kp.z = s[2];
    }
    ns.prev_keypoints = Some(smoothed);
}

fn clamp_bone_lengths_f64(pose: &mut [[f64; 3]], prev: &[[f64; 3]]) {
    for &(p, c) in POSE_BONE_PAIRS {
        if p >= pose.len() || c >= pose.len() {
            continue;
        }
        let prev_len = dist_f64(&prev[p], &prev[c]);
        if prev_len < 1e-6 {
            continue;
        }
        let cur_len = dist_f64(&pose[p], &pose[c]);
        if cur_len < 1e-6 {
            continue;
        }
        let ratio = cur_len / prev_len;
        let lo = 1.0 - MAX_BONE_CHANGE_RATIO;
        let hi = 1.0 + MAX_BONE_CHANGE_RATIO;
        if ratio < lo || ratio > hi {
            let target = prev_len * ratio.clamp(lo, hi);
            let scale = target / cur_len;
            for dim in 0..3 {
                let diff = pose[c][dim] - pose[p][dim];
                pose[c][dim] = pose[p][dim] + diff * scale;
            }
        }
    }
}

fn dist_f64(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let dz = b[2] - a[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}
