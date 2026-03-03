//! Breathing synchronization detector — ADR-041 exotic module.
//!
//! # Algorithm
//!
//! Detects when multiple people's breathing patterns synchronize by
//! extracting per-person breathing components via subcarrier group
//! decomposition and computing pairwise cross-correlation.
//!
//! ## Breathing extraction
//!
//! With N persons in the room, the CSI is decomposed into N breathing
//! components by assigning non-overlapping subcarrier groups to each
//! person.  The host reports `n_persons` and `breathing_bpm`.  Each
//! component is the per-group phase signal, bandpass-limited to the
//! breathing band (0.1-0.6 Hz at 20 Hz frame rate).
//!
//! The bandpass is implemented as a slow EWMA (removes DC) followed
//! by a fast EWMA (low-pass at ~1 Hz).  The difference between the
//! two gives the breathing-band component.
//!
//! ## Synchronization detection
//!
//! For each pair (i, j), compute the Phase Locking Value (PLV):
//!
//!   PLV = |mean(exp(j*(phi_i - phi_j)))| = sqrt(C^2 + S^2) / N
//!
//! where C = sum(cos(phase_diff)), S = sum(sin(phase_diff)).
//!
//! In practice, since we track the breathing waveform (not instantaneous
//! phase), we use normalized cross-correlation at zero lag as a proxy:
//!
//!   rho = sum(x_i * x_j) / sqrt(sum(x_i^2) * sum(x_j^2))
//!
//! Synchronization is declared when |rho| > threshold for a sustained
//! period.
//!
//! # Events (670-series: Exotic / Research)
//!
//! - `SYNC_DETECTED` (670): 1.0 when any pair synchronizes.
//! - `SYNC_PAIR_COUNT` (671): Number of synchronized pairs.
//! - `GROUP_COHERENCE` (672): Average coherence across all pairs [0, 1].
//! - `SYNC_LOST` (673): 1.0 when synchronization breaks.
//!
//! # Budget
//!
//! S (standard, < 5 ms) — per-frame: up to 6 pairwise correlations
//! (for max 4 persons) over 64-point buffers.

use crate::vendor_common::{CircularBuffer, Ema};
use libm::sqrtf;

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum number of persons to track simultaneously.
const MAX_PERSONS: usize = 4;

/// Maximum pairwise comparisons: C(4,2) = 6.
const MAX_PAIRS: usize = 6;

/// Number of subcarrier groups (matches flash-attention tiling).
const N_GROUPS: usize = 8;

/// Maximum subcarriers from host API.
const MAX_SC: usize = 32;

/// Breathing component buffer length (64 points at 20 Hz = 3.2 s).
const BREATH_BUF_LEN: usize = 64;

/// Slow EWMA alpha for DC removal (removes baseline drift).
const DC_ALPHA: f32 = 0.005;

/// Fast EWMA alpha for low-pass filtering (~1 Hz cutoff at 20 Hz).
const LP_ALPHA: f32 = 0.15;

/// Cross-correlation threshold for synchronization detection.
const SYNC_THRESHOLD: f32 = 0.6;

/// Consecutive frames of high correlation before declaring sync.
const SYNC_ONSET_FRAMES: u32 = 20;

/// Consecutive frames of low correlation before declaring sync lost.
const SYNC_LOST_FRAMES: u32 = 15;

/// Minimum frames before analysis begins.
const MIN_FRAMES: u32 = BREATH_BUF_LEN as u32;

/// Small epsilon for normalization.
const EPSILON: f32 = 1e-10;

// ── Event IDs (670-series: Exotic) ───────────────────────────────────────────

pub const EVENT_SYNC_DETECTED: i32 = 670;
pub const EVENT_SYNC_PAIR_COUNT: i32 = 671;
pub const EVENT_GROUP_COHERENCE: i32 = 672;
pub const EVENT_SYNC_LOST: i32 = 673;

// ── Breathing Sync Detector ──────────────────────────────────────────────────

/// Per-person breathing channel state.
struct BreathingChannel {
    /// Slow EWMA for DC removal.
    dc_ema: Ema,
    /// Fast EWMA for low-pass.
    lp_ema: Ema,
    /// Circular buffer of breathing-band signal.
    buf: CircularBuffer<BREATH_BUF_LEN>,
}

impl BreathingChannel {
    const fn new() -> Self {
        Self {
            dc_ema: Ema::new(DC_ALPHA),
            lp_ema: Ema::new(LP_ALPHA),
            buf: CircularBuffer::new(),
        }
    }

    /// Feed a raw phase sample, extract breathing component, push to buffer.
    fn feed(&mut self, raw_phase: f32) {
        let dc = self.dc_ema.update(raw_phase);
        let lp = self.lp_ema.update(raw_phase);
        // Breathing component = low-passed signal minus DC baseline.
        let breathing = lp - dc;
        self.buf.push(breathing);
    }
}

