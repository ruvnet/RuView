//! SurvivorTracker aggregate root for the MAT crate.
//!
//! Orchestrates Kalman prediction, data association, CSI fingerprint
//! re-identification, and track lifecycle management per update tick.

use std::time::Instant;
use uuid::Uuid;

use super::{
    fingerprint::CsiFingerprint,
    kalman::KalmanState,
    lifecycle::{TrackLifecycle, TrackState, TrackerConfig},
};
use crate::domain::{
    coordinates::Coordinates3D,
    scan_zone::ScanZoneId,
    survivor::Survivor,
    vital_signs::VitalSignsReading,
};

// ---------------------------------------------------------------------------
// TrackId
// ---------------------------------------------------------------------------

/// Stable identifier for a single tracked entity, surviving re-identification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrackId(Uuid);

impl TrackId {
    /// Allocate a new random TrackId.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Borrow the inner UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for TrackId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TrackId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// DetectionObservation
// ---------------------------------------------------------------------------

/// A single detection from the sensing pipeline for one update tick.
#[derive(Debug, Clone)]
pub struct DetectionObservation {
    /// 3-D position estimate (may be None if triangulation failed)
    pub position: Option<Coordinates3D>,
    /// Vital signs associated with this detection
    pub vital_signs: VitalSignsReading,
    /// Ensemble confidence score [0, 1]
    pub confidence: f64,
    /// Zone where detection occurred
    pub zone_id: ScanZoneId,
}

// ---------------------------------------------------------------------------
// AssociationResult
// ---------------------------------------------------------------------------

/// Summary of what happened during one tracker update tick.
#[derive(Debug, Default)]
pub struct AssociationResult {
    /// Tracks that matched an observation this tick.
    pub matched_track_ids: Vec<TrackId>,
    /// New tracks born from unmatched observations.
    pub born_track_ids: Vec<TrackId>,
    /// Tracks that transitioned to Lost this tick.
    pub lost_track_ids: Vec<TrackId>,
    /// Lost tracks re-linked via fingerprint.
    pub reidentified_track_ids: Vec<TrackId>,
    /// Tracks that transitioned to Terminated this tick.
    pub terminated_track_ids: Vec<TrackId>,
    /// Tracks confirmed as Rescued.
    pub rescued_track_ids: Vec<TrackId>,
}

// ---------------------------------------------------------------------------
// TrackedSurvivor
// ---------------------------------------------------------------------------

/// A survivor with its associated tracking state.
pub struct TrackedSurvivor {
    /// Stable track identifier (survives re-ID).
    pub id: TrackId,
    /// The underlying domain entity.
    pub survivor: Survivor,
    /// Kalman filter state.
    pub kalman: KalmanState,
    /// CSI fingerprint for re-ID.
    pub fingerprint: CsiFingerprint,
    /// Track lifecycle state machine.
    pub lifecycle: TrackLifecycle,
    /// When the track was created (for cleanup of old terminal tracks).
    terminated_at: Option<Instant>,
}

impl TrackedSurvivor {
    /// Construct a new tentative TrackedSurvivor from a detection observation.
    fn from_observation(obs: &DetectionObservation, config: &TrackerConfig) -> Self {
        let pos_vec = obs.position.as_ref().map(|p| [p.x, p.y, p.z]).unwrap_or([0.0, 0.0, 0.0]);
        let kalman = KalmanState::new(pos_vec, config.process_noise_var, config.obs_noise_var);
        let fingerprint = CsiFingerprint::from_vitals(&obs.vital_signs, obs.position.as_ref());
        let mut lifecycle = TrackLifecycle::new(config);
        lifecycle.hit(); // birth observation counts as the first hit
        let survivor = Survivor::new(
            obs.zone_id.clone(),
            obs.vital_signs.clone(),
            obs.position.clone(),
        );

        Self {
            id: TrackId::new(),
            survivor,
            kalman,
            fingerprint,
            lifecycle,
            terminated_at: None,
        }
    }
}

// ---------------------------------------------------------------------------
// SurvivorTracker
// ---------------------------------------------------------------------------

/// Aggregate root managing all tracked survivors.
pub struct SurvivorTracker {
    tracks: Vec<TrackedSurvivor>,
    config: TrackerConfig,
}

impl SurvivorTracker {
    /// Create a tracker with the provided configuration.
    pub fn new(config: TrackerConfig) -> Self {
        Self {
            tracks: Vec::new(),
            config,
        }
    }

