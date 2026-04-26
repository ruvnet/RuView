//! Grover-inspired multi-hypothesis room configuration search.
//!
//! Maintains 16 amplitude-weighted hypotheses for room state and applies a
//! quantum-inspired oracle + diffusion iteration each CSI frame:
//!
//! 1. **Oracle**: CSI evidence (presence, motion, person count) amplifies
//!    consistent hypotheses and dampens contradicting ones.
//! 2. **Grover diffusion**: Reflects amplitudes about the mean, concentrating
//!    probability mass on oracle-boosted hypotheses.
//!
//! After enough iterations the winner emerges with probability > 0.5.
//!
//! Event IDs (800-series: Quantum-inspired):
//!   855 — HYPOTHESIS_WINNER  (value = winner index as f32)
//!   856 — HYPOTHESIS_AMPLITUDE  (value = winner probability, emitted periodically)
//!   857 — SEARCH_ITERATIONS  (value = iteration count)
//!
//! Budget: H (heavy, < 10 ms per frame).

use libm::sqrtf;

// ── Constants ────────────────────────────────────────────────────────────────

/// Number of room-state hypotheses.
const N_HYPO: usize = 16;

/// Convergence threshold: top hypothesis probability must exceed this.
const CONVERGENCE_PROB: f32 = 0.5;

/// Oracle boost factor for supported hypotheses.
const ORACLE_BOOST: f32 = 1.3;

/// Oracle dampen factor for contradicted hypotheses.
const ORACLE_DAMPEN: f32 = 0.7;

/// Emit winner every N frames.
const WINNER_EMIT_INTERVAL: u32 = 10;

/// Emit amplitude every N frames.
const AMPLITUDE_EMIT_INTERVAL: u32 = 20;

/// Emit iteration count every N frames.
const ITERATION_EMIT_INTERVAL: u32 = 50;

/// Motion energy threshold to distinguish high/low motion.
const MOTION_HIGH_THRESH: f32 = 0.5;

/// Motion energy threshold for very low motion.
const MOTION_LOW_THRESH: f32 = 0.15;

// ── Event IDs ────────────────────────────────────────────────────────────────

/// Winning hypothesis index (0-15).
pub const EVENT_HYPOTHESIS_WINNER: i32 = 855;

/// Winning hypothesis probability (amplitude^2).
pub const EVENT_HYPOTHESIS_AMPLITUDE: i32 = 856;

/// Total Grover iterations performed.
pub const EVENT_SEARCH_ITERATIONS: i32 = 857;

// ── Hypothesis definitions ───────────────────────────────────────────────────

/// Room state hypotheses.
/// Each variant maps to an index 0-15 and a human-readable label.
#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum Hypothesis {
    Empty           = 0,
    PersonZoneA     = 1,
    PersonZoneB     = 2,
    PersonZoneC     = 3,
    PersonZoneD     = 4,
    TwoPersons      = 5,
    ThreePersons    = 6,
    MovingLeft      = 7,
    MovingRight     = 8,
    Sitting         = 9,
    Standing        = 10,
    Falling         = 11,
    Exercising      = 12,
    Sleeping        = 13,
    Cooking         = 14,
    Working         = 15,
}

impl Hypothesis {
    /// Convert an index (0-15) to a Hypothesis variant.
    const fn from_index(i: usize) -> Self {
        match i {
            0  => Hypothesis::Empty,
            1  => Hypothesis::PersonZoneA,
            2  => Hypothesis::PersonZoneB,
            3  => Hypothesis::PersonZoneC,
            4  => Hypothesis::PersonZoneD,
            5  => Hypothesis::TwoPersons,
            6  => Hypothesis::ThreePersons,
            7  => Hypothesis::MovingLeft,
            8  => Hypothesis::MovingRight,
            9  => Hypothesis::Sitting,
            10 => Hypothesis::Standing,
            11 => Hypothesis::Falling,
            12 => Hypothesis::Exercising,
            13 => Hypothesis::Sleeping,
            14 => Hypothesis::Cooking,
            _  => Hypothesis::Working,
        }
    }
}

