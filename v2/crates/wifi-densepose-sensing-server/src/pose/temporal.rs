use crate::domain::types::PersonDetection;

/// Maximum allowed bone-length change ratio between frames (20%).
pub const MAX_BONE_CHANGE_RATIO: f64 = 0.20;

/// Expected bone lengths in pixel-space for the COCO-17 skeleton.
/// Pairs are (parent_idx, child_idx).
pub const POSE_BONE_PAIRS: &[(usize, usize)] = &[
    (5, 7),
    (7, 9),
    (6, 8),
    (8, 10), // arms
    (5, 11),
    (6, 12), // torso
    (11, 13),
    (13, 15),
    (12, 14),
    (14, 16), // legs
    (5, 6),
    (11, 12), // shoulders, hips
];

/// Clamp bone lengths so no bone changes by more than MAX_BONE_CHANGE_RATIO.
pub fn clamp_bone_lengths_f64(pose: &mut [[f64; 3]], prev: &[[f64; 3]]) {
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

/// Euclidean distance between two 3D points.
pub fn dist_f64(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let dz = b[2] - a[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Apply temporal EMA smoothing and bone-length clamping to primary person detection.
pub fn apply_temporal_smoothing_to_detection(
    person: &mut PersonDetection,
    prev_keypoints: &mut Option<Vec<[f64; 3]>>,
    alpha: f64,
) {
    let current_kps: Vec<[f64; 3]> = person
        .keypoints
        .iter()
        .map(|kp| [kp.x, kp.y, kp.z])
        .collect();

    let smoothed = if let Some(ref prev) = prev_keypoints {
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

    *prev_keypoints = Some(smoothed);
}

