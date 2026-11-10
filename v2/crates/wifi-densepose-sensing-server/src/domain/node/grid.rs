//! Subcarrier grid voting, admission gating, and calibration candidate selection.
//!
//! Complies with ELMAS.md: <= 350 lines, guard clauses, pure logic.

use std::time::Instant;

use crate::domain::csi_grid::{CalibrationGridEvidence, CsiGridKey, RawGridObservation};
use super::constants::*;
use super::state::NodeState;

impl NodeState {
    /// Ingest a header-only grid observation into the evidence buffer.
    pub fn observe_raw_grid(&mut self, grid: CsiGridKey, now: Instant) {
        while self
            .raw_grid_observations
            .front()
            .is_some_and(|s| now.saturating_duration_since(s.observed_at) > CALIBRATION_GRID_EVIDENCE_WINDOW)
        {
            self.raw_grid_observations.pop_front();
        }
        if self.raw_grid_observations.len() == CALIBRATION_GRID_EVIDENCE_CAPACITY {
            self.raw_grid_observations.pop_front();
        }
        self.raw_grid_observations.push_back(RawGridObservation {
            grid,
            observed_at: now,
        });
    }

    /// Group raw grid observations and compute span, rate, and gap metrics.
    pub fn calibration_grid_candidates(
        &self,
        now: Instant,
    ) -> Vec<(CsiGridKey, CalibrationGridEvidence)> {
        let mut grouped: Vec<(CsiGridKey, Vec<Instant>)> = Vec::new();
        for sample in self.raw_grid_observations.iter().filter(|s| {
            now.saturating_duration_since(s.observed_at) <= CALIBRATION_GRID_EVIDENCE_WINDOW
        }) {
            if let Some((_, arrivals)) = grouped
                .iter_mut()
                .find(|(candidate, _)| *candidate == sample.grid)
            {
                arrivals.push(sample.observed_at);
            } else {
                grouped.push((sample.grid, vec![sample.observed_at]));
            }
        }

        grouped
            .into_iter()
            .filter_map(|(grid, arrivals)| {
                let first = *arrivals.first()?;
                let last = *arrivals.last()?;
                let span_s = last.saturating_duration_since(first).as_secs_f64();
                let latest_age_s = now.saturating_duration_since(last).as_secs_f64();
                let mut max_gap_s = latest_age_s;
                for pair in arrivals.windows(2) {
                    let gap = pair[1].saturating_duration_since(pair[0]).as_secs_f64();
                    max_gap_s = max_gap_s.max(gap);
                }
                let rate_hz = if span_s > 0.0 {
                    arrivals.len().saturating_sub(1) as f64 / span_s
                } else {
                    0.0
                };
                Some((
                    grid,
                    CalibrationGridEvidence {
                        sample_count: arrivals.len(),
                        span_s,
                        rate_hz,
                        max_gap_s,
                        latest_age_s,
                    },
                ))
            })
            .collect()
    }

    /// Select the highest-density stable calibration grid candidate.
    pub fn select_calibration_grid(
        &self,
        now: Instant,
    ) -> Option<(CsiGridKey, CalibrationGridEvidence)> {
        self.calibration_grid_candidates(now)
            .into_iter()
            .filter(|(grid, evidence)| {
                grid.ppdu_type <= 3
                    && evidence.span_s >= CALIBRATION_GRID_MIN_SPAN_S
                    && evidence.rate_hz >= CALIBRATION_GRID_MIN_RATE_HZ
                    && evidence.max_gap_s < CALIBRATION_GRID_MAX_GAP_S
                    && evidence.latest_age_s < CALIBRATION_GRID_MAX_GAP_S
            })
            .min_by(|(left_grid, left), (right_grid, right)| {
                right_grid
                    .n_subcarriers
                    .cmp(&left_grid.n_subcarriers)
                    .then_with(|| left.max_gap_s.total_cmp(&right.max_gap_s))
                    .then_with(|| right.rate_hz.total_cmp(&left.rate_hz))
                    .then_with(|| left_grid.ppdu_type.cmp(&right_grid.ppdu_type))
            })
    }

    /// Clear field model frame history and state.
    pub fn clear_field_model_history(&mut self) {
        self.field_model_history.clear();
        self.field_model_latest_sequence = None;
        self.field_model_latest_seen = None;
    }

    /// Push an amplitude frame into the dedicated field model history buffer.
    pub fn push_field_model_frame(
        &mut self,
        sequence: u32,
        amplitudes: &[f64],
        now: Instant,
    ) {
        self.field_model_history.push_back(amplitudes.to_vec());
        if self.field_model_history.len() > FRAME_HISTORY_CAPACITY {
            self.field_model_history.pop_front();
        }
        self.field_model_latest_sequence = Some(sequence);
        self.field_model_latest_seen = Some(now);
    }