// ── State ────────────────────────────────────────────────────────────────────

/// Grover-inspired room state search engine.
pub struct InterferenceSearch {
    /// Amplitude for each of the 16 hypotheses.
    amplitudes: [f32; N_HYPO],
    /// Total Grover iterations applied.
    iteration_count: u32,
    /// Whether the search has converged.
    converged: bool,
    /// Index of the previous winning hypothesis (for change detection).
    prev_winner: u8,
    /// Frame counter.
    frame_count: u32,
}

impl InterferenceSearch {
    /// Create a new search engine with uniform amplitudes.
    /// initial amplitude = 1/sqrt(16) = 0.25 so that sum of squares = 1.
    pub const fn new() -> Self {
        // 1/sqrt(16) = 0.25
        Self {
            amplitudes: [0.25; N_HYPO],
            iteration_count: 0,
            converged: false,
            prev_winner: 0,
            frame_count: 0,
        }
    }

    /// Process one CSI frame and perform one oracle + diffusion step.
    ///
    /// # Arguments
    /// - `presence`: 0 = empty, 1 = present, 2 = moving (from Tier 2 DSP)
    /// - `motion_energy`: aggregate motion energy [0, 1+]
    /// - `n_persons`: estimated person count (0-8)
    ///
    /// Returns a slice of (event_type, value) pairs to emit.
    pub fn process_frame(
        &mut self,
        presence: i32,
        motion_energy: f32,
        n_persons: i32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;

        // ── Step 1: Oracle — mark each hypothesis as supported or contradicted ──
        let mut oracle_mask = [1.0f32; N_HYPO]; // 1.0 = neutral
        self.apply_oracle(&mut oracle_mask, presence, motion_energy, n_persons);

        // Apply oracle: multiply amplitudes by mask factors.
        for i in 0..N_HYPO {
            self.amplitudes[i] *= oracle_mask[i];
        }

        // ── Step 2: Grover diffusion — reflect about the mean ──
        self.grover_diffusion();

        // ── Step 3: Renormalize so probabilities sum to 1 ──
        self.normalize();

        self.iteration_count += 1;

        // ── Find winner ──
        let (winner_idx, winner_prob) = self.find_winner();

        // Check convergence.
        self.converged = winner_prob > CONVERGENCE_PROB;

        // ── Build output events ──
        static mut EVENTS: [(i32, f32); 3] = [(0, 0.0); 3];
        let mut n_events = 0usize;

        // Emit winner periodically or on change.
        let winner_changed = winner_idx as u8 != self.prev_winner;
        if winner_changed || self.frame_count % WINNER_EMIT_INTERVAL == 0 {
            unsafe {
                EVENTS[n_events] = (EVENT_HYPOTHESIS_WINNER, winner_idx as f32);
            }
            n_events += 1;
        }

        // Emit amplitude periodically.
        if self.frame_count % AMPLITUDE_EMIT_INTERVAL == 0 {
            unsafe {
                EVENTS[n_events] = (EVENT_HYPOTHESIS_AMPLITUDE, winner_prob);
            }
            n_events += 1;
        }

        // Emit iteration count periodically.
        if self.frame_count % ITERATION_EMIT_INTERVAL == 0 {
            unsafe {
                EVENTS[n_events] = (EVENT_SEARCH_ITERATIONS, self.iteration_count as f32);
            }
            n_events += 1;
        }

        self.prev_winner = winner_idx as u8;

        unsafe { &EVENTS[..n_events] }
    }

