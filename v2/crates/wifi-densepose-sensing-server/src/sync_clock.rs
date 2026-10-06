//! Per-node clock quality for the ADR-138 gate, derived from ADR-110 sync
//! packets (issue #2156).
//!
//! The engine used to feed every node into the array coordinator with a
//! hard-coded `ClockQualityScore { offset_stdev_us: 50.0, valid: true }`, so a
//! node that had never sent a sync packet looked well synchronised. This
//! tracker builds the score from what the node actually reported:
//!
//! - no sync packet yet, or the newest one has `is_valid` cleared: `valid =
//!   false`, which the gate rejects as `ClockInvalid`;
//! - a valid sync: the offset dispersion is an EMA of the jump in
//!   `local_us - epoch_us` between consecutive valid packets, never below the
//!   ADR-110 measured follower stdev;
//! - as the newest sync ages past [`SYNC_FRESH_US`], the dispersion grows
//!   linearly so the score reaches 0 at [`SYNC_STALE_US`], the same 9 s
//!   ceiling the server's mesh-time recovery and the ADR-138 gate use.
//!
//! The floor means a node can never score better than ADR-110 measured, so
//! sync evidence can lower the score but not push it above that.

use std::time::Instant;

use wifi_densepose_engine::{ClockQualityScore, UNKNOWN_CLOCK};
use wifi_densepose_hardware::SyncPacket;

/// Follower offset stdev measured in ADR-110 §A0.10 (~104 µs). The firmware
/// reports an EMA-smoothed offset, so jumps between packets understate the
/// residual error against the leader; this is the lowest dispersion we accept.
const MEASURED_FLOOR_STDEV_US: f64 = 104.0;
/// A sync younger than this gets no age penalty (firmware emits ~0.5 Hz).
const SYNC_FRESH_US: u64 = 2_000_000;
/// Age at which the score reaches 0. Matches the 9 s staleness ceiling in
/// `NodeState::mesh_aligned_us` and `ClockQualityGate::default_params`.
const SYNC_STALE_US: u64 = 9_000_000;
/// Dispersion at which `ClockQualityScore::quality` is 0 and the default gate
/// rejects as dispersed (5 x its 200 µs monitor floor).
const DISPERSED_STDEV_US: f64 = 1_000.0;
/// Weight of the newest offset jump in the dispersion EMA.
const JITTER_EMA_ALPHA: f64 = 0.25;

/// Sync-derived clock state for one node.
#[derive(Debug, Clone, Default)]
pub(crate) struct SyncClockTracker {
    /// Offset (`local_us - epoch_us`) and arrival time of the newest valid
    /// sync packet. Cleared when a packet arrives with `is_valid` unset.
    last_valid: Option<(i64, Instant)>,
    /// EMA of absolute offset jumps between consecutive valid packets (µs).
    jitter_ema_us: Option<f64>,
}

impl SyncClockTracker {
    /// Fold one sync packet in. An invalid packet drops all sync evidence, so a
    /// re-acquired sync starts again from the measured floor.
    pub(crate) fn observe(&mut self, pkt: &SyncPacket, now: Instant) {
        if !pkt.flags.is_valid {
            *self = Self::default();
            return;
        }
        let offset = pkt.local_minus_epoch_us();
        if let Some((prev, _)) = self.last_valid {
            let jump = offset.abs_diff(prev) as f64;
            self.jitter_ema_us = Some(match self.jitter_ema_us {
                Some(ema) => ema + JITTER_EMA_ALPHA * (jump - ema),
                None => jump,
            });
        }
        self.last_valid = Some((offset, now));
    }

    /// Clock quality at `now` for the ADR-138 gate.
    pub(crate) fn score(&self, now: Instant) -> ClockQualityScore {
        let Some((_, seen_at)) = self.last_valid else {
            return UNKNOWN_CLOCK;
        };
        let age_us = u64::try_from(now.saturating_duration_since(seen_at).as_micros())
            .unwrap_or(u64::MAX);
        let jitter = self.jitter_ema_us.unwrap_or(0.0).max(MEASURED_FLOOR_STDEV_US);
        let stdev = (jitter + age_penalty_us(age_us)).min(DISPERSED_STDEV_US);
        ClockQualityScore { offset_stdev_us: stdev as f32, age_us, valid: true }
    }
}