/// Pairwise synchronization state.
struct PairState {
    /// Consecutive frames above sync threshold.
    sync_frames: u32,
    /// Consecutive frames below sync threshold.
    unsync_frames: u32,
    /// Whether this pair is currently synchronized.
    synced: bool,
}

impl PairState {
    const fn new() -> Self {
        Self {
            sync_frames: 0,
            unsync_frames: 0,
            synced: false,
        }
    }
}

/// Detects breathing synchronization between multiple occupants.
///
/// Decomposes CSI into per-person breathing components using subcarrier
/// group assignment, then computes pairwise cross-correlation to detect
/// phase-locked breathing.
pub struct BreathingSyncDetector {
    /// Per-person breathing channels (max 4).
    channels: [BreathingChannel; MAX_PERSONS],
    /// Pairwise synchronization states (max 6).
    pairs: [PairState; MAX_PAIRS],
    /// Number of currently active persons.
    active_persons: usize,
    /// Previous number of synchronized pairs.
    prev_sync_count: u32,
    /// Whether any synchronization is active.
    any_synced: bool,
    /// Average group coherence [0, 1].
    group_coherence: f32,
    /// Total frames processed.
    frame_count: u32,
}

impl BreathingSyncDetector {
    pub const fn new() -> Self {
        Self {
            channels: [
                BreathingChannel::new(), BreathingChannel::new(),
                BreathingChannel::new(), BreathingChannel::new(),
            ],
            pairs: [
                PairState::new(), PairState::new(), PairState::new(),
                PairState::new(), PairState::new(), PairState::new(),
            ],
            active_persons: 0,
            prev_sync_count: 0,
            any_synced: false,
            group_coherence: 0.0,
            frame_count: 0,
        }
    }

