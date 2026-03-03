//! Min-cut based multi-person identity tracking — ADR-041 signal module.
//!
//! Maintains per-person CSI signatures (up to 4 persons) as 8-element feature
//! vectors derived from subcarrier variance patterns.  Each frame, the module
//! extracts current-frame features for each detected person, builds a bipartite
//! cost matrix (L2 distance), and performs greedy Hungarian-lite assignment to
//! maintain stable person IDs across frames.
//!
//! Ported from `ruvector-mincut` concepts (DynamicPersonMatcher) for WASM
//! edge execution on ESP32-S3.
//!
//! Budget: H (heavy, < 10ms).

use libm::sqrtf;

/// Maximum persons to track simultaneously.
const MAX_PERSONS: usize = 4;

/// Feature vector dimension per person (top-8 subcarrier variances).
const FEAT_DIM: usize = 8;

/// Maximum subcarriers to process.
const MAX_SC: usize = 32;

/// EMA blending factor for signature updates.
const SIG_ALPHA: f32 = 0.15;

/// Maximum L2 distance for a valid match (above this, treat as new person).
const MAX_MATCH_DISTANCE: f32 = 5.0;

/// Minimum frames a person must be tracked before being considered stable.
const STABLE_FRAMES: u16 = 10;

/// Frames of absence before a person slot is released.
const ABSENT_TIMEOUT: u16 = 100;

/// Sentinel value for unassigned slots.
const UNASSIGNED: u8 = 255;

/// Event IDs (700-series: Signal Processing — Person Tracking).
pub const EVENT_PERSON_ID_ASSIGNED: i32 = 720;
pub const EVENT_PERSON_ID_SWAP: i32 = 721;
pub const EVENT_MATCH_CONFIDENCE: i32 = 722;

/// Per-person tracked state.
struct PersonSlot {
    signature: [f32; FEAT_DIM],  // EMA-smoothed variance features
    active: bool,
    tracked_frames: u16,
    absent_frames: u16,
    person_id: u8,
}

impl PersonSlot {
    const fn new(id: u8) -> Self {
        Self { signature: [0.0; FEAT_DIM], active: false, tracked_frames: 0, absent_frames: 0, person_id: id }
    }
}

/// Min-cut person identity matcher.
pub struct PersonMatcher {
    slots: [PersonSlot; MAX_PERSONS],
    active_count: u8,
    prev_assignment: [u8; MAX_PERSONS],
    frame_count: u32,
    swap_count: u32,
}

impl PersonMatcher {
    pub const fn new() -> Self {
        Self {
            slots: [
                PersonSlot::new(0),
                PersonSlot::new(1),
                PersonSlot::new(2),
                PersonSlot::new(3),
            ],
            active_count: 0,
            prev_assignment: [UNASSIGNED; MAX_PERSONS],
            frame_count: 0,
            swap_count: 0,
        }
    }