/// Extra dispersion for an ageing sync: 0 up to [`SYNC_FRESH_US`], then linear
/// up to [`DISPERSED_STDEV_US`] at [`SYNC_STALE_US`].
fn age_penalty_us(age_us: u64) -> f64 {
    if age_us <= SYNC_FRESH_US {
        return 0.0;
    }
    let span = (SYNC_STALE_US - SYNC_FRESH_US) as f64;
    let frac = ((age_us - SYNC_FRESH_US) as f64 / span).min(1.0);
    frac * DISPERSED_STDEV_US
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wifi_densepose_hardware::SyncPacketFlags;

    /// Monitor floor of `ClockQualityGate::default_params`.
    const GATE_FLOOR_US: f32 = 200.0;
    /// Quality the old hard-coded score produced (50 µs against the floor).
    const OLD_HARDCODED_QUALITY: f32 = 0.95;

    fn pkt(offset_us: i64, valid: bool) -> SyncPacket {
        let epoch_us = 10_000_000u64;
        SyncPacket {
            node_id: 3,
            proto_ver: 1,
            flags: SyncPacketFlags { is_leader: false, is_valid: valid, smoothed_used: true },
            local_us: (epoch_us as i64 + offset_us) as u64,
            epoch_us,
            sequence: 1,
        }
    }

    fn quality(s: &ClockQualityScore) -> f32 {
        s.quality(GATE_FLOOR_US)
    }

    #[test]
    fn no_sync_is_unknown() {
        let s = SyncClockTracker::default().score(Instant::now());
        assert!(!s.valid);
        assert_eq!(quality(&s), 0.0);
    }

    #[test]
    fn invalid_flag_is_unknown() {
        let t0 = Instant::now();
        let mut t = SyncClockTracker::default();
        t.observe(&pkt(500, false), t0);
        assert!(!t.score(t0).valid);
    }

    #[test]
    fn invalid_packet_drops_earlier_valid_sync() {
        let t0 = Instant::now();
        let mut t = SyncClockTracker::default();
        t.observe(&pkt(500, true), t0);
        assert!(t.score(t0).valid);
        t.observe(&pkt(500, false), t0 + Duration::from_secs(1));
        let s = t.score(t0 + Duration::from_secs(1));
        assert!(!s.valid);
        assert_eq!(quality(&s), 0.0);
    }

    #[test]
    fn fresh_valid_sync_is_high_but_capped_by_the_measured_floor() {
        let t0 = Instant::now();
        let mut t = SyncClockTracker::default();
        t.observe(&pkt(500, true), t0);
        t.observe(&pkt(500, true), t0 + Duration::from_secs(2));
        let s = t.score(t0 + Duration::from_millis(2_500));
        assert!(s.valid);
        assert_eq!(s.offset_stdev_us, MEASURED_FLOOR_STDEV_US as f32);
        let q = quality(&s);
        assert!(q > 0.85, "fresh, steady sync should score high, got {q}");
        assert!(q < OLD_HARDCODED_QUALITY, "never above the old hard-coded score, got {q}");
        assert!(s.offset_stdev_us < GATE_FLOOR_US, "admitted at full weight");
    }

    #[test]
    fn stale_sync_decays_to_zero() {
        let t0 = Instant::now();
        let mut t = SyncClockTracker::default();
        t.observe(&pkt(500, true), t0);
        let fresh = quality(&t.score(t0 + Duration::from_secs(1)));
        let ageing = quality(&t.score(t0 + Duration::from_secs(5)));
        let stale = t.score(t0 + Duration::from_secs(9));
        assert!(ageing < fresh, "{ageing} !< {fresh}");
        assert!(ageing > 0.0);
        assert_eq!(quality(&stale), 0.0);
        assert!(stale.valid, "staleness is reported through age, not validity");
        assert!(stale.age_us >= SYNC_STALE_US);
    }

    #[test]
    fn offset_jumps_lower_the_score() {
        let t0 = Instant::now();
        let mut steady = SyncClockTracker::default();
        let mut jumpy = SyncClockTracker::default();
        for (i, off) in [0i64, 400, -300, 500].into_iter().enumerate() {
            let at = t0 + Duration::from_secs(2 * i as u64);
            steady.observe(&pkt(100, true), at);
            jumpy.observe(&pkt(off, true), at);
        }
        let now = t0 + Duration::from_secs(6);
        let qs = quality(&steady.score(now));
        let qj = quality(&jumpy.score(now));
        assert!(qj < qs, "jumpy {qj} should score below steady {qs}");
        assert!(jumpy.score(now).offset_stdev_us > MEASURED_FLOOR_STDEV_US as f32);
    }

    #[test]
    fn unknown_clock_never_passes_the_gate_quality() {
        // `valid` is covered by no_sync_is_unknown; a constant assert would be
        // optimised out.
        assert_eq!(quality(&UNKNOWN_CLOCK), 0.0);
    }
}