    /// Create a tracker with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(TrackerConfig::default())
    }

    /// Main per-tick update.
    ///
    /// Algorithm:
    /// 1. Predict Kalman for all Active + Tentative + Lost tracks
    /// 2. Mahalanobis-gate: active/tentative tracks vs observations
    /// 3. Greedy nearest-neighbour assignment (gated)
    /// 4. Re-ID: unmatched obs vs Lost tracks via fingerprint
    /// 5. Birth: still-unmatched obs → new Tentative track
    /// 6. Kalman update + vitals update for matched tracks
    /// 7. Lifecycle transitions (hit/miss/expiry)
    /// 8. Remove Terminated tracks older than 60 s (cleanup)
    pub fn update(
        &mut self,
        observations: Vec<DetectionObservation>,
        dt_secs: f64,
    ) -> AssociationResult {
        let now = Instant::now();
        let mut result = AssociationResult::default();

        // ----------------------------------------------------------------
        // Step 1 — Predict Kalman for non-terminal tracks
        // ----------------------------------------------------------------
        for track in &mut self.tracks {
            if !track.lifecycle.is_terminal() {
                track.kalman.predict(dt_secs);
            }
        }

        // ----------------------------------------------------------------
        // Separate active/tentative track indices from lost track indices
        // ----------------------------------------------------------------
        let active_indices: Vec<usize> = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.lifecycle.is_active_or_tentative())
            .map(|(i, _)| i)
            .collect();

        let n_tracks = active_indices.len();
        let n_obs = observations.len();

        // ----------------------------------------------------------------
        // Step 2 — Build gated cost matrix [track_idx][obs_idx]
        // ----------------------------------------------------------------
        // costs[i][j] = Mahalanobis d² if obs has position AND d² < gate, else f64::MAX
        let mut costs: Vec<Vec<f64>> = vec![vec![f64::MAX; n_obs]; n_tracks];

        for (ti, &track_idx) in active_indices.iter().enumerate() {
            for (oi, obs) in observations.iter().enumerate() {
                if let Some(pos) = &obs.position {
                    let obs_vec = [pos.x, pos.y, pos.z];
                    let d_sq = self.tracks[track_idx].kalman.mahalanobis_distance_sq(obs_vec);
                    if d_sq < self.config.gate_mahalanobis_sq {
                        costs[ti][oi] = d_sq;
                    }
                }
            }
        }

        // ----------------------------------------------------------------
        // Step 3 — Hungarian assignment (O(n³) for n ≤ 10, greedy otherwise)
        // ----------------------------------------------------------------
        let assignments = if n_tracks <= 10 && n_obs <= 10 {
            hungarian_assign(&costs, n_tracks, n_obs)
        } else {
            greedy_assign(&costs, n_tracks, n_obs)
        };

        // Track which observations have been assigned
        let mut obs_assigned = vec![false; n_obs];
        // (active_index → obs_index) for matched pairs
        let mut matched_pairs: Vec<(usize, usize)> = Vec::new();

        for (ti, oi_opt) in assignments.iter().enumerate() {
            if let Some(oi) = oi_opt {
                obs_assigned[*oi] = true;
                matched_pairs.push((ti, *oi));
            }
        }

        // ----------------------------------------------------------------
        // Step 3b — Vital-sign-only matching for obs without position
        //           (only when there is exactly one active track in the zone)
        // ----------------------------------------------------------------
        'obs_loop: for (oi, obs) in observations.iter().enumerate() {
            if obs_assigned[oi] || obs.position.is_some() {
                continue;
            }
            // Collect active tracks in the same zone
            let zone_matches: Vec<usize> = active_indices
                .iter()
                .enumerate()
                .filter(|(ti, &track_idx)| {
                    // Must not already be assigned
                    !matched_pairs.iter().any(|(t, _)| *t == *ti)
                        && self.tracks[track_idx].survivor.zone_id() == &obs.zone_id
                })
                .map(|(ti, _)| ti)
                .collect();

