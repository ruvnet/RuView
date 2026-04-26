//! Bridge between sensing-server PersonDetection types and signal crate PoseTracker.
//!
//! The sensing server uses f64 types (PersonDetection, PoseKeypoint, BoundingBox)
//! while the signal crate's PoseTracker operates on f32 Kalman states. This module
//! provides conversion functions and a single `tracker_update` entry point that
//! accepts server-side detections and returns tracker-smoothed results.

use std::time::Instant;
use wifi_densepose_signal::ruvsense::{
    self, KeypointState, PoseTrack, TrackLifecycleState, TrackId, NUM_KEYPOINTS,
};
use wifi_densepose_signal::ruvsense::pose_tracker::PoseTracker;

use super::{BoundingBox, PersonDetection, PoseKeypoint};

/// COCO-17 keypoint names in index order.
const COCO_NAMES: [&str; 17] = [
    "nose",
    "left_eye",
    "right_eye",
    "left_ear",
    "right_ear",
    "left_shoulder",
    "right_shoulder",
    "left_elbow",
    "right_elbow",
    "left_wrist",
    "right_wrist",
    "left_hip",
    "right_hip",
    "left_knee",
    "right_knee",
    "left_ankle",
    "right_ankle",
];

/// Map a lowercase keypoint name to its COCO-17 index.
fn keypoint_name_to_coco_index(name: &str) -> Option<usize> {
    COCO_NAMES.iter().position(|&n| n.eq_ignore_ascii_case(name))
}

/// Convert server-side PersonDetection slices into tracker-compatible keypoint arrays.
///
/// For each person, maps named keypoints to COCO-17 positions. Unmapped slots are
/// filled with the centroid of the mapped keypoints so the Kalman filter has a
/// reasonable initial value rather than zeros.
fn detections_to_tracker_keypoints(persons: &[PersonDetection]) -> Vec<[[f32; 3]; 17]> {
    persons
        .iter()
        .map(|person| {
            let mut kps = [[0.0_f32; 3]; 17];
            let mut mapped_count = 0u32;
            let mut cx = 0.0_f32;
            let mut cy = 0.0_f32;
            let mut cz = 0.0_f32;

            // First pass: place mapped keypoints and accumulate centroid
            for kp in &person.keypoints {
                if let Some(idx) = keypoint_name_to_coco_index(&kp.name) {
                    kps[idx] = [kp.x as f32, kp.y as f32, kp.z as f32];
                    cx += kp.x as f32;
                    cy += kp.y as f32;
                    cz += kp.z as f32;
                    mapped_count += 1;
                }
            }

            // Compute centroid of mapped keypoints
            let centroid = if mapped_count > 0 {
                let n = mapped_count as f32;
                [cx / n, cy / n, cz / n]
            } else {
                [0.0, 0.0, 0.0]
            };

            // Second pass: fill unmapped slots with centroid
            // Build a set of mapped indices
            let mut mapped = [false; 17];
            for kp in &person.keypoints {
                if let Some(idx) = keypoint_name_to_coco_index(&kp.name) {
                    mapped[idx] = true;
                }
            }
            for i in 0..17 {
                if !mapped[i] {
                    kps[i] = centroid;
                }
            }

            kps
        })
        .collect()
}