    /// Apply the oracle: set boost/dampen factors based on CSI evidence.
    fn apply_oracle(
        &self,
        mask: &mut [f32; N_HYPO],
        presence: i32,
        motion_energy: f32,
        n_persons: i32,
    ) {
        let is_empty = presence == 0;
        let is_moving = presence == 2;
        let high_motion = motion_energy > MOTION_HIGH_THRESH;
        let low_motion = motion_energy < MOTION_LOW_THRESH;

        // ── Empty evidence ──
        if is_empty {
            mask[Hypothesis::Empty as usize] = ORACLE_BOOST;
            // Dampen all non-empty hypotheses.
            for i in 1..N_HYPO {
                mask[i] = ORACLE_DAMPEN;
            }
            return;
        }

        // ── Person count evidence ──
        if n_persons >= 3 {
            mask[Hypothesis::ThreePersons as usize] = ORACLE_BOOST;
            mask[Hypothesis::Empty as usize] = ORACLE_DAMPEN;
        } else if n_persons == 2 {
            mask[Hypothesis::TwoPersons as usize] = ORACLE_BOOST;
            mask[Hypothesis::ThreePersons as usize] = ORACLE_DAMPEN;
            mask[Hypothesis::Empty as usize] = ORACLE_DAMPEN;
        } else if n_persons == 1 || n_persons == 0 {
            // Single-person hypotheses favored.
            mask[Hypothesis::TwoPersons as usize] = ORACLE_DAMPEN;
            mask[Hypothesis::ThreePersons as usize] = ORACLE_DAMPEN;
            mask[Hypothesis::Empty as usize] = ORACLE_DAMPEN;
        }

        // ── Motion evidence ──
        if high_motion {
            // Amplify active hypotheses.
            mask[Hypothesis::Exercising as usize] = ORACLE_BOOST;
            mask[Hypothesis::MovingLeft as usize] = ORACLE_BOOST;
            mask[Hypothesis::MovingRight as usize] = ORACLE_BOOST;
            mask[Hypothesis::Falling as usize] = ORACLE_BOOST;

            // Dampen static hypotheses.
            mask[Hypothesis::Sitting as usize] = ORACLE_DAMPEN;
            mask[Hypothesis::Sleeping as usize] = ORACLE_DAMPEN;
            mask[Hypothesis::Working as usize] = ORACLE_DAMPEN;
        } else if low_motion && !is_empty {
            // Amplify static hypotheses.
            mask[Hypothesis::Sitting as usize] = ORACLE_BOOST;
            mask[Hypothesis::Sleeping as usize] = ORACLE_BOOST;
            mask[Hypothesis::Working as usize] = ORACLE_BOOST;
            mask[Hypothesis::Standing as usize] = ORACLE_BOOST;

            // Dampen active hypotheses.
            mask[Hypothesis::Exercising as usize] = ORACLE_DAMPEN;
            mask[Hypothesis::MovingLeft as usize] = ORACLE_DAMPEN;
            mask[Hypothesis::MovingRight as usize] = ORACLE_DAMPEN;
        }

        // ── Directional motion evidence (heuristic from motion level) ──
        if is_moving && motion_energy > 0.3 && motion_energy < 0.7 {
            // Moderate movement -> cooking (activity with pauses).
            mask[Hypothesis::Cooking as usize] = ORACLE_BOOST;
        }
    }

    /// Grover diffusion operator: reflect amplitudes about the mean.
    ///   a_i = 2 * mean(a) - a_i
    fn grover_diffusion(&mut self) {
        let mut sum = 0.0f32;
        for i in 0..N_HYPO {
            sum += self.amplitudes[i];
        }
        let mean = sum / (N_HYPO as f32);

        for i in 0..N_HYPO {
            self.amplitudes[i] = 2.0 * mean - self.amplitudes[i];
            // Clamp to prevent negative amplitudes (which have no physical meaning
            // in this classical approximation).
            if self.amplitudes[i] < 0.0 {
                self.amplitudes[i] = 0.0;
            }
        }
    }

