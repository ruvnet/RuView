//! Bridge between sensing-server PersonDetection types and signal crate PoseTracker.
//!
//! The sensing server uses f64 types (PersonDetection, PoseKeypoint, BoundingBox)
//! while the signal crate's PoseTracker operates on f32 Kalman states. This module
//! provides conversion functions and a single `tracker_update` entry point that
//! accepts server-side detections and returns tracker-smoothed results.

use std::time::Instant;
use wifi_densepose_signal::ruvsense::pose_tracker::PoseTracker;
use wifi_densepose_signal::ruvsense::{TrackId, TrackLifecycleState, NUM_KEYPOINTS};

use super::{BoundingBox, PersonDetection, PoseKeypoint};

/// Pixels per metre for the pixel-space <-> world-space conversion at the
/// tracker boundary.
///
/// `PersonDetection` keypoints are in image pixels (~0..800), but `PoseTracker`
/// is tuned for world-space metres: `measurement_noise = 0.08 m`, the
/// Mahalanobis gate, and the Kalman covariances all assume metre-scale inputs.
/// Feeding raw pixels made the effective association gate ~1.5 px, so the
/// natural per-frame jitter of a single person (±20 px) never matched its own
/// track — every frame spawned a fresh track and the stale ones lingered as
/// confirmed "ghost" skeletons stacked on top of each other (the reported
/// "Persons: 4" for one real person, all piled at the centre).
///
/// Converting on the way in (px -> m) and back out (m -> px) runs the filter in
/// the metre space it was designed for: a ±20 px jitter becomes 0.2 m (well
/// inside the gate) while two genuinely distinct people 180 px apart become
/// 1.8 m (well outside it). 100 px/m ⇒ an 800 px frame maps to an 8 m room.
const PIXELS_PER_METER: f32 = 100.0;

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
    COCO_NAMES
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Convert server-side PersonDetection slices into tracker-compatible keypoint arrays.
///
/// For each person, maps named keypoints to COCO-17 positions. Unmapped slots are
/// filled with the centroid of the mapped keypoints so the Kalman filter has a
/// reasonable initial value rather than zeros.
///
/// Returns positions **and** per-keypoint confidences. Unmapped (centroid-filled)
/// slots get confidence `0.0`, which is what marks them as unobserved for bbox
/// fitting and for the UI renderer — mapped slots carry the detector's real
/// confidence so the skeleton survives the round-trip through the tracker.
type TrackerKeypoints = (Vec<[[f32; 3]; 17]>, Vec<[f32; 17]>);