    /// Process one CSI frame.  `n_persons` = detected persons (0..=4).
    /// Returns events as (event_type, value) pairs.
    pub fn process_frame(
        &mut self,
        amplitudes: &[f32],
        variances: &[f32],
        n_persons: usize,
    ) -> &[(i32, f32)] {
        let n_sc = amplitudes.len().min(variances.len()).min(MAX_SC);
        if n_sc < FEAT_DIM {
            return &[];
        }

        self.frame_count += 1;
        let n_det = n_persons.min(MAX_PERSONS);

        static mut EVENTS: [(i32, f32); 8] = [(0, 0.0); 8];
        let mut n_events = 0usize;

        // Extract per-person feature vectors (spatial region -> top-8 variances).
        let mut current_features = [[0.0f32; FEAT_DIM]; MAX_PERSONS];

        if n_det > 0 {
            let subs_per_person = n_sc / n_det;
            for p in 0..n_det {
                let start = p * subs_per_person;
                let end = if p == n_det - 1 { n_sc } else { start + subs_per_person };
                self.extract_features(
                    variances,
                    start,
                    end,
                    &mut current_features[p],
                );
            }
        }

        // Build cost matrix and greedy-assign.
        let mut assignment = [UNASSIGNED; MAX_PERSONS];
        let mut costs = [0.0f32; MAX_PERSONS];
        if n_det > 0 {
            self.greedy_assign(&current_features, n_det, &mut assignment, &mut costs);
        }

        // Detect ID swaps.
        for p in 0..n_det {
            let curr = assignment[p];
            let prev = self.prev_assignment[p];

            if prev != UNASSIGNED && curr != UNASSIGNED && curr != prev {
                self.swap_count += 1;
                if n_events < 7 {
                    let swap_val = (prev as f32) * 16.0 + (curr as f32);
                    unsafe {
                        EVENTS[n_events] = (EVENT_PERSON_ID_SWAP, swap_val);
                    }
                    n_events += 1;
                }
            }
        }

        // Update signatures via EMA blending.
        for slot in self.slots.iter_mut() {
            if slot.active {
                slot.absent_frames = slot.absent_frames.saturating_add(1);
            }
        }

        for p in 0..n_det {
            let slot_idx = assignment[p] as usize;
            if slot_idx >= MAX_PERSONS {
                continue;
            }

            let slot = &mut self.slots[slot_idx];

            if slot.active {
                for f in 0..FEAT_DIM {
                    slot.signature[f] = SIG_ALPHA * current_features[p][f]
                        + (1.0 - SIG_ALPHA) * slot.signature[f];
                }
                slot.tracked_frames = slot.tracked_frames.saturating_add(1);
            } else {
                slot.signature = current_features[p];
                slot.active = true;
                slot.tracked_frames = 1;
            }
            slot.absent_frames = 0;

            if n_events < 7 {
                let confidence = if costs[p] < MAX_MATCH_DISTANCE {
                    1.0 - costs[p] / MAX_MATCH_DISTANCE
                } else {
                    0.0
                };
                let val = slot.person_id as f32 + confidence.min(0.99) * 0.01;
                unsafe {
                    EVENTS[n_events] = (EVENT_PERSON_ID_ASSIGNED, val);
                }
                n_events += 1;
            }
        }

        // Release timed-out slots.
        let mut active = 0u8;
        for slot in self.slots.iter_mut() {
            if slot.active && slot.absent_frames >= ABSENT_TIMEOUT {
                slot.active = false;
                slot.tracked_frames = 0;
                slot.absent_frames = 0;
                slot.signature = [0.0; FEAT_DIM];
            }
            if slot.active {
                active += 1;
            }
        }
        self.active_count = active;

        // Emit aggregate confidence (every 10 frames).
        if self.frame_count % 10 == 0 && n_det > 0 {
            let mut avg_conf = 0.0f32;
            for p in 0..n_det {
                let c = if costs[p] < MAX_MATCH_DISTANCE {
                    1.0 - costs[p] / MAX_MATCH_DISTANCE
                } else {
                    0.0
                };
                avg_conf += c;
            }
            avg_conf /= n_det as f32;

            if n_events < 8 {
                unsafe {
                    EVENTS[n_events] = (EVENT_MATCH_CONFIDENCE, avg_conf);
                }
                n_events += 1;
            }
        }

        // Save current assignment for next-frame swap detection.
        self.prev_assignment = assignment;

        unsafe { &EVENTS[..n_events] }
    }

    /// Extract top-FEAT_DIM variance values (descending) from a subcarrier range.
    fn extract_features(
        &self,
        variances: &[f32],
        start: usize,
        end: usize,
        out: &mut [f32; FEAT_DIM],
    ) {
        let count = end - start;
        let mut vals = [0.0f32; MAX_SC];
        for i in 0..count.min(MAX_SC) {
            vals[i] = variances[start + i];
        }

        let n = count.min(MAX_SC);
        let pick = FEAT_DIM.min(n);
        for i in 0..pick {
            let mut max_idx = i;
            for j in (i + 1)..n {
                if vals[j] > vals[max_idx] {
                    max_idx = j;
                }
            }
            let tmp = vals[i];
            vals[i] = vals[max_idx];
            vals[max_idx] = tmp;
            out[i] = vals[i];
        }

        for i in pick..FEAT_DIM {
            out[i] = 0.0;
        }
    }