/// Convert confirmed PoseTracker tracks back into server-side PersonDetection values.
///
/// Returns only tracks the UI is meant to render right now (Tentative + Active).
/// `Lost` tracks — kept around inside `reid_window` for re-identification but
/// not currently observed — are excluded so they don't ship to the WebSocket
/// stream as ghost skeletons. See ADR-082 and #420.
pub fn tracker_to_person_detections(tracker: &PoseTracker) -> Vec<PersonDetection> {
    tracker
        .confirmed_tracks()
        .into_iter()
        .map(|track| {
            let id = track.id.0 as u32;

            let confidence = match track.lifecycle {
                TrackLifecycleState::Active => 0.9,
                TrackLifecycleState::Tentative => 0.5,
                TrackLifecycleState::Lost => 0.3,
                TrackLifecycleState::Terminated => 0.0,
            };

            // Build keypoints from Kalman state
            let keypoints: Vec<PoseKeypoint> = (0..NUM_KEYPOINTS)
                .map(|i| {
                    let pos = track.keypoints[i].position();
                    PoseKeypoint {
                        name: COCO_NAMES[i].to_string(),
                        x: pos[0] as f64,
                        y: pos[1] as f64,
                        z: pos[2] as f64,
                        confidence: track.keypoints[i].confidence as f64,
                    }
                })
                .collect();

            // Compute bounding box from observed keypoints only (confidence > 0).
            // Unobserved slots (centroid-filled) collapse the bbox over time.
            let mut min_x = f64::MAX;
            let mut min_y = f64::MAX;
            let mut max_x = f64::MIN;
            let mut max_y = f64::MIN;
            let mut observed = 0;
            for kp in &keypoints {
                if kp.confidence > 0.0 {
                    if kp.x < min_x { min_x = kp.x; }
                    if kp.y < min_y { min_y = kp.y; }
                    if kp.x > max_x { max_x = kp.x; }
                    if kp.y > max_y { max_y = kp.y; }
                    observed += 1;
                }
            }

            let bbox = if observed > 0 {
                BoundingBox {
                    x: min_x,
                    y: min_y,
                    width: (max_x - min_x).max(0.01),
                    height: (max_y - min_y).max(0.01),
                }
            } else {
                // No observed keypoints — use a default bbox at centroid
                let cx = keypoints.iter().map(|k| k.x).sum::<f64>() / keypoints.len() as f64;
                let cy = keypoints.iter().map(|k| k.y).sum::<f64>() / keypoints.len() as f64;
                BoundingBox { x: cx - 0.3, y: cy - 0.5, width: 0.6, height: 1.0 }
            };

            PersonDetection {
                id,
                confidence,
                keypoints,
                bbox,
                zone: "tracked".to_string(),
            }
        })
        .collect()
}