fn detections_to_tracker_keypoints(persons: &[PersonDetection]) -> TrackerKeypoints {
    persons
        .iter()
        .map(|person| {
            let mut kps = [[0.0_f32; 3]; 17];
            let mut confs = [0.0_f32; 17];
            let mut mapped_count = 0u32;
            let mut cx = 0.0_f32;
            let mut cy = 0.0_f32;
            let mut cz = 0.0_f32;

            // First pass: place mapped keypoints and accumulate centroid.
            // Pixels -> metres so the metre-tuned Kalman filter/gate behave.
            let s = 1.0 / PIXELS_PER_METER;
            for kp in &person.keypoints {
                if let Some(idx) = keypoint_name_to_coco_index(&kp.name) {
                    let (x, y, z) = (kp.x as f32 * s, kp.y as f32 * s, kp.z as f32 * s);
                    kps[idx] = [x, y, z];
                    confs[idx] = kp.confidence as f32;
                    cx += x;
                    cy += y;
                    cz += z;
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
                    // Centroid-filled slot: never observed, so no confidence.
                    confs[i] = 0.0;
                }
            }

            (kps, confs)
        })
        .unzip()
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

            // Build keypoints from Kalman state. Metres -> pixels to undo the
            // px->m scaling applied on the way into the tracker.
            let inv = PIXELS_PER_METER as f64;
            let keypoints: Vec<PoseKeypoint> = (0..NUM_KEYPOINTS)
                .map(|i| {
                    let pos = track.keypoints[i].position();
                    PoseKeypoint {
                        name: COCO_NAMES[i].to_string(),
                        x: pos[0] as f64 * inv,
                        y: pos[1] as f64 * inv,
                        z: pos[2] as f64 * inv,
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
                    if kp.x < min_x {
                        min_x = kp.x;
                    }
                    if kp.y < min_y {
                        min_y = kp.y;
                    }
                    if kp.x > max_x {
                        max_x = kp.x;
                    }
                    if kp.y > max_y {
                        max_y = kp.y;
                    }
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
                // No observed keypoints — use a default bbox at centroid.
                // Pixel-space, human-sized (~100x200 px), matching the keypoint
                // coordinate space so a fallback never collapses to a point.
                let cx = keypoints.iter().map(|k| k.x).sum::<f64>() / keypoints.len() as f64;
                let cy = keypoints.iter().map(|k| k.y).sum::<f64>() / keypoints.len() as f64;
                BoundingBox {
                    x: cx - 50.0,
                    y: cy - 100.0,
                    width: 100.0,
                    height: 200.0,
                }
            };

            PersonDetection {
                id,
                confidence,
                keypoints,
                bbox,
                zone: "tracked".to_string(),
                // Field-derived position/motion_score/pose are (re)attached from
                // the live signal_field by `attach_field_positions` after this
                // tracker step (#1050); the Kalman tracker smooths keypoints only,
                // so we default here and let the field readout fill them in.
                position: [0.0, 0.0, 0.0],
                motion_score: 0.0,
                pose: None,
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

    // Convert detections to f32 keypoint arrays (positions + per-keypoint confidence)
    let (all_keypoints, all_confidences) = detections_to_tracker_keypoints(&persons);

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
    let active: Vec<(TrackId, [f32; 3])> = tracker
        .active_tracks()
        .iter()
        .map(|t| {
            let centroid = {
                let mut c = [0.0_f32; 3];
                for kp in &t.keypoints {
                    let p = kp.position();
                    c[0] += p[0];
                    c[1] += p[1];
                    c[2] += p[2];
                }
                let n = NUM_KEYPOINTS as f32;
                [c[0] / n, c[1] / n, c[2] / n]
            };
            (t.id, centroid)
        })
        .collect();

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
                track.update_keypoints(
                    &all_keypoints[det_idx],
                    &all_confidences[det_idx],
                    0.08,
                    1.0,
                    timestamp_us,
                );
            }
        }
    }

    // Create new tracks for unmatched detections
    for (det_idx, track_id_opt) in matched.iter().enumerate() {
        if track_id_opt.is_none() {
            tracker.create_track(
                &all_keypoints[det_idx],
                &all_confidences[det_idx],
                timestamp_us,
            );
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
            position: [0.0, 0.0, 0.0],
            motion_score: 0.0,
            pose: None,
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

        let (positions, confidences) = detections_to_tracker_keypoints(&[person]);
        assert_eq!(positions.len(), 1);
        assert_eq!(confidences.len(), 1);

        let kps = &positions[0];
        let confs = &confidences[0];

        // Positions are scaled px -> m by 1/PIXELS_PER_METER on the way in.
        let s = 1.0 / PIXELS_PER_METER;

        // Mapped keypoints should have correct (scaled) values
        assert!((kps[0][0] - 1.0 * s).abs() < 1e-6); // nose x
        assert!((kps[0][1] - 2.0 * s).abs() < 1e-6); // nose y
        assert!((kps[0][2] - 0.5 * s).abs() < 1e-6); // nose z

        assert!((kps[5][0] - 0.8 * s).abs() < 1e-6); // left_shoulder x
        assert!((kps[6][0] - 1.2 * s).abs() < 1e-6); // right_shoulder x

        // Mapped keypoints carry the detector's confidence (make_keypoint sets
        // it to 0.9); unmapped/centroid-filled slots must be exactly 0.0.
        assert!(confs[0] > 0.0, "mapped nose must be observed");
        assert!(confs[5] > 0.0, "mapped left_shoulder must be observed");
        assert!(confs[6] > 0.0, "mapped right_shoulder must be observed");
        assert_eq!(confs[1], 0.0, "unmapped left_eye must be unobserved");

        // Unmapped keypoints should be at centroid of mapped keypoints (scaled)
        // centroid = ((1.0+0.8+1.2)/3, (2.0+2.5+2.5)/3, (0.5+0.4+0.6)/3) * s
        let cx = (1.0 + 0.8 + 1.2) / 3.0 * s;
        let cy = (2.0 + 2.5 + 2.5) / 3.0 * s;
        let cz = (0.5 + 0.4 + 0.6) / 3.0 * s;

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

    /// Build a full COCO-17 person centered at pixel `cx`, roughly the shape
    /// `derive_single_person_pose` emits (torso ~100px wide, ~210px tall).
    fn make_person_at(id: u32, cx: f64) -> PersonDetection {
        let offsets: [(f64, f64); 17] = [
            (0.0, -80.0),
            (-8.0, -88.0),
            (8.0, -88.0),
            (-16.0, -82.0),
            (16.0, -82.0),
            (-30.0, -50.0),
            (30.0, -50.0),
            (-45.0, -15.0),
            (45.0, -15.0),
            (-50.0, 20.0),
            (50.0, 20.0),
            (-20.0, 20.0),
            (20.0, 20.0),
            (-22.0, 70.0),
            (22.0, 70.0),
            (-24.0, 120.0),
            (24.0, 120.0),
        ];
        let kps = COCO_NAMES
            .iter()
            .zip(offsets.iter())
            .map(|(name, (dx, dy))| PoseKeypoint {
                name: name.to_string(),
                x: cx + dx,
                y: 240.0 + dy,
                z: 0.0,
                confidence: 0.5,
            })
            .collect();
        make_person(id, kps)
    }

    /// Regression: three people generated far apart in pixel-space must stay
    /// spatially separated after passing through the tracker across many
    /// frames. Before the tracker was fed pixel-space centroids through a
    /// gate/noise model tuned for metres, tracks churned and collapsed toward
    /// the centre, so the UI drew several skeletons stacked on top of each
    /// other. See the "tudo em cima de tudo" report.
    #[test]
    fn test_multi_person_stay_separated() {
        let mut tracker = PoseTracker::new();
        let mut last_instant: Option<Instant> = None;

        let centers = [140.0_f64, 320.0, 500.0];
        let mut out = vec![];
        // Run enough frames for tracks to confirm (Active) and settle.
        for _ in 0..15 {
            let persons: Vec<PersonDetection> = centers
                .iter()
                .enumerate()
                .map(|(i, &cx)| make_person_at(i as u32, cx))
                .collect();
            out = tracker_update(&mut tracker, &mut last_instant, persons);
        }

        // Each input person must be represented by a distinct tracked skeleton
        // near its true centre, not merged into a central blob.
        let centroid_x = |p: &PersonDetection| -> f64 {
            p.keypoints.iter().map(|k| k.x).sum::<f64>() / p.keypoints.len() as f64
        };
        let mut xs: Vec<f64> = out.iter().map(centroid_x).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        assert_eq!(
            out.len(),
            3,
            "expected 3 tracked persons, got {} (churn/merge)",
            out.len()
        );
        // Adjacent centroids should stay ~180px apart; allow drift but reject
        // collapse (they must not all pile within a narrow band).
        for w in xs.windows(2) {
            assert!(
                w[1] - w[0] > 90.0,
                "tracked persons collapsed together: centroids {xs:?}"
            );
        }
    }

    /// Regression: a single jittering person must stay ONE track, not spawn a
    /// pile of ghosts. The simulator emits ~1 person whose pixel centroid
    /// jitters ±20 px frame-to-frame; with the tracker fed raw pixels the
    /// association gate was ~1.5 px, so every frame minted a fresh track and
    /// the UI showed "Persons: 4+" all stacked at the centre. Scaling px->m at
    /// the boundary keeps the jitter (0.2 m) well inside the gate.
    #[test]
    fn test_single_jittering_person_stays_one_track() {
        let mut tracker = PoseTracker::new();
        let mut last_instant: Option<Instant> = None;

        // Deterministic pseudo-jitter around x = 300 px, amplitude ~±20 px.
        let mut out = vec![];
        for f in 0..30 {
            let jitter = ((f as f64 * 1.9).sin() * 20.0).round();
            let person = make_person_at(0, 300.0 + jitter);
            out = tracker_update(&mut tracker, &mut last_instant, vec![person]);
        }

        assert_eq!(
            out.len(),
            1,
            "one jittering person must yield exactly one tracked skeleton, got {} ghosts",
            out.len()
        );
    }

    /// Regression test for #420 (ADR-082): tracks that have transitioned to
    /// `Lost` must NOT appear in `tracker_update`'s returned PersonDetection
    /// vector, even though they remain in the tracker for re-identification.
    #[test]
    fn test_lost_tracks_excluded_from_bridge_output() {
        use wifi_densepose_signal::ruvsense::{TrackLifecycleState, TrackerConfig};

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
            tracker
                .all_tracks()
                .iter()
                .any(|t| t.lifecycle == TrackLifecycleState::Lost),
            "Lost track must remain in tracker for re-identification window"
        );
    }
}