    /// Greedy bipartite assignment (Hungarian-lite for max 4 persons).
    /// Picks minimum-cost pair, removes row+col, repeats.
    fn greedy_assign(
        &self,
        current: &[[f32; FEAT_DIM]; MAX_PERSONS],
        n_det: usize,
        assignment: &mut [u8; MAX_PERSONS],
        costs: &mut [f32; MAX_PERSONS],
    ) {
        let mut cost_matrix = [[f32::MAX; MAX_PERSONS]; MAX_PERSONS];
        let mut active_slots = [false; MAX_PERSONS];
        let mut n_active = 0usize;

        for s in 0..MAX_PERSONS {
            if self.slots[s].active {
                active_slots[s] = true;
                n_active += 1;
                for d in 0..n_det {
                    cost_matrix[d][s] = self.l2_distance(
                        &current[d],
                        &self.slots[s].signature,
                    );
                }
            }
        }

        let mut det_used = [false; MAX_PERSONS];
        let mut slot_used = [false; MAX_PERSONS];

        let passes = n_det.min(n_active);
        for _ in 0..passes {
            let mut min_cost = f32::MAX;
            let mut best_d = 0usize;
            let mut best_s = 0usize;

            for d in 0..n_det {
                if det_used[d] {
                    continue;
                }
                for s in 0..MAX_PERSONS {
                    if slot_used[s] || !active_slots[s] {
                        continue;
                    }
                    if cost_matrix[d][s] < min_cost {
                        min_cost = cost_matrix[d][s];
                        best_d = d;
                        best_s = s;
                    }
                }
            }

            if min_cost > MAX_MATCH_DISTANCE { break; }
            assignment[best_d] = best_s as u8;
            costs[best_d] = min_cost;
            det_used[best_d] = true;
            slot_used[best_s] = true;
        }

        // Assign unmatched detections to free slots (prefer inactive, then any).
        for d in 0..n_det {
            if assignment[d] != UNASSIGNED { continue; }
            for s in 0..MAX_PERSONS {
                if !slot_used[s] && !self.slots[s].active {
                    assignment[d] = s as u8;
                    costs[d] = MAX_MATCH_DISTANCE;
                    slot_used[s] = true;
                    break;
                }
            }
            if assignment[d] != UNASSIGNED { continue; }
            for s in 0..MAX_PERSONS {
                if !slot_used[s] {
                    assignment[d] = s as u8;
                    costs[d] = MAX_MATCH_DISTANCE;
                    slot_used[s] = true;
                    break;
                }
            }
        }
    }

    /// L2 distance between two feature vectors.
    #[inline]
    fn l2_distance(&self, a: &[f32; FEAT_DIM], b: &[f32; FEAT_DIM]) -> f32 {
        let mut sum = 0.0f32;
        for i in 0..FEAT_DIM {
            let d = a[i] - b[i];
            sum += d * d;
        }
        sqrtf(sum)
    }

    /// Get the number of currently active person tracks.
    pub fn active_persons(&self) -> u8 {
        self.active_count
    }

    /// Get the total number of ID swaps detected.
    pub fn total_swaps(&self) -> u32 {
        self.swap_count
    }

    /// Check if a specific person slot is stable (tracked long enough).
    pub fn is_person_stable(&self, slot: usize) -> bool {
        slot < MAX_PERSONS
            && self.slots[slot].active
            && self.slots[slot].tracked_frames >= STABLE_FRAMES
    }

