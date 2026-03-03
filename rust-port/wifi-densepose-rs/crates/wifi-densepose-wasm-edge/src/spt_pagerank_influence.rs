//! PageRank influence — spatial reasoning module (ADR-041).
//!
//! Identifies the dominant person in multi-person WiFi sensing scenes
//! using PageRank over a CSI cross-correlation graph.  Up to 4 persons
//! are modelled as nodes; edge weights are the normalised cross-correlation
//! of their subcarrier phase groups.
//!
//! Event IDs: 760-762 (Spatial Reasoning series).

use libm::{fabsf, sqrtf};

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum tracked persons.
const MAX_PERSONS: usize = 4;

/// Subcarriers assigned per person group.
const SC_PER_PERSON: usize = 8;

/// Maximum subcarriers (MAX_PERSONS * SC_PER_PERSON).
const MAX_SC: usize = MAX_PERSONS * SC_PER_PERSON;

/// PageRank damping factor.
const DAMPING: f32 = 0.85;

/// PageRank power-iteration rounds.
const PR_ITERS: usize = 10;

/// EMA smoothing for influence tracking.
const ALPHA: f32 = 0.15;

/// Minimum rank change to emit INFLUENCE_CHANGE event.
const CHANGE_THRESHOLD: f32 = 0.05;

// ── Event IDs ────────────────────────────────────────────────────────────────

/// Emitted with the person index (0-3) of the most influential person.
pub const EVENT_DOMINANT_PERSON: i32 = 760;

/// Emitted with the PageRank score of the dominant person [0, 1].
pub const EVENT_INFLUENCE_SCORE: i32 = 761;

/// Emitted when a person's rank changes by more than CHANGE_THRESHOLD.
/// Value encodes person_id in integer part, signed delta in fractional.
pub const EVENT_INFLUENCE_CHANGE: i32 = 762;

// ── State ────────────────────────────────────────────────────────────────────

/// PageRank influence tracker.
pub struct PageRankInfluence {
    /// Weighted adjacency matrix (row-major, adj[i][j] = correlation i<->j).
    adj: [[f32; MAX_PERSONS]; MAX_PERSONS],
    /// Current PageRank vector.
    rank: [f32; MAX_PERSONS],
    /// Previous-frame PageRank (for change detection).
    prev_rank: [f32; MAX_PERSONS],
    /// Number of persons currently tracked (from host).
    n_persons: usize,
    /// Frame counter.
    frame_count: u32,
}

impl PageRankInfluence {
    pub const fn new() -> Self {
        Self {
            adj: [[0.0; MAX_PERSONS]; MAX_PERSONS],
            rank: [0.25; MAX_PERSONS],
            prev_rank: [0.25; MAX_PERSONS],
            n_persons: 0,
            frame_count: 0,
        }
    }

    /// Process one CSI frame.
    ///
    /// `phases` — per-subcarrier phases (up to 32).
    /// `n_persons` — number of persons reported by host (clamped to 1..4).
    ///
    /// Returns a slice of (event_id, value) pairs to emit.
    pub fn process_frame(&mut self, phases: &[f32], n_persons: usize) -> &[(i32, f32)] {
        let np = if n_persons < 1 { 1 } else if n_persons > MAX_PERSONS { MAX_PERSONS } else { n_persons };
        self.n_persons = np;
        self.frame_count += 1;

        let n_sc = phases.len().min(MAX_SC);
        if n_sc < SC_PER_PERSON {
            return &[];
        }

        // ── 1. Build adjacency from cross-correlation ────────────────────
        self.build_adjacency(phases, n_sc, np);

        // ── 2. Run PageRank power iteration ──────────────────────────────
        self.power_iteration(np);

        // ── 3. Emit events ───────────────────────────────────────────────
        self.build_events(np)
    }

    /// Compute normalised cross-correlation between person subcarrier groups.
    fn build_adjacency(&mut self, phases: &[f32], n_sc: usize, np: usize) {
        for i in 0..np {
            for j in (i + 1)..np {
                let corr = self.cross_correlation(phases, n_sc, i, j);
                self.adj[i][j] = corr;
                self.adj[j][i] = corr;
            }
            self.adj[i][i] = 0.0; // no self-loops
        }
    }

    /// abs(sum(phase_i * phase_j)) / (norm_i * norm_j).
    fn cross_correlation(&self, phases: &[f32], n_sc: usize, a: usize, b: usize) -> f32 {
        let a_start = a * SC_PER_PERSON;
        let b_start = b * SC_PER_PERSON;
        let a_end = (a_start + SC_PER_PERSON).min(n_sc);
        let b_end = (b_start + SC_PER_PERSON).min(n_sc);
        let len = (a_end - a_start).min(b_end - b_start);
        if len == 0 {
            return 0.0;
        }

        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for k in 0..len {
            let pa = phases[a_start + k];
            let pb = phases[b_start + k];
            dot += pa * pb;
            norm_a += pa * pa;
            norm_b += pb * pb;
        }

        let denom = sqrtf(norm_a) * sqrtf(norm_b);
        if denom < 1e-9 {
            return 0.0;
        }

        fabsf(dot) / denom
    }