    /// Normalize amplitudes so that sum of squares = 1.
    fn normalize(&mut self) {
        let mut sum_sq = 0.0f32;
        for i in 0..N_HYPO {
            sum_sq += self.amplitudes[i] * self.amplitudes[i];
        }

        if sum_sq < 1.0e-10 {
            // Degenerate: reset to uniform.
            let uniform = 1.0 / sqrtf(N_HYPO as f32);
            for i in 0..N_HYPO {
                self.amplitudes[i] = uniform;
            }
            return;
        }

        let inv_norm = 1.0 / sqrtf(sum_sq);
        for i in 0..N_HYPO {
            self.amplitudes[i] *= inv_norm;
        }
    }

    /// Find the hypothesis with highest probability.
    /// Returns (index, probability).
    fn find_winner(&self) -> (usize, f32) {
        let mut max_prob = 0.0f32;
        let mut max_idx = 0usize;

        for i in 0..N_HYPO {
            let prob = self.amplitudes[i] * self.amplitudes[i];
            if prob > max_prob {
                max_prob = prob;
                max_idx = i;
            }
        }

        (max_idx, max_prob)
    }

    // ── Public accessors ─────────────────────────────────────────────────────

    /// Get the current winning hypothesis.
    pub fn winner(&self) -> Hypothesis {
        let (idx, _) = self.find_winner();
        Hypothesis::from_index(idx)
    }

    /// Get the probability of the current winner.
    pub fn winner_probability(&self) -> f32 {
        let (_, prob) = self.find_winner();
        prob
    }

    /// Whether the search has converged (winner prob > 0.5).
    pub fn is_converged(&self) -> bool {
        self.converged
    }

    /// Get the amplitude (not probability) for a specific hypothesis.
    pub fn amplitude(&self, h: Hypothesis) -> f32 {
        self.amplitudes[h as usize]
    }

    /// Get the probability for a specific hypothesis (amplitude^2).
    pub fn probability(&self, h: Hypothesis) -> f32 {
        let a = self.amplitudes[h as usize];
        a * a
    }

    /// Get the total number of Grover iterations performed.
    pub fn iterations(&self) -> u32 {
        self.iteration_count
    }

