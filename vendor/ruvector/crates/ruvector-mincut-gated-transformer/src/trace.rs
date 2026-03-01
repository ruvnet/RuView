//! Tracing and diagnostics for gate decisions.
//!
//! This module is only available with the `trace` feature.

extern crate alloc;
use alloc::vec::Vec;

use crate::packets::{GateDecision, GateReason, Witness};

/// Rolling buffer size for trace history.
pub const TRACE_BUFFER_SIZE: usize = 64;

/// Trace counters for statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct TraceCounters {
    /// Total inference calls
    pub calls: u64,

    /// Allow decisions
    pub allow: u64,

    /// ReduceScope decisions
    pub reduce_scope: u64,

    /// FlushKv decisions
    pub flush_kv: u64,

    /// FreezeWrites decisions
    pub freeze_writes: u64,

    /// QuarantineUpdates decisions
    pub quarantine: u64,

    /// Skipped inferences
    pub skipped: u64,
}

impl TraceCounters {
    /// Increment counter for a decision
    pub fn record(&mut self, decision: GateDecision, skipped: bool) {
        self.calls += 1;

        if skipped {
            self.skipped += 1;
            return;
        }

        match decision {
            GateDecision::Allow => self.allow += 1,
            GateDecision::ReduceScope => self.reduce_scope += 1,
            GateDecision::FlushKv => self.flush_kv += 1,
            GateDecision::FreezeWrites => self.freeze_writes += 1,
            GateDecision::QuarantineUpdates => self.quarantine += 1,
        }
    }

    /// Get intervention rate (non-Allow decisions / total)
    pub fn intervention_rate(&self) -> f64 {
        if self.calls == 0 {
            return 0.0;
        }
        let interventions =
            self.reduce_scope + self.flush_kv + self.freeze_writes + self.quarantine;
        interventions as f64 / (self.calls - self.skipped) as f64
    }

    /// Get skip rate
    pub fn skip_rate(&self) -> f64 {
        if self.calls == 0 {
            return 0.0;
        }
        self.skipped as f64 / self.calls as f64
    }
}

/// Snapshot of recent trace history.
#[derive(Clone, Debug)]
pub struct TraceSnapshot {
    /// Last N decisions
    pub last_decisions: [GateDecision; TRACE_BUFFER_SIZE],

    /// Last N reasons
    pub last_reasons: [GateReason; TRACE_BUFFER_SIZE],

    /// Last N lambda values
    pub last_lambda: [u32; TRACE_BUFFER_SIZE],

    /// Last N tiers
    pub last_tier: [u8; TRACE_BUFFER_SIZE],

    /// Aggregate counters
    pub counters: TraceCounters,

    /// Current write index
    pub write_index: usize,

    /// Number of valid entries
    pub valid_entries: usize,
}

impl Default for TraceSnapshot {
    fn default() -> Self {
        Self {
            last_decisions: [GateDecision::Allow; TRACE_BUFFER_SIZE],
            last_reasons: [GateReason::None; TRACE_BUFFER_SIZE],
            last_lambda: [0; TRACE_BUFFER_SIZE],
            last_tier: [0; TRACE_BUFFER_SIZE],
            counters: TraceCounters::default(),
            write_index: 0,
            valid_entries: 0,
        }
    }
}

impl TraceSnapshot {
    /// Get the most recent N entries (up to valid_entries)
    pub fn recent(
        &self,
        n: usize,
    ) -> impl Iterator<Item = (GateDecision, GateReason, u32, u8)> + '_ {
        let n = n.min(self.valid_entries);
        let start = if self.valid_entries >= TRACE_BUFFER_SIZE {
            self.write_index
        } else {
            0
        };

        (0..n).map(move |i| {
            let idx = (start + self.valid_entries - n + i) % TRACE_BUFFER_SIZE;
            (
                self.last_decisions[idx],
                self.last_reasons[idx],
                self.last_lambda[idx],
                self.last_tier[idx],
            )
        })
    }

    /// Check if recent history shows instability
    pub fn is_unstable(&self, window: usize, threshold: usize) -> bool {
        let window = window.min(self.valid_entries);
        let interventions = self
            .recent(window)
            .filter(|(d, _, _, _)| d.is_intervention())
            .count();
        interventions >= threshold
    }

    /// Get lambda trend over recent history
    pub fn lambda_trend(&self, window: usize) -> LambdaTrend {
        let window = window.min(self.valid_entries);
        if window < 2 {
            return LambdaTrend::Stable;
        }

        let values: Vec<u32> = self.recent(window).map(|(_, _, l, _)| l).collect();

        // Simple linear trend
        let first_half_avg: f64 =
            values[..window / 2].iter().map(|&x| x as f64).sum::<f64>() / (window / 2) as f64;
        let second_half_avg: f64 = values[window / 2..].iter().map(|&x| x as f64).sum::<f64>()
            / (window - window / 2) as f64;

        let change = (second_half_avg - first_half_avg) / first_half_avg.max(1.0);

        if change > 0.1 {
            LambdaTrend::Increasing
        } else if change < -0.1 {
            LambdaTrend::Decreasing
        } else {
            LambdaTrend::Stable
        }
    }
}

/// Lambda trend direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LambdaTrend {
    /// Lambda is increasing (coherence improving)
    Increasing,
    /// Lambda is stable
    Stable,
    /// Lambda is decreasing (coherence degrading)
    Decreasing,
}

/// Trace state for recording inference history.
pub struct TraceState {
    /// Rolling buffer of decisions
    decisions: [GateDecision; TRACE_BUFFER_SIZE],