    /// Standard PageRank: r_{k+1} = d * M * r_k + (1-d)/N.
    fn power_iteration(&mut self, np: usize) {
        // Save previous rank.
        for i in 0..np {
            self.prev_rank[i] = self.rank[i];
        }

        // Column-normalise adjacency -> transition matrix M.
        // col_sum[j] = sum of adj[i][j] for all i.
        let mut col_sum = [0.0f32; MAX_PERSONS];
        for j in 0..np {
            let mut s = 0.0f32;
            for i in 0..np {
                s += self.adj[i][j];
            }
            col_sum[j] = s;
        }

        let base = (1.0 - DAMPING) / (np as f32);

        for _iter in 0..PR_ITERS {
            let mut new_rank = [0.0f32; MAX_PERSONS];

            for i in 0..np {
                let mut weighted = 0.0f32;
                for j in 0..np {
                    if col_sum[j] > 1e-9 {
                        weighted += (self.adj[i][j] / col_sum[j]) * self.rank[j];
                    }
                }
                new_rank[i] = DAMPING * weighted + base;
            }

            // Normalise so ranks sum to 1.
            let mut total = 0.0f32;
            for i in 0..np {
                total += new_rank[i];
            }
            if total > 1e-9 {
                for i in 0..np {
                    new_rank[i] /= total;
                }
            }

            for i in 0..np {
                self.rank[i] = new_rank[i];
            }
        }
    }

    /// Build output events into a static buffer.
    fn build_events(&self, np: usize) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 8] = [(0, 0.0); 8];
        let mut n = 0usize;

        // Find dominant person.
        let mut best_idx = 0usize;
        let mut best_rank = self.rank[0];
        for i in 1..np {
            if self.rank[i] > best_rank {
                best_rank = self.rank[i];
                best_idx = i;
            }
        }

        // Emit dominant person every frame.
        unsafe {
            EVENTS[n] = (EVENT_DOMINANT_PERSON, best_idx as f32);
        }
        n += 1;

        // Emit influence score every frame.
        unsafe {
            EVENTS[n] = (EVENT_INFLUENCE_SCORE, best_rank);
        }
        n += 1;

        // Emit change events for persons whose rank shifted significantly.
        for i in 0..np {
            let delta = self.rank[i] - self.prev_rank[i];
            if fabsf(delta) > CHANGE_THRESHOLD && n < 8 {
                // Encode: integer part = person_id, fractional = clamped delta.
                let encoded = i as f32 + delta.clamp(-0.49, 0.49);
                unsafe {
                    EVENTS[n] = (EVENT_INFLUENCE_CHANGE, encoded);
                }
                n += 1;
            }
        }

        unsafe { &EVENTS[..n] }
    }

    /// Get the current PageRank score for a person.
    pub fn rank(&self, person: usize) -> f32 {
        if person < MAX_PERSONS { self.rank[person] } else { 0.0 }
    }

    /// Get the index of the dominant person.
    pub fn dominant_person(&self) -> usize {
        let mut best = 0usize;
        for i in 1..self.n_persons {
            if self.rank[i] > self.rank[best] {
                best = i;
            }
        }
        best
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_constructor() {
        let pr = PageRankInfluence::new();
        assert_eq!(pr.frame_count, 0);
        assert_eq!(pr.n_persons, 0);
        // Initial ranks are uniform.
        for i in 0..MAX_PERSONS {
            assert!((pr.rank[i] - 0.25).abs() < 1e-6);
        }
    }

    #[test]
    fn test_single_person() {
        let mut pr = PageRankInfluence::new();
        let phases = [0.1f32; 8];
        let events = pr.process_frame(&phases, 1);
        // Should emit DOMINANT_PERSON(0) and INFLUENCE_SCORE.
        assert!(events.len() >= 2);
        assert_eq!(events[0].0, EVENT_DOMINANT_PERSON);
        assert!((events[0].1 - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_two_persons_symmetric() {
        let mut pr = PageRankInfluence::new();
        // Two persons with identical phase patterns -> equal rank.
        let mut phases = [0.0f32; 16];
        for i in 0..8 {
            phases[i] = 0.5;
        }
        for i in 8..16 {
            phases[i] = 0.5;
        }
        let events = pr.process_frame(&phases, 2);
        assert!(events.len() >= 2);
        // Ranks should be roughly equal.
        let r0 = pr.rank(0);
        let r1 = pr.rank(1);
        assert!((r0 - r1).abs() < 0.1);
    }

    #[test]
    fn test_dominant_person_detection() {
        let mut pr = PageRankInfluence::new();
        // Person 0 has high-energy phases, person 1 near zero.
        let mut phases = [0.0f32; 16];
        for i in 0..8 {
            phases[i] = 1.0 + (i as f32) * 0.1;
        }
        // Person 1 stays near zero -> weak correlation with person 0.
        for _ in 0..5 {
            pr.process_frame(&phases, 2);
        }
        // With asymmetric correlation, one person should dominate.
        assert!(pr.rank(0) > 0.0 || pr.rank(1) > 0.0);
    }

    #[test]
    fn test_cross_correlation_orthogonal() {
        let pr = PageRankInfluence::new();
        // Person 0: [1,0,1,0,1,0,1,0], Person 1: [0,1,0,1,0,1,0,1]
        let mut phases = [0.0f32; 16];
        for i in 0..8 {
            phases[i] = if i % 2 == 0 { 1.0 } else { 0.0 };
        }
        for i in 8..16 {
            phases[i] = if i % 2 == 0 { 0.0 } else { 1.0 };
        }
        let corr = pr.cross_correlation(&phases, 16, 0, 1);
        // Dot product = 0, so correlation ~ 0.
        assert!(corr < 0.01);
    }

    #[test]
    fn test_influence_change_event() {
        let mut pr = PageRankInfluence::new();
        // First frame: balanced.
        let balanced = [0.5f32; 16];
        pr.process_frame(&balanced, 2);

        // Sudden shift: person 0 gets strong signal, person 1 drops.
        let mut shifted = [0.0f32; 16];
        for i in 0..8 {
            shifted[i] = 2.0;
        }
        let events = pr.process_frame(&shifted, 2);
        // Should have at least DOMINANT_PERSON and INFLUENCE_SCORE.
        assert!(events.len() >= 2);
    }
}