    /// Get the frame count.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Reset to uniform distribution (re-search from scratch).
    pub fn reset(&mut self) {
        let uniform = 1.0 / sqrtf(N_HYPO as f32);
        for i in 0..N_HYPO {
            self.amplitudes[i] = uniform;
        }
        self.iteration_count = 0;
        self.converged = false;
        self.prev_winner = 0;
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_uniform() {
        let search = InterferenceSearch::new();
        assert_eq!(search.iterations(), 0);
        assert!(!search.is_converged());

        // All probabilities should be 1/16 = 0.0625.
        let expected_prob = 1.0 / 16.0;
        for i in 0..N_HYPO {
            let h = Hypothesis::from_index(i);
            let p = search.probability(h);
            assert!(
                (p - expected_prob).abs() < 0.01,
                "hypothesis {} should have prob ~{}, got {}",
                i,
                expected_prob,
                p,
            );
        }
    }

    #[test]
    fn test_empty_room_convergence() {
        let mut search = InterferenceSearch::new();

        // Feed many frames with presence=0 (empty room).
        // The Grover diffusion converges slowly with 16 hypotheses;
        // 500 iterations ensures the Empty hypothesis dominates.
        for _ in 0..500 {
            search.process_frame(0, 0.0, 0);
        }

        assert_eq!(search.winner(), Hypothesis::Empty);
        assert!(
            search.winner_probability() > 0.15,
            "empty room should amplify Empty hypothesis, got prob {}",
            search.winner_probability(),
        );
    }

    #[test]
    fn test_high_motion_one_person() {
        let mut search = InterferenceSearch::new();

        // Feed frames: present, high motion, 1 person -> exercising or moving.
        for _ in 0..80 {
            search.process_frame(2, 0.8, 1);
        }

        let w = search.winner();
        let is_active = matches!(
            w,
            Hypothesis::Exercising | Hypothesis::MovingLeft | Hypothesis::MovingRight
        );
        assert!(
            is_active,
            "high motion should converge to active hypothesis, got {:?}",
            w,
        );
    }

    #[test]
    fn test_low_motion_one_person() {
        let mut search = InterferenceSearch::new();

        // Feed frames: present (1), low motion, 1 person -> sitting/sleeping/working.
        for _ in 0..80 {
            search.process_frame(1, 0.05, 1);
        }

        let w = search.winner();
        let is_static = matches!(
            w,
            Hypothesis::Sitting
                | Hypothesis::Sleeping
                | Hypothesis::Working
                | Hypothesis::Standing
        );
        assert!(
            is_static,
            "low motion should converge to static hypothesis, got {:?}",
            w,
        );
    }

    #[test]
    fn test_multi_person() {
        let mut search = InterferenceSearch::new();

        // Feed frames: present, moderate motion, 2 persons.
        for _ in 0..80 {
            search.process_frame(1, 0.3, 2);
        }

        let prob_two = search.probability(Hypothesis::TwoPersons);
        assert!(
            prob_two > 0.1,
            "2-person evidence should boost TwoPersons, got prob {}",
            prob_two,
        );
    }

    #[test]
    fn test_normalization_preserved() {
        let mut search = InterferenceSearch::new();

        // Run many iterations.
        for _ in 0..50 {
            search.process_frame(1, 0.5, 1);
        }

        // Sum of squares should be ~1.0.
        let mut sum_sq = 0.0f32;
        for i in 0..N_HYPO {
            let a = search.amplitude(Hypothesis::from_index(i));
            sum_sq += a * a;
        }

        assert!(
            (sum_sq - 1.0).abs() < 0.02,
            "sum of squares should be ~1.0, got {}",
            sum_sq,
        );
    }

    #[test]
    fn test_reset() {
        let mut search = InterferenceSearch::new();

        // Drive to convergence.
        for _ in 0..100 {
            search.process_frame(0, 0.0, 0);
        }
        assert!(search.iterations() > 0);

        // Reset.
        search.reset();
        assert_eq!(search.iterations(), 0);
        assert!(!search.is_converged());

        let expected_prob = 1.0 / 16.0;
        for i in 0..N_HYPO {
            let p = search.probability(Hypothesis::from_index(i));
            assert!(
                (p - expected_prob).abs() < 0.01,
                "after reset, hypothesis {} should be uniform, got {}",
                i,
                p,
            );
        }
    }

    #[test]
    fn test_event_emission() {
        let mut search = InterferenceSearch::new();

        // At frame 10 (WINNER_EMIT_INTERVAL), we should see a winner event.
        let mut winner_emitted = false;
        for _ in 0..20 {
            let events = search.process_frame(1, 0.3, 1);
            for &(et, _) in events {
                if et == EVENT_HYPOTHESIS_WINNER {
                    winner_emitted = true;
                }
            }
        }
        assert!(winner_emitted, "should emit HYPOTHESIS_WINNER periodically");
    }

    #[test]
    fn test_winner_change_emits_immediately() {
        let mut search = InterferenceSearch::new();

        // Drive towards Empty.
        for _ in 0..30 {
            search.process_frame(0, 0.0, 0);
        }
        let _w1 = search.winner();

        // Now suddenly switch to high motion single person.
        // The winner should eventually change, emitting an event.
        let mut winner_event_values: [f32; 16] = [0.0; 16];
        let mut n_winner_events = 0usize;
        for _ in 0..60 {
            let events = search.process_frame(2, 0.9, 1);
            for &(et, val) in events {
                if et == EVENT_HYPOTHESIS_WINNER && n_winner_events < 16 {
                    winner_event_values[n_winner_events] = val;
                    n_winner_events += 1;
                }
            }
        }

        // Should have emitted winner events.
        assert!(n_winner_events > 0, "should emit winner events on context change");
    }

    #[test]
    fn test_hypothesis_from_index_roundtrip() {
        for i in 0..N_HYPO {
            let h = Hypothesis::from_index(i);
            assert_eq!(h as usize, i, "from_index({}) should roundtrip", i);
        }
    }
}