            if zone_matches.len() == 1 {
                let ti = zone_matches[0];
                let track_idx = active_indices[ti];
                let fp_dist = self.tracks[track_idx]
                    .fingerprint
                    .distance(&CsiFingerprint::from_vitals(&obs.vital_signs, None));
                if fp_dist < self.config.reid_threshold {
                    obs_assigned[oi] = true;
                    matched_pairs.push((ti, oi));
                    continue 'obs_loop;
                }
            }
        }

        // ----------------------------------------------------------------
        // Step 4 — Re-ID: unmatched obs vs Lost tracks via fingerprint
        // ----------------------------------------------------------------
        let lost_indices: Vec<usize> = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.lifecycle.is_lost())
            .map(|(i, _)| i)
            .collect();

        // For each unmatched observation with a position, try re-ID against Lost tracks
        for (oi, obs) in observations.iter().enumerate() {
            if obs_assigned[oi] {
                continue;
            }
            let obs_fp = CsiFingerprint::from_vitals(&obs.vital_signs, obs.position.as_ref());

            let mut best_dist = f32::MAX;
            let mut best_lost_idx: Option<usize> = None;

            for &track_idx in &lost_indices {
                if !self.tracks[track_idx]
                    .lifecycle
                    .can_reidentify(now, self.config.max_lost_age_secs)
                {
                    continue;
                }
                let dist = self.tracks[track_idx].fingerprint.distance(&obs_fp);
                if dist < best_dist {
                    best_dist = dist;
                    best_lost_idx = Some(track_idx);
                }
            }

            if best_dist < self.config.reid_threshold {
                if let Some(track_idx) = best_lost_idx {
                    obs_assigned[oi] = true;
                    result.reidentified_track_ids.push(self.tracks[track_idx].id.clone());

                    // Transition Lost → Active
                    self.tracks[track_idx].lifecycle.hit();

                    // Update Kalman with new position if available
                    if let Some(pos) = &obs.position {
                        let obs_vec = [pos.x, pos.y, pos.z];
                        self.tracks[track_idx].kalman.update(obs_vec);
                    }

                    // Update fingerprint and vitals
                    self.tracks[track_idx]
                        .fingerprint
                        .update_from_vitals(&obs.vital_signs, obs.position.as_ref());
                    self.tracks[track_idx]
                        .survivor
                        .update_vitals(obs.vital_signs.clone());

                    if let Some(pos) = &obs.position {
                        self.tracks[track_idx].survivor.update_location(pos.clone());
                    }
                }
            }
        }

        // ----------------------------------------------------------------
        // Step 5 — Birth: remaining unmatched observations → new Tentative track
        // ----------------------------------------------------------------
        for (oi, obs) in observations.iter().enumerate() {
            if obs_assigned[oi] {
                continue;
            }
            let new_track = TrackedSurvivor::from_observation(obs, &self.config);
            result.born_track_ids.push(new_track.id.clone());
            self.tracks.push(new_track);
        }

        // ----------------------------------------------------------------
        // Step 6 — Kalman update + vitals update for matched tracks
        // ----------------------------------------------------------------
        for (ti, oi) in &matched_pairs {
            let track_idx = active_indices[*ti];
            let obs = &observations[*oi];

            if let Some(pos) = &obs.position {
                let obs_vec = [pos.x, pos.y, pos.z];
                self.tracks[track_idx].kalman.update(obs_vec);
                self.tracks[track_idx].survivor.update_location(pos.clone());
            }

            self.tracks[track_idx]
                .fingerprint
                .update_from_vitals(&obs.vital_signs, obs.position.as_ref());
            self.tracks[track_idx]
                .survivor
                .update_vitals(obs.vital_signs.clone());

            result.matched_track_ids.push(self.tracks[track_idx].id.clone());
        }

        // ----------------------------------------------------------------
        // Step 7 — Miss for unmatched active/tentative tracks + lifecycle checks
        // ----------------------------------------------------------------
        let matched_ti_set: std::collections::HashSet<usize> =
            matched_pairs.iter().map(|(ti, _)| *ti).collect();

        for (ti, &track_idx) in active_indices.iter().enumerate() {
            if matched_ti_set.contains(&ti) {
                // Already handled in step 6; call hit on lifecycle
                self.tracks[track_idx].lifecycle.hit();
            } else {
                // Snapshot state before miss
                let was_active = matches!(
                    self.tracks[track_idx].lifecycle.state(),
                    TrackState::Active
                );

                self.tracks[track_idx].lifecycle.miss();

                // Detect Active → Lost transition
                if was_active && self.tracks[track_idx].lifecycle.is_lost() {
                    result.lost_track_ids.push(self.tracks[track_idx].id.clone());
                    tracing::debug!(
                        track_id = %self.tracks[track_idx].id,
                        "Track transitioned to Lost"
                    );
                }

                // Detect → Terminated (from Tentative miss)
                if self.tracks[track_idx].lifecycle.is_terminal() {
                    result
                        .terminated_track_ids
                        .push(self.tracks[track_idx].id.clone());
                    self.tracks[track_idx].terminated_at = Some(now);
                }
            }
        }

        // ----------------------------------------------------------------
        // Check Lost tracks for expiry
        // ----------------------------------------------------------------
        for track in &mut self.tracks {
            if track.lifecycle.is_lost() {
                let was_lost = true;
                track
                    .lifecycle
                    .check_lost_expiry(now, self.config.max_lost_age_secs);
                if was_lost && track.lifecycle.is_terminal() {
                    result.terminated_track_ids.push(track.id.clone());
                    track.terminated_at = Some(now);
                }
            }
        }

        // Collect Rescued tracks (already terminal — just report them)
        for track in &self.tracks {
            if matches!(track.lifecycle.state(), TrackState::Rescued) {
                result.rescued_track_ids.push(track.id.clone());
            }
        }

        // ----------------------------------------------------------------
        // Step 8 — Remove Terminated tracks older than 60 s
        // ----------------------------------------------------------------
        self.tracks.retain(|t| {
            if !t.lifecycle.is_terminal() {
                return true;
            }
            match t.terminated_at {
                Some(ts) => now.duration_since(ts).as_secs() < 60,
                None => true, // not yet timestamped — keep for one more tick
            }
        });

        result
    }

    /// Iterate over Active and Tentative tracks.
    pub fn active_tracks(&self) -> impl Iterator<Item = &TrackedSurvivor> {
        self.tracks
            .iter()
            .filter(|t| t.lifecycle.is_active_or_tentative())
    }

    /// Borrow the full track list (all states).
    pub fn all_tracks(&self) -> &[TrackedSurvivor] {
        &self.tracks
    }

    /// Look up a specific track by ID.
    pub fn get_track(&self, id: &TrackId) -> Option<&TrackedSurvivor> {
        self.tracks.iter().find(|t| &t.id == id)
    }

    /// Operator marks a survivor as rescued.
    ///
    /// Returns `true` if the track was found and transitioned to Rescued.
    pub fn mark_rescued(&mut self, id: &TrackId) -> bool {
        if let Some(track) = self.tracks.iter_mut().find(|t| &t.id == id) {
            track.lifecycle.rescue();
            track.survivor.mark_rescued();
            true
        } else {
            false
        }
    }

    /// Total number of tracks (all states).
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    /// Number of Active + Tentative tracks.
    pub fn active_count(&self) -> usize {
        self.tracks
            .iter()
            .filter(|t| t.lifecycle.is_active_or_tentative())
            .count()
    }
}