    /// Rolling buffer of reasons
    reasons: [GateReason; TRACE_BUFFER_SIZE],

    /// Rolling buffer of lambda values
    lambdas: [u32; TRACE_BUFFER_SIZE],

    /// Rolling buffer of tiers
    tiers: [u8; TRACE_BUFFER_SIZE],

    /// Current write index
    write_index: usize,

    /// Number of valid entries
    valid_entries: usize,

    /// Aggregate counters
    counters: TraceCounters,
}

impl TraceState {
    /// Create new trace state.
    pub fn new() -> Self {
        Self {
            decisions: [GateDecision::Allow; TRACE_BUFFER_SIZE],
            reasons: [GateReason::None; TRACE_BUFFER_SIZE],
            lambdas: [0; TRACE_BUFFER_SIZE],
            tiers: [0; TRACE_BUFFER_SIZE],
            write_index: 0,
            valid_entries: 0,
            counters: TraceCounters::default(),
        }
    }

    /// Record a witness.
    pub fn record(&mut self, witness: &Witness) {
        self.decisions[self.write_index] = witness.decision;
        self.reasons[self.write_index] = witness.reason;
        self.lambdas[self.write_index] = witness.lambda;

        // Determine tier from effective parameters
        let tier = if witness.effective_seq_len == 0 {
            3
        } else if witness.decision == GateDecision::FreezeWrites
            || witness.decision == GateDecision::QuarantineUpdates
        {
            2
        } else if witness.decision == GateDecision::ReduceScope
            || witness.decision == GateDecision::FlushKv
        {
            1
        } else {
            0
        };
        self.tiers[self.write_index] = tier;

        // Update counters
        let skipped = witness.effective_seq_len == 0;
        self.counters.record(witness.decision, skipped);

        // Advance write index
        self.write_index = (self.write_index + 1) % TRACE_BUFFER_SIZE;
        if self.valid_entries < TRACE_BUFFER_SIZE {
            self.valid_entries += 1;
        }
    }

    /// Get current snapshot.
    pub fn snapshot(&self) -> TraceSnapshot {
        TraceSnapshot {
            last_decisions: self.decisions,
            last_reasons: self.reasons,
            last_lambda: self.lambdas,
            last_tier: self.tiers,
            counters: self.counters,
            write_index: self.write_index,
            valid_entries: self.valid_entries,
        }
    }

    /// Reset trace state.
    pub fn reset(&mut self) {
        self.write_index = 0;
        self.valid_entries = 0;
        self.counters = TraceCounters::default();
    }

    /// Get counters.
    pub fn counters(&self) -> &TraceCounters {
        &self.counters
    }
}

impl Default for TraceState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::GatePacket;
    use alloc::vec::Vec;

    #[test]
    fn test_trace_counters() {
        let mut counters = TraceCounters::default();

        counters.record(GateDecision::Allow, false);
        counters.record(GateDecision::ReduceScope, false);
        counters.record(GateDecision::Allow, true); // skipped

        assert_eq!(counters.calls, 3);
        assert_eq!(counters.allow, 1);
        assert_eq!(counters.reduce_scope, 1);
        assert_eq!(counters.skipped, 1);
    }

    #[test]
    fn test_intervention_rate() {
        let mut counters = TraceCounters::default();

        for _ in 0..8 {
            counters.record(GateDecision::Allow, false);
        }
        for _ in 0..2 {
            counters.record(GateDecision::ReduceScope, false);
        }

        let rate = counters.intervention_rate();
        assert!((rate - 0.2).abs() < 0.01);
    }

    #[test]
    fn test_trace_state() {
        let mut state = TraceState::new();

        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 95,
            ..Default::default()
        };

        let witness = Witness::allow(&gate, 64, 16);
        state.record(&witness);

        let snapshot = state.snapshot();
        assert_eq!(snapshot.valid_entries, 1);
        assert_eq!(snapshot.counters.allow, 1);
    }

    #[test]
    fn test_trace_snapshot_recent() {
        let mut state = TraceState::new();

        for i in 0..10 {
            let gate = GatePacket {
                lambda: 100 + i,
                lambda_prev: 95,
                ..Default::default()
            };
            let witness = Witness::allow(&gate, 64, 16);
            state.record(&witness);
        }

        let snapshot = state.snapshot();
        let recent: Vec<_> = snapshot.recent(5).collect();

        assert_eq!(recent.len(), 5);
        // Most recent should have lambda 109 (100 + 9)
        assert_eq!(recent[4].2, 109);
    }

    #[test]
    fn test_instability_detection() {
        let mut state = TraceState::new();

        // Record alternating stable and unstable
        for i in 0..10 {
            let gate = GatePacket {
                lambda: 100,
                ..Default::default()
            };

            let witness = if i % 2 == 0 {
                Witness::allow(&gate, 64, 16)
            } else {
                Witness::intervention(
                    GateDecision::ReduceScope,
                    GateReason::BoundarySpike,
                    &gate,
                    32,
                    8,
                )
            };
            state.record(&witness);
        }

        let snapshot = state.snapshot();
        // 5 interventions out of 10 in last 10 should be unstable with threshold 4
        assert!(snapshot.is_unstable(10, 4));
    }

    #[test]
    fn test_lambda_trend() {
        let mut state = TraceState::new();

        // Decreasing lambda trend
        for i in 0..10 {
            let gate = GatePacket {
                lambda: 100 - i * 5,
                ..Default::default()
            };
            let witness = Witness::allow(&gate, 64, 16);
            state.record(&witness);
        }

        let snapshot = state.snapshot();
        assert_eq!(snapshot.lambda_trend(10), LambdaTrend::Decreasing);
    }
}