/// Run one tracker cycle: predict, match detections, update, prune.
///
/// This is the main entry point called each sensing frame. It:
/// 1. Computes dt from the previous call instant
/// 2. Predicts all existing tracks forward
/// 3. Greedily assigns detections to tracks by Mahalanobis cost
/// 4. Updates matched tracks, creates new tracks for unmatched detections
/// 5. Prunes terminated tracks
/// 6. Returns smoothed PersonDetection values from the tracker state
pub fn tracker_update(
    tracker: &mut PoseTracker,
    last_instant: &mut Option<Instant>,
    persons: Vec<PersonDetection>,
) -> Vec<PersonDetection> {
    let now = Instant::now();
    let dt = last_instant.map_or(0.1_f32, |prev| now.duration_since(prev).as_secs_f32());
    *last_instant = Some(now);

    // Predict all tracks forward
    tracker.predict_all(dt);

    if persons.is_empty() {
        tracker.prune_terminated();
        return tracker_to_person_detections(tracker);
    }

    // Convert detections to f32 keypoint arrays
    let all_keypoints = detections_to_tracker_keypoints(&persons);

    // Compute centroids for each detection
    let centroids: Vec<[f32; 3]> = all_keypoints
        .iter()
        .map(|kps| {
            let mut c = [0.0_f32; 3];
            for kp in kps {
                c[0] += kp[0];
                c[1] += kp[1];
                c[2] += kp[2];
            }
            let n = NUM_KEYPOINTS as f32;
            c[0] /= n;
            c[1] /= n;
            c[2] /= n;
            c
        })
        .collect();

    // Greedy assignment: for each detection, find the best matching active track.
    // Collect tracks once to avoid re-borrowing tracker per detection.
    let active: Vec<(TrackId, [f32; 3])> = tracker.active_tracks().iter().map(|t| {
        let centroid = {
            let mut c = [0.0_f32; 3];
            for kp in &t.keypoints {
                let p = kp.position();
                c[0] += p[0]; c[1] += p[1]; c[2] += p[2];
            }
            let n = NUM_KEYPOINTS as f32;
            [c[0] / n, c[1] / n, c[2] / n]
        };
        (t.id, centroid)
    }).collect();

    let mut used_tracks: Vec<bool> = vec![false; active.len()];
    let mut matched: Vec<Option<TrackId>> = vec![None; persons.len()];

    for det_idx in 0..persons.len() {
        let mut best_cost = f32::MAX;
        let mut best_track_idx = None;

        let active_refs = tracker.active_tracks();
        for (track_idx, track) in active_refs.iter().enumerate() {
            if used_tracks[track_idx] {
                continue;
            }
            let cost = tracker.assignment_cost(track, &centroids[det_idx], &[]);
            if cost < best_cost {
                best_cost = cost;
                best_track_idx = Some(track_idx);
            }
        }

        // Mahalanobis gate: 9.0 (default TrackerConfig)
        if best_cost < 9.0 {
            if let Some(tidx) = best_track_idx {
                matched[det_idx] = Some(active[tidx].0);
                used_tracks[tidx] = true;
            }
        }
    }

    // Timestamp for new/updated tracks (microseconds since UNIX epoch)
    let timestamp_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);

    // Update matched tracks (uses update_keypoints for proper lifecycle transitions)
    for (det_idx, track_id_opt) in matched.iter().enumerate() {
        if let Some(track_id) = track_id_opt {
            if let Some(track) = tracker.find_track_mut(*track_id) {
                track.update_keypoints(&all_keypoints[det_idx], 0.08, 1.0, timestamp_us);
            }
        }
    }

    // Create new tracks for unmatched detections
    for (det_idx, track_id_opt) in matched.iter().enumerate() {
        if track_id_opt.is_none() {
            tracker.create_track(&all_keypoints[det_idx], timestamp_us);
        }
    }

    tracker.prune_terminated();
    tracker_to_person_detections(tracker)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_keypoint(name: &str, x: f64, y: f64, z: f64) -> PoseKeypoint {
        PoseKeypoint {
            name: name.to_string(),
            x,
            y,
            z,
            confidence: 0.9,
        }
    }

    fn make_person(id: u32, keypoints: Vec<PoseKeypoint>) -> PersonDetection {
        PersonDetection {
            id,
            confidence: 0.8,
            keypoints,
            bbox: BoundingBox {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            zone: "test".to_string(),
        }
    }

    #[test]
    fn test_keypoint_name_to_coco_index() {
        assert_eq!(keypoint_name_to_coco_index("nose"), Some(0));
        assert_eq!(keypoint_name_to_coco_index("left_eye"), Some(1));
        assert_eq!(keypoint_name_to_coco_index("right_eye"), Some(2));
        assert_eq!(keypoint_name_to_coco_index("left_ear"), Some(3));
        assert_eq!(keypoint_name_to_coco_index("right_ear"), Some(4));
        assert_eq!(keypoint_name_to_coco_index("left_shoulder"), Some(5));
        assert_eq!(keypoint_name_to_coco_index("right_shoulder"), Some(6));
        assert_eq!(keypoint_name_to_coco_index("left_elbow"), Some(7));
        assert_eq!(keypoint_name_to_coco_index("right_elbow"), Some(8));
        assert_eq!(keypoint_name_to_coco_index("left_wrist"), Some(9));
        assert_eq!(keypoint_name_to_coco_index("right_wrist"), Some(10));
        assert_eq!(keypoint_name_to_coco_index("left_hip"), Some(11));
        assert_eq!(keypoint_name_to_coco_index("right_hip"), Some(12));
        assert_eq!(keypoint_name_to_coco_index("left_knee"), Some(13));
        assert_eq!(keypoint_name_to_coco_index("right_knee"), Some(14));
        assert_eq!(keypoint_name_to_coco_index("left_ankle"), Some(15));
        assert_eq!(keypoint_name_to_coco_index("right_ankle"), Some(16));
        assert_eq!(keypoint_name_to_coco_index("unknown"), None);
        // Case insensitive
        assert_eq!(keypoint_name_to_coco_index("NOSE"), Some(0));
        assert_eq!(keypoint_name_to_coco_index("Left_Eye"), Some(1));
    }

    #[test]
    fn test_detections_to_tracker_keypoints() {
        let person = make_person(
            1,
            vec![
                make_keypoint("nose", 1.0, 2.0, 0.5),
                make_keypoint("left_shoulder", 0.8, 2.5, 0.4),
                make_keypoint("right_shoulder", 1.2, 2.5, 0.6),
            ],
        );

        let result = detections_to_tracker_keypoints(&[person]);
        assert_eq!(result.len(), 1);

        let kps = &result[0];

        // Mapped keypoints should have correct values
        assert!((kps[0][0] - 1.0).abs() < 1e-5); // nose x
        assert!((kps[0][1] - 2.0).abs() < 1e-5); // nose y
        assert!((kps[0][2] - 0.5).abs() < 1e-5); // nose z

        assert!((kps[5][0] - 0.8).abs() < 1e-5); // left_shoulder x
        assert!((kps[6][0] - 1.2).abs() < 1e-5); // right_shoulder x

        // Unmapped keypoints should be at centroid of mapped keypoints
        // centroid = ((1.0+0.8+1.2)/3, (2.0+2.5+2.5)/3, (0.5+0.4+0.6)/3)
        let cx = (1.0 + 0.8 + 1.2) / 3.0;
        let cy = (2.0 + 2.5 + 2.5) / 3.0;
        let cz = (0.5 + 0.4 + 0.6) / 3.0;

        // left_eye (index 1) should be at centroid
        assert!((kps[1][0] - cx).abs() < 1e-4);
        assert!((kps[1][1] - cy).abs() < 1e-4);
        assert!((kps[1][2] - cz).abs() < 1e-4);
    }

    #[test]
    fn test_tracker_update_stable_ids() {
        let mut tracker = PoseTracker::new();
        let mut last_instant: Option<Instant> = None;

        let person = make_person(
            0,
            vec![
                make_keypoint("nose", 1.0, 2.0, 0.0),
                make_keypoint("left_shoulder", 0.8, 2.5, 0.0),
                make_keypoint("right_shoulder", 1.2, 2.5, 0.0),
                make_keypoint("left_hip", 0.9, 3.5, 0.0),
                make_keypoint("right_hip", 1.1, 3.5, 0.0),
            ],
        );

        // First update: creates a new track
        let result1 = tracker_update(&mut tracker, &mut last_instant, vec![person.clone()]);
        assert_eq!(result1.len(), 1);
        let id1 = result1[0].id;

        // Second update: should match the existing track
        let result2 = tracker_update(&mut tracker, &mut last_instant, vec![person.clone()]);
        assert_eq!(result2.len(), 1);
        let id2 = result2[0].id;

        // Third update: same track ID should persist
        let result3 = tracker_update(&mut tracker, &mut last_instant, vec![person.clone()]);
        assert_eq!(result3.len(), 1);
        let id3 = result3[0].id;

        // All three updates should return the same track ID
        assert_eq!(id1, id2, "Track ID should be stable across updates");
        assert_eq!(id2, id3, "Track ID should be stable across updates");
    }

    /// Regression test for #420 (ADR-082): tracks that have transitioned to
    /// `Lost` must NOT appear in `tracker_update`'s returned PersonDetection
    /// vector, even though they remain in the tracker for re-identification.
    #[test]
    fn test_lost_tracks_excluded_from_bridge_output() {
        use wifi_densepose_signal::ruvsense::{TrackerConfig, TrackLifecycleState};

        // Tight config so the test doesn't have to spin for hundreds of ticks.
        let cfg = TrackerConfig {
            loss_misses: 3,
            reid_window: 100, // intentionally large — we want Lost, not Terminated
            ..TrackerConfig::default()
        };
        let mut tracker = PoseTracker::with_config(cfg);
        let mut last_instant: Option<Instant> = None;

        let person = make_person(
            0,
            vec![
                make_keypoint("nose", 1.0, 2.0, 0.0),
                make_keypoint("left_shoulder", 0.8, 2.5, 0.0),
                make_keypoint("right_shoulder", 1.2, 2.5, 0.0),
                make_keypoint("left_hip", 0.9, 3.5, 0.0),
                make_keypoint("right_hip", 1.1, 3.5, 0.0),
            ],
        );

        // Drive the track to Active (≥2 consecutive hits).
        let r1 = tracker_update(&mut tracker, &mut last_instant, vec![person.clone()]);
        let r2 = tracker_update(&mut tracker, &mut last_instant, vec![person.clone()]);
        assert_eq!(r1.len(), 1);
        assert_eq!(r2.len(), 1);

        // Submit empty detections enough times to push the track into Lost.
        // Each empty call increments time_since_update via predict_all().
        for _ in 0..6 {
            let _ = tracker_update(&mut tracker, &mut last_instant, vec![]);
        }

        // Pre-condition: a track exists internally and is in Lost state.
        let has_lost = tracker
            .all_tracks()
            .iter()
            .any(|t| t.lifecycle == TrackLifecycleState::Lost);
        assert!(
            has_lost,
            "Test setup invariant violated: expected the track to be Lost \
             after {} empty updates with loss_misses=3",
            6
        );

        // The fix: `tracker_update` must NOT return any phantom detections
        // for the Lost track when there are no current detections.
        let after_lost = tracker_update(&mut tracker, &mut last_instant, vec![]);
        assert_eq!(
            after_lost.len(),
            0,
            "Lost tracks must not appear in bridge output (ADR-082, #420). \
             Got {} phantom detection(s).",
            after_lost.len()
        );

        // Sanity: the Lost track is still tracked internally (for re-ID), it
        // just shouldn't ship to the UI.
        assert!(
            tracker.all_tracks().iter().any(|t| t.lifecycle == TrackLifecycleState::Lost),
            "Lost track must remain in tracker for re-identification window"
        );
    }
}