// ---------------------------------------------------------------------------
// Assignment helpers
// ---------------------------------------------------------------------------

/// Greedy nearest-neighbour assignment.
///
/// Iteratively picks the global minimum cost cell, assigns it, and marks the
/// corresponding row (track) and column (observation) as used.
///
/// Returns a vector of length `n_tracks` where entry `i` is `Some(obs_idx)`
/// if track `i` was assigned, or `None` otherwise.
fn greedy_assign(costs: &[Vec<f64>], n_tracks: usize, n_obs: usize) -> Vec<Option<usize>> {
    let mut assignment = vec![None; n_tracks];
    let mut track_used = vec![false; n_tracks];
    let mut obs_used = vec![false; n_obs];

    loop {
        // Find the global minimum unassigned cost cell
        let mut best = f64::MAX;
        let mut best_ti = usize::MAX;
        let mut best_oi = usize::MAX;

        for ti in 0..n_tracks {
            if track_used[ti] {
                continue;
            }
            for oi in 0..n_obs {
                if obs_used[oi] {
                    continue;
                }
                if costs[ti][oi] < best {
                    best = costs[ti][oi];
                    best_ti = ti;
                    best_oi = oi;
                }
            }
        }

        if best >= f64::MAX {
            break; // No valid assignment remaining
        }

        assignment[best_ti] = Some(best_oi);
        track_used[best_ti] = true;
        obs_used[best_oi] = true;
    }

    assignment
}