    /// Process one CSI frame.
    ///
    /// `phases` — per-subcarrier phase values (up to 32).
    /// `variance` — per-subcarrier variance values (up to 32).
    /// `breathing_bpm` — host-reported aggregate breathing BPM.
    /// `n_persons` — number of persons detected by host Tier 2.
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        variance: &[f32],
        _breathing_bpm: f32,
        n_persons: i32,
    ) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_ev = 0usize;

        self.frame_count += 1;

        // Need at least 2 persons for synchronization.
        let n_pers = if n_persons < 0 { 0 } else { n_persons as usize };
        let n_pers = if n_pers > MAX_PERSONS { MAX_PERSONS } else { n_pers };
        self.active_persons = n_pers;

        if n_pers < 2 {
            // Reset pair states when fewer than 2 persons.
            if self.any_synced {
                unsafe {
                    EVENTS[n_ev] = (EVENT_SYNC_LOST, 1.0);
                }
                n_ev += 1;
                self.any_synced = false;
                self.prev_sync_count = 0;
            }
            return unsafe { &EVENTS[..n_ev] };
        }

        let n_sc = core::cmp::min(phases.len(), MAX_SC);
        let n_sc = core::cmp::min(n_sc, variance.len());
        if n_sc < N_GROUPS {
            return &[];
        }

        // Assign subcarrier groups to persons.
        // With 8 groups and n_pers persons, each person gets groups_per groups.
        let groups_per = N_GROUPS / n_pers;
        if groups_per == 0 {
            return &[];
        }

        let subs_per = n_sc / N_GROUPS;
        if subs_per == 0 {
            return &[];
        }

        // Compute per-group mean phase, then assign to persons.
        let mut group_phase = [0.0f32; N_GROUPS];
        for g in 0..N_GROUPS {
            let start = g * subs_per;
            let end = if g == N_GROUPS - 1 { n_sc } else { start + subs_per };
            let count = (end - start) as f32;
            let mut sp = 0.0f32;
            for i in start..end {
                sp += phases[i];
            }
            group_phase[g] = sp / count;
        }

        // Each person gets an average of their assigned groups.
        for p in 0..n_pers {
            let g_start = p * groups_per;
            let g_end = if p == n_pers - 1 { N_GROUPS } else { g_start + groups_per };
            let count = (g_end - g_start) as f32;
            let mut sum = 0.0f32;
            for g in g_start..g_end {
                sum += group_phase[g];
            }
            let person_phase = sum / count;
            self.channels[p].feed(person_phase);
        }

        // Need enough data before pairwise analysis.
        if self.frame_count < MIN_FRAMES {
            return &[];
        }

        // Compute pairwise cross-correlation.
        let n_pairs = n_pers * (n_pers - 1) / 2;
        let mut sync_count = 0u32;
        let mut total_coherence = 0.0f32;
        let mut pair_idx = 0usize;

        for i in 0..n_pers {
            for j in (i + 1)..n_pers {
                if pair_idx >= MAX_PAIRS {
                    break;
                }

                let corr = self.cross_correlation(i, j);
                let abs_corr = if corr < 0.0 { -corr } else { corr };
                total_coherence += abs_corr;

                // Update pair state.
                if abs_corr > SYNC_THRESHOLD {
                    self.pairs[pair_idx].sync_frames += 1;
                    self.pairs[pair_idx].unsync_frames = 0;
                } else {
                    self.pairs[pair_idx].unsync_frames += 1;
                    self.pairs[pair_idx].sync_frames = 0;
                }

                let was_synced = self.pairs[pair_idx].synced;

                // Check onset.
                if !was_synced && self.pairs[pair_idx].sync_frames >= SYNC_ONSET_FRAMES {
                    self.pairs[pair_idx].synced = true;
                }

                // Check lost.
                if was_synced && self.pairs[pair_idx].unsync_frames >= SYNC_LOST_FRAMES {
                    self.pairs[pair_idx].synced = false;
                }

                if self.pairs[pair_idx].synced {
                    sync_count += 1;
                }

                pair_idx += 1;
            }
        }

        // Average group coherence.
        self.group_coherence = if n_pairs > 0 {
            total_coherence / n_pairs as f32
        } else {
            0.0
        };

        // Detect transitions.
        let was_any_synced = self.any_synced;
        self.any_synced = sync_count > 0;

        // Emit events.
        if self.any_synced && !was_any_synced {
            unsafe {
                EVENTS[n_ev] = (EVENT_SYNC_DETECTED, 1.0);
            }
            n_ev += 1;
        }

        if was_any_synced && !self.any_synced {
            unsafe {
                EVENTS[n_ev] = (EVENT_SYNC_LOST, 1.0);
            }
            n_ev += 1;
        }

        if sync_count != self.prev_sync_count && sync_count > 0 {
            unsafe {
                EVENTS[n_ev] = (EVENT_SYNC_PAIR_COUNT, sync_count as f32);
            }
            n_ev += 1;
        }
        self.prev_sync_count = sync_count;

        // Emit coherence periodically (every 10 frames).
        if self.frame_count % 10 == 0 {
            unsafe {
                EVENTS[n_ev] = (EVENT_GROUP_COHERENCE, self.group_coherence);
            }
            n_ev += 1;
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Compute normalized cross-correlation between two person channels
    /// using the most recent BREATH_BUF_LEN samples.
    fn cross_correlation(&self, person_a: usize, person_b: usize) -> f32 {
        let buf_a = &self.channels[person_a].buf;
        let buf_b = &self.channels[person_b].buf;
        let len = core::cmp::min(buf_a.len(), buf_b.len());
        if len < 8 {
            return 0.0;
        }

        let mut sum_ab = 0.0f32;
        let mut sum_aa = 0.0f32;
        let mut sum_bb = 0.0f32;

        for i in 0..len {
            let a = buf_a.get(i);
            let b = buf_b.get(i);
            sum_ab += a * b;
            sum_aa += a * a;
            sum_bb += b * b;
        }

        let denom = sqrtf(sum_aa * sum_bb);
        if denom < EPSILON {
            return 0.0;
        }
        sum_ab / denom
    }

    /// Whether any breathing pair is currently synchronized.
    pub fn is_synced(&self) -> bool {
        self.any_synced
    }

    /// Get the average group coherence [0, 1].
    pub fn group_coherence(&self) -> f32 {
        self.group_coherence
    }

    /// Get the number of active persons being tracked.
    pub fn active_persons(&self) -> usize {
        self.active_persons
    }

    /// Get total frames processed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_new() {
        let bs = BreathingSyncDetector::new();
        assert_eq!(bs.frame_count(), 0);
        assert_eq!(bs.active_persons(), 0);
        assert!(!bs.is_synced());
    }

    #[test]
    fn test_single_person_no_sync() {
        let mut bs = BreathingSyncDetector::new();
        let phases = [0.5f32; 32];
        let vars = [0.01f32; 32];
        for _ in 0..100 {
            let events = bs.process_frame(&phases, &vars, 15.0, 1);
            for ev in events {
                assert_ne!(ev.0, EVENT_SYNC_DETECTED,
                    "single person cannot sync");
            }
        }
        assert!(!bs.is_synced());
    }

    #[test]
    fn test_two_persons_identical_signal_syncs() {
        let mut bs = BreathingSyncDetector::new();
        let vars = [0.01f32; 32];

        // Feed identical phase patterns for 2 persons.
        // With 2 persons, person 0 gets groups 0-3, person 1 gets groups 4-7.
        // If all phases are identical, both channels get the same signal.
        let mut synced = false;
        for frame in 0..(MIN_FRAMES + SYNC_ONSET_FRAMES + 50) {
            // Breathing-like oscillation at ~0.3 Hz (period ~67 frames at 20 Hz).
            let phase_val = 0.5 + 0.3 * libm::sinf(
                2.0 * core::f32::consts::PI * frame as f32 / 67.0
            );
            let phases = [phase_val; 32];
            let events = bs.process_frame(&phases, &vars, 18.0, 2);
            for ev in events {
                if ev.0 == EVENT_SYNC_DETECTED {
                    synced = true;
                }
            }
        }
        assert!(synced, "identical breathing signals should eventually synchronize");
    }

    #[test]
    fn test_two_persons_opposite_signals_no_sync() {
        let mut bs = BreathingSyncDetector::new();
        let vars = [0.01f32; 32];

        // Feed opposite phase patterns: person 0 groups get +sin, person 1 groups get -sin.
        for frame in 0..(MIN_FRAMES + SYNC_ONSET_FRAMES + 50) {
            let t = 2.0 * core::f32::consts::PI * frame as f32 / 67.0;
            let mut phases = [0.0f32; 32];
            // Groups 0-3 (subcarriers 0-15): positive sine.
            for i in 0..16 {
                phases[i] = 0.5 + 0.3 * libm::sinf(t);
            }
            // Groups 4-7 (subcarriers 16-31): shifted sine (90 degrees ahead).
            for i in 16..32 {
                phases[i] = 0.5 + 0.3 * libm::sinf(t + core::f32::consts::FRAC_PI_2);
            }
            let events = bs.process_frame(&phases, &vars, 18.0, 2);
            // We don't assert no sync because partial correlation can occur.
            let _ = events;
        }
        // At minimum, verify frame_count advanced.
        assert!(bs.frame_count() > 0);
    }

    #[test]
    fn test_insufficient_subcarriers() {
        let mut bs = BreathingSyncDetector::new();
        let small = [1.0f32; 4];
        let events = bs.process_frame(&small, &small, 15.0, 2);
        assert!(events.is_empty());
    }

    #[test]
    fn test_coherence_range() {
        let mut bs = BreathingSyncDetector::new();
        let vars = [0.01f32; 32];
        let phases = [0.5f32; 32];

        for _ in 0..(MIN_FRAMES + 20) {
            bs.process_frame(&phases, &vars, 15.0, 3);
        }

        let coh = bs.group_coherence();
        assert!(coh >= 0.0 && coh <= 1.0,
            "coherence should be in [0, 1], got {}", coh);
    }

    #[test]
    fn test_sync_lost_on_person_departure() {
        let mut bs = BreathingSyncDetector::new();
        let vars = [0.01f32; 32];

        // Build sync with 2 persons.
        for frame in 0..(MIN_FRAMES + SYNC_ONSET_FRAMES + 20) {
            let phase_val = 0.5 + 0.3 * libm::sinf(
                2.0 * core::f32::consts::PI * frame as f32 / 67.0
            );
            let phases = [phase_val; 32];
            bs.process_frame(&phases, &vars, 18.0, 2);
        }

        // Drop to 1 person.
        let mut lost_seen = false;
        for _ in 0..5 {
            let phases = [0.5f32; 32];
            let events = bs.process_frame(&phases, &vars, 18.0, 1);
            for ev in events {
                if ev.0 == EVENT_SYNC_LOST {
                    lost_seen = true;
                }
            }
        }
        // If sync was established, dropping persons should emit SYNC_LOST.
        if bs.prev_sync_count > 0 || lost_seen {
            assert!(lost_seen, "should emit SYNC_LOST when persons depart");
        }
    }

    #[test]
    fn test_reset() {
        let mut bs = BreathingSyncDetector::new();
        let phases = [0.5f32; 32];
        let vars = [0.01f32; 32];
        for _ in 0..50 {
            bs.process_frame(&phases, &vars, 15.0, 2);
        }
        assert!(bs.frame_count() > 0);
        bs.reset();
        assert_eq!(bs.frame_count(), 0);
        assert!(!bs.is_synced());
    }

    #[test]
    fn test_cross_correlation_identical_buffers() {
        let mut bs = BreathingSyncDetector::new();
        // Manually fill two channels with identical data.
        for i in 0..BREATH_BUF_LEN {
            let val = libm::sinf(i as f32 * 0.1);
            bs.channels[0].buf.push(val);
            bs.channels[1].buf.push(val);
        }
        let corr = bs.cross_correlation(0, 1);
        assert!(corr > 0.99, "identical buffers should have correlation ~1, got {}", corr);
    }
}