    /// Get the signature of a person slot (for external use).
    pub fn person_signature(&self, slot: usize) -> Option<&[f32; FEAT_DIM]> {
        if slot < MAX_PERSONS && self.slots[slot].active {
            Some(&self.slots[slot].signature)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_person_matcher_init() {
        let pm = PersonMatcher::new();
        assert_eq!(pm.active_persons(), 0);
        assert_eq!(pm.total_swaps(), 0);
        assert_eq!(pm.frame_count, 0);
    }

    #[test]
    fn test_no_persons_no_events() {
        let mut pm = PersonMatcher::new();
        let amps = [1.0f32; 16];
        let vars = [0.1f32; 16];

        let events = pm.process_frame(&amps, &vars, 0);
        assert!(events.is_empty());
        assert_eq!(pm.active_persons(), 0);
    }

    #[test]
    fn test_single_person_tracking() {
        let mut pm = PersonMatcher::new();
        let amps = [1.0f32; 16];
        let mut vars = [0.0f32; 16];
        // Create a distinctive variance pattern.
        for i in 0..16 {
            vars[i] = 0.5 + 0.1 * (i as f32);
        }

        // Track 1 person over several frames.
        for _ in 0..20 {
            pm.process_frame(&amps, &vars, 1);
        }

        assert_eq!(pm.active_persons(), 1);
        assert!(pm.is_person_stable(0) || pm.is_person_stable(1)
                || pm.is_person_stable(2) || pm.is_person_stable(3),
                "at least one slot should be stable after 20 frames");
    }

    #[test]
    fn test_two_persons_distinct_signatures() {
        let mut pm = PersonMatcher::new();
        let amps = [1.0f32; 32];

        // Two persons with very different variance profiles.
        let mut vars = [0.0f32; 32];
        // Person 0 region (subcarriers 0-15): high variance.
        for i in 0..16 {
            vars[i] = 2.0 + 0.3 * (i as f32);
        }
        // Person 1 region (subcarriers 16-31): low variance.
        for i in 16..32 {
            vars[i] = 0.1 + 0.02 * ((i - 16) as f32);
        }

        for _ in 0..20 {
            pm.process_frame(&amps, &vars, 2);
        }

        assert_eq!(pm.active_persons(), 2);
        assert_eq!(pm.total_swaps(), 0, "no swaps expected with stable signatures");
    }

    #[test]
    fn test_person_timeout() {
        let mut pm = PersonMatcher::new();
        let amps = [1.0f32; 16];
        let vars = [0.5f32; 16];

        // Activate 1 person.
        for _ in 0..5 {
            pm.process_frame(&amps, &vars, 1);
        }
        assert_eq!(pm.active_persons(), 1);

        // Now send 0 persons for ABSENT_TIMEOUT frames.
        for _ in 0..ABSENT_TIMEOUT as usize + 1 {
            pm.process_frame(&amps, &vars, 0);
        }

        assert_eq!(pm.active_persons(), 0, "person should time out after absence");
    }

    #[test]
    fn test_l2_distance_zero() {
        let pm = PersonMatcher::new();
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        assert!(pm.l2_distance(&a, &a) < 1e-6);
    }

    #[test]
    fn test_l2_distance_known() {
        let pm = PersonMatcher::new();
        let a = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let b = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert!((pm.l2_distance(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_assignment_events_emitted() {
        let mut pm = PersonMatcher::new();
        let amps = [1.0f32; 16];
        let vars = [0.5f32; 16];

        let events = pm.process_frame(&amps, &vars, 1);

        let mut found_assignment = false;
        for &(et, _) in events {
            if et == EVENT_PERSON_ID_ASSIGNED {
                found_assignment = true;
            }
        }
        assert!(found_assignment, "should emit person ID assignment event");
    }

    #[test]
    fn test_too_few_subcarriers() {
        let mut pm = PersonMatcher::new();
        let amps = [1.0f32; 4];
        let vars = [0.5f32; 4];

        // With only 4 subcarriers (< FEAT_DIM=8), should return empty.
        let events = pm.process_frame(&amps, &vars, 1);
        assert!(events.is_empty());
    }

    #[test]
    fn test_extract_features_sorted() {
        let pm = PersonMatcher::new();
        let vars = [0.1, 0.5, 0.3, 0.9, 0.2, 0.7, 0.4, 0.8,
                     0.6, 0.15, 0.25, 0.35, 0.45, 0.55, 0.65, 0.75];
        let mut out = [0.0f32; FEAT_DIM];
        pm.extract_features(&vars, 0, 16, &mut out);

        // Features should be sorted descending (top-8 variances).
        for i in 0..FEAT_DIM - 1 {
            assert!(
                out[i] >= out[i + 1],
                "features should be sorted descending: out[{}]={} < out[{}]={}",
                i, out[i], i + 1, out[i + 1],
            );
        }
        // Highest should be 0.9.
        assert!((out[0] - 0.9).abs() < 1e-6);
    }
}