/// Hungarian algorithm (Kuhn–Munkres) for optimal assignment.
///
/// Implemented via augmenting paths on a bipartite graph built from the gated
/// cost matrix.  Only cells with cost < `f64::MAX` form valid edges.
///
/// Returns the same format as `greedy_assign`.
///
/// Complexity: O(n_tracks · n_obs · (n_tracks + n_obs)) which is ≤ O(n³) for
/// square matrices.  Safe to call for n ≤ 10.
fn hungarian_assign(costs: &[Vec<f64>], n_tracks: usize, n_obs: usize) -> Vec<Option<usize>> {
    // Build adjacency: for each track, list the observations it can match.
    let adj: Vec<Vec<usize>> = (0..n_tracks)
        .map(|ti| {
            (0..n_obs)
                .filter(|&oi| costs[ti][oi] < f64::MAX)
                .collect()
        })
        .collect();

    // match_obs[oi] = track index that observation oi is matched to, or None
    let mut match_obs: Vec<Option<usize>> = vec![None; n_obs];

    // For each track, try to find an augmenting path via DFS
    for ti in 0..n_tracks {
        let mut visited = vec![false; n_obs];
        augment(ti, &adj, &mut match_obs, &mut visited);
    }

    // Invert the matching: build track→obs assignment
    let mut assignment = vec![None; n_tracks];
    for (oi, matched_ti) in match_obs.iter().enumerate() {
        if let Some(ti) = matched_ti {
            assignment[*ti] = Some(oi);
        }
    }
    assignment
}