    /// ADR-110 / issue #1005 grid gate: decide whether a frame on `grid` may enter.
    pub fn accept_grid(&mut self, grid: (u16, wifi_densepose_hardware::PpduType)) -> bool {
        if self.grid_votes.len() == GRID_VOTE_FRAMES {
            self.grid_votes.pop_front();
        }
        self.grid_votes.push_back(grid);

        let Some(active) = self.active_grid else {
            self.active_grid = Some(grid);
            return true;
        };

        if grid != active {
            let votes = |g| self.grid_votes.iter().filter(|&&v| v == g).count();
            if 2 * votes(grid) > 3 * votes(active) {
                self.active_grid = Some(grid);
                self.frame_history.clear();
                self.baseline_motion = 0.0;
                self.baseline_frames = 0;
                return true;
            }
        }

        grid == active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_densepose_hardware::PpduType;

    fn ht(n: u16) -> (u16, PpduType) {
        (n, PpduType::HtLegacy)
    }

    const S3_NODE_GRID_RUNS: &[(u16, usize)] = &[
        (192, 19), (128, 2), (192, 5), (128, 1), (192, 6), (306, 1), (192, 14),
        (128, 1), (192, 55), (306, 2), (192, 1), (306, 1), (128, 1), (192, 6),
        (188, 1), (128, 2), (192, 1), (306, 1), (192, 7), (306, 1), (192, 35),
        (128, 1), (192, 6), (306, 1), (192, 28), (306, 1), (192, 10), (128, 1),
        (192, 4), (306, 1), (192, 4), (306, 1), (192, 9), (306, 2), (192, 6),
        (128, 1), (192, 13), (306, 1), (192, 11), (306, 1), (192, 3), (128, 1),
        (192, 27), (128, 1), (192, 13), (128, 1), (192, 3), (306, 1), (192, 1),
        (306, 1), (192, 3),
    ];

    #[test]
    fn real_s3_interleaved_grids_keep_the_majority_grid() {
        let mut ns = NodeState::new();
        let (mut accepted, mut on_192, mut switches) = (0, 0, 0);
        for &(n, run) in S3_NODE_GRID_RUNS {
            for _ in 0..run {
                let before = ns.active_grid;
                if ns.accept_grid(ht(n)) {
                    accepted += 1;
                }
                if before.is_some() && ns.active_grid != before {
                    switches += 1;
                }
                on_192 += usize::from(n == 192);
            }
        }
        assert_eq!(ns.active_grid, Some(ht(192)));
        assert_eq!(switches, 0, "interleaved grids must not flap the lock");
        assert_eq!(accepted, on_192, "every 192-bin frame and nothing else");
        assert!(accepted * 10 > 320 * 8, "accepted {accepted} of 320");
    }

    #[test]
    fn two_to_one_interleave_does_not_flap() {
        let mut ns = NodeState::new();
        let mut accepted = 0;
        for i in 0..300 {
            let n = if i % 3 == 2 { 192 } else { 306 };
            accepted += usize::from(ns.accept_grid(ht(n)));
        }
        assert_eq!(ns.active_grid, Some(ht(306)));
        assert_eq!(accepted, 200);
    }

    #[test]
    fn permanent_drift_to_a_sparser_grid_relocks() {
        let mut ns = NodeState::new();
        for _ in 0..GRID_VOTE_FRAMES {
            assert!(ns.accept_grid(ht(128)));
        }
        ns.frame_history.push_back(vec![1.0; 128]);
        let mut first_accept = None;
        for i in 0..GRID_VOTE_FRAMES {
            if ns.accept_grid(ht(64)) && first_accept.is_none() {
                first_accept = Some(i);
                assert!(ns.frame_history.is_empty(), "relock must clear the window");
            }
        }
        let first = first_accept.expect("64-bin stream never re-locked");
        assert!(first < GRID_VOTE_FRAMES, "re-locked after {first} frames");
        assert_eq!(ns.active_grid, Some(ht(64)));
    }

    #[test]
    fn c6_he_majority_rejects_ht_minority() {
        let mut ns = NodeState::new();
        assert!(ns.accept_grid(ht(64)));
        for i in 0..200 {
            let he = (256, PpduType::HeSu);
            if i % 6 == 5 {
                ns.accept_grid(ht(64));
            } else {
                ns.accept_grid(he);
            }
        }
        assert_eq!(ns.active_grid, Some((256, PpduType::HeSu)));
        assert!(!ns.accept_grid(ht(64)));
    }
}