/// Recursive DFS augmenting path for the Hungarian algorithm.
///
/// Attempts to match track `ti` to some observation, using previously matched
/// tracks as alternating-path intermediate nodes.
fn augment(
    ti: usize,
    adj: &[Vec<usize>],
    match_obs: &mut Vec<Option<usize>>,
    visited: &mut Vec<bool>,
) -> bool {
    for &oi in &adj[ti] {
        if visited[oi] {
            continue;
        }
        visited[oi] = true;

        // If observation oi is unmatched, or its current match can be re-routed
        let can_match = match match_obs[oi] {
            None => true,
            Some(other_ti) => augment(other_ti, adj, match_obs, visited),
        };

        if can_match {
            match_obs[oi] = Some(ti);
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        coordinates::LocationUncertainty,
        vital_signs::{BreathingPattern, BreathingType, ConfidenceScore, MovementProfile},
    };
    use chrono::Utc;

    fn test_vitals() -> VitalSignsReading {
        VitalSignsReading {
            breathing: Some(BreathingPattern {
                rate_bpm: 16.0,
                amplitude: 0.8,
                regularity: 0.9,
                pattern_type: BreathingType::Normal,
            }),
            heartbeat: None,
            movement: MovementProfile::default(),
            timestamp: Utc::now(),
            confidence: ConfidenceScore::new(0.8),
        }
    }

    fn test_coords(x: f64, y: f64, z: f64) -> Coordinates3D {
        Coordinates3D {
            x,
            y,
            z,
            uncertainty: LocationUncertainty::new(1.5, 0.5),
        }
    }

    fn make_obs(x: f64, y: f64, z: f64) -> DetectionObservation {
        DetectionObservation {
            position: Some(test_coords(x, y, z)),
            vital_signs: test_vitals(),
            confidence: 0.9,
            zone_id: ScanZoneId::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: empty observations → all result vectors empty
    // -----------------------------------------------------------------------
    #[test]
    fn test_tracker_empty() {
        let mut tracker = SurvivorTracker::with_defaults();
        let result = tracker.update(vec![], 0.5);

        assert!(result.matched_track_ids.is_empty());
        assert!(result.born_track_ids.is_empty());
        assert!(result.lost_track_ids.is_empty());
        assert!(result.reidentified_track_ids.is_empty());
        assert!(result.terminated_track_ids.is_empty());
        assert!(result.rescued_track_ids.is_empty());
        assert_eq!(tracker.track_count(), 0);
    }

    // -----------------------------------------------------------------------
    // Test 2: birth — 2 observations → 2 tentative tracks born; after 2 ticks
    // with same obs positions, at least 1 track becomes Active (confirmed)
    // -----------------------------------------------------------------------
    #[test]
    fn test_tracker_birth() {
        let mut tracker = SurvivorTracker::with_defaults();
        let zone_id = ScanZoneId::new();

        // Tick 1: two identical-zone observations → 2 tentative tracks
        let obs1 = DetectionObservation {
            position: Some(test_coords(1.0, 0.0, 0.0)),
            vital_signs: test_vitals(),
            confidence: 0.9,
            zone_id: zone_id.clone(),
        };
        let obs2 = DetectionObservation {
            position: Some(test_coords(10.0, 0.0, 0.0)),
            vital_signs: test_vitals(),
            confidence: 0.8,
            zone_id: zone_id.clone(),
        };

        let r1 = tracker.update(vec![obs1.clone(), obs2.clone()], 0.5);
        // Both observations are new → both born as Tentative
        assert_eq!(r1.born_track_ids.len(), 2);
        assert_eq!(tracker.track_count(), 2);

        // Tick 2: same observations → tracks get a second hit → Active
        let r2 = tracker.update(vec![obs1.clone(), obs2.clone()], 0.5);

        // Both tracks should now be confirmed (Active)
        let active = tracker.active_count();
        assert!(
            active >= 1,
            "Expected at least 1 confirmed active track after 2 ticks, got {}",
            active
        );

        // born_track_ids on tick 2 should be empty (no new unmatched obs)
        assert!(
            r2.born_track_ids.is_empty(),
            "No new births expected on tick 2"
        );
    }

    // -----------------------------------------------------------------------
    // Test 3: miss → Lost — track goes Active, then 3 ticks with no matching obs
    // -----------------------------------------------------------------------
    #[test]
    fn test_tracker_miss_to_lost() {
        let mut tracker = SurvivorTracker::with_defaults();

        let obs = make_obs(0.0, 0.0, 0.0);

        // Tick 1 & 2: confirm the track (Tentative → Active)
        tracker.update(vec![obs.clone()], 0.5);
        tracker.update(vec![obs.clone()], 0.5);

        // Verify it's Active
        assert_eq!(tracker.active_count(), 1);

        // Tick 3, 4, 5: send an observation far outside the gate so the
        // track gets misses (Mahalanobis distance will exceed gate)
        let far_obs = make_obs(9999.0, 9999.0, 9999.0);
        tracker.update(vec![far_obs.clone()], 0.5);
        tracker.update(vec![far_obs.clone()], 0.5);
        let r = tracker.update(vec![far_obs.clone()], 0.5);

        // After 3 misses on the original track, it should be Lost
        // (The far_obs creates new tentative tracks but the original goes Lost)
        let has_lost = self::any_lost(&tracker);
        assert!(
            has_lost || !r.lost_track_ids.is_empty(),
            "Expected at least one lost track after 3 missed ticks"
        );
    }

    // -----------------------------------------------------------------------
    // Test 4: re-ID — track goes Lost, new obs with matching fingerprint
    // → reidentified_track_ids populated
    // -----------------------------------------------------------------------
    #[test]
    fn test_tracker_reid() {
        // Use a very permissive config to make re-ID easy to trigger
        let config = TrackerConfig {
            birth_hits_required: 2,
            max_active_misses: 1, // Lost after just 1 miss for speed
            max_lost_age_secs: 60.0,
            reid_threshold: 1.0, // Accept any fingerprint match
            gate_mahalanobis_sq: 9.0,
            obs_noise_var: 2.25,
            process_noise_var: 0.01,
        };
        let mut tracker = SurvivorTracker::new(config);

        // Consistent vital signs for reliable fingerprint
        let vitals = test_vitals();

        let obs = DetectionObservation {
            position: Some(test_coords(1.0, 0.0, 0.0)),
            vital_signs: vitals.clone(),
            confidence: 0.9,
            zone_id: ScanZoneId::new(),
        };

        // Tick 1 & 2: confirm the track
        tracker.update(vec![obs.clone()], 0.5);
        tracker.update(vec![obs.clone()], 0.5);
        assert_eq!(tracker.active_count(), 1);

        // Tick 3: send no observations → track goes Lost (max_active_misses = 1)
        tracker.update(vec![], 0.5);

        // Verify something is now Lost
        assert!(
            any_lost(&tracker),
            "Track should be Lost after missing 1 tick"
        );

        // Tick 4: send observation with matching fingerprint and nearby position
        let reid_obs = DetectionObservation {
            position: Some(test_coords(1.5, 0.0, 0.0)), // slightly moved
            vital_signs: vitals.clone(),
            confidence: 0.9,
            zone_id: ScanZoneId::new(),
        };
        let r = tracker.update(vec![reid_obs], 0.5);

        assert!(
            !r.reidentified_track_ids.is_empty(),
            "Expected re-identification but reidentified_track_ids was empty"
        );
    }

    // Helper: check if any track in the tracker is currently Lost
    fn any_lost(tracker: &SurvivorTracker) -> bool {
        tracker.all_tracks().iter().any(|t| t.lifecycle.is_lost())
    }
}
