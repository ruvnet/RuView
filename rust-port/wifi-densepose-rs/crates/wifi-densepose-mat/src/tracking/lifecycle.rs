//! Track lifecycle state machine for survivor tracking.
//!
//! Manages the lifecycle of a tracked survivor:
//! Tentative → Active → Lost → Terminated (or Rescued)

/// Configuration for SurvivorTracker behaviour.
#[derive(Debug, Clone)]
pub struct TrackerConfig {
    /// Consecutive hits required to promote Tentative → Active (default: 2)
    pub birth_hits_required: u32,
    /// Consecutive misses to transition Active → Lost (default: 3)
    pub max_active_misses: u32,
    /// Seconds a Lost track is eligible for re-identification (default: 30.0)
    pub max_lost_age_secs: f64,
    /// Fingerprint distance threshold for re-identification (default: 0.35)
    pub reid_threshold: f32,
    /// Mahalanobis distance² gate for data association (default: 9.0 = 3σ in 3D)
    pub gate_mahalanobis_sq: f64,
    /// Kalman measurement noise variance σ²_obs in m² (default: 2.25 = 1.5m²)
    pub obs_noise_var: f64,
    /// Kalman process noise variance σ²_a in (m/s²)² (default: 0.01)
    pub process_noise_var: f64,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            birth_hits_required: 2,
            max_active_misses: 3,
            max_lost_age_secs: 30.0,
            reid_threshold: 0.35,
            gate_mahalanobis_sq: 9.0,
            obs_noise_var: 2.25,
            process_noise_var: 0.01,
        }
    }
}

/// Current lifecycle state of a tracked survivor.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackState {
    /// Newly detected; awaiting confirmation hits.
    Tentative {
        /// Number of consecutive matched observations received.
        hits: u32,
    },
    /// Confirmed active track; receiving regular observations.
    Active,
    /// Signal lost; Kalman predicts position; re-ID window open.
    Lost {
        /// Consecutive frames missed since going Lost.
        miss_count: u32,
        /// Instant when the track entered Lost state.
        lost_since: std::time::Instant,
    },
    /// Re-ID window expired or explicitly terminated. Cannot recover.
    Terminated,
    /// Operator confirmed rescue. Terminal state.
    Rescued,
}

/// Controls lifecycle transitions for a single track.
pub struct TrackLifecycle {
    state: TrackState,
    birth_hits_required: u32,
    max_active_misses: u32,
    max_lost_age_secs: f64,
    /// Consecutive misses while Active (resets on hit).
    active_miss_count: u32,
}

impl TrackLifecycle {
    /// Create a new lifecycle starting in Tentative { hits: 0 }.
    pub fn new(config: &TrackerConfig) -> Self {
        Self {
            state: TrackState::Tentative { hits: 0 },
            birth_hits_required: config.birth_hits_required,
            max_active_misses: config.max_active_misses,
            max_lost_age_secs: config.max_lost_age_secs,
            active_miss_count: 0,
        }
    }

    /// Register a matched observation this frame.
    ///
    /// - Tentative: increment hits; if hits >= birth_hits_required → Active
    /// - Active: reset active_miss_count
    /// - Lost: transition back to Active, reset miss_count
    pub fn hit(&mut self) {
        match &self.state {
            TrackState::Tentative { hits } => {
                let new_hits = hits + 1;
                if new_hits >= self.birth_hits_required {
                    self.state = TrackState::Active;
                    self.active_miss_count = 0;
                } else {
                    self.state = TrackState::Tentative { hits: new_hits };
                }
            }
            TrackState::Active => {
                self.active_miss_count = 0;
            }
            TrackState::Lost { .. } => {
                self.state = TrackState::Active;
                self.active_miss_count = 0;
            }
            // Terminal states: no transition
            TrackState::Terminated | TrackState::Rescued => {}
        }
    }

    /// Register a frame with no matching observation.
    ///
    /// - Tentative: → Terminated immediately (not enough evidence)
    /// - Active: increment active_miss_count; if >= max_active_misses → Lost
    /// - Lost: increment miss_count
    pub fn miss(&mut self) {
        match &self.state {
            TrackState::Tentative { .. } => {
                self.state = TrackState::Terminated;
            }
            TrackState::Active => {
                self.active_miss_count += 1;
                if self.active_miss_count >= self.max_active_misses {
                    self.state = TrackState::Lost {
                        miss_count: 0,
                        lost_since: std::time::Instant::now(),
                    };
                }
            }
            TrackState::Lost { miss_count, lost_since } => {
                let new_count = miss_count + 1;
                let since = *lost_since;
                self.state = TrackState::Lost {
                    miss_count: new_count,
                    lost_since: since,
                };
            }
            // Terminal states: no transition
            TrackState::Terminated | TrackState::Rescued => {}
        }
    }

    /// Operator marks survivor as rescued.
    pub fn rescue(&mut self) {
        self.state = TrackState::Rescued;
    }

    /// Called each tick to check if Lost track has expired.
    pub fn check_lost_expiry(&mut self, now: std::time::Instant, max_lost_age_secs: f64) {
        if let TrackState::Lost { lost_since, .. } = &self.state {
            let elapsed = now.duration_since(*lost_since).as_secs_f64();
            if elapsed > max_lost_age_secs {
                self.state = TrackState::Terminated;
            }
        }
    }

    /// Get the current state.
    pub fn state(&self) -> &TrackState {
        &self.state
    }

    /// True if track is Active or Tentative (should keep in active pool).
    pub fn is_active_or_tentative(&self) -> bool {
        matches!(self.state, TrackState::Active | TrackState::Tentative { .. })
    }

    /// True if track is in Lost state.
    pub fn is_lost(&self) -> bool {
        matches!(self.state, TrackState::Lost { .. })
    }

    /// True if track is Terminated or Rescued (remove from pool eventually).
    pub fn is_terminal(&self) -> bool {
        matches!(self.state, TrackState::Terminated | TrackState::Rescued)
    }

    /// True if a Lost track is still within re-ID window.
    pub fn can_reidentify(&self, now: std::time::Instant, max_lost_age_secs: f64) -> bool {
        if let TrackState::Lost { lost_since, .. } = &self.state {
            let elapsed = now.duration_since(*lost_since).as_secs_f64();
            elapsed <= max_lost_age_secs
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn default_lifecycle() -> TrackLifecycle {
        TrackLifecycle::new(&TrackerConfig::default())
    }

    #[test]
    fn test_tentative_confirmation() {
        // Default config: birth_hits_required = 2
        let mut lc = default_lifecycle();
        assert!(matches!(lc.state(), TrackState::Tentative { hits: 0 }));

        lc.hit();
        assert!(matches!(lc.state(), TrackState::Tentative { hits: 1 }));

        lc.hit();
        // 2 hits → Active
        assert!(matches!(lc.state(), TrackState::Active));
        assert!(lc.is_active_or_tentative());
        assert!(!lc.is_lost());
        assert!(!lc.is_terminal());
    }

    #[test]
    fn test_tentative_miss_terminates() {
        let mut lc = default_lifecycle();
        assert!(matches!(lc.state(), TrackState::Tentative { .. }));

        // 1 miss while Tentative → Terminated
        lc.miss();
        assert!(matches!(lc.state(), TrackState::Terminated));
        assert!(lc.is_terminal());
        assert!(!lc.is_active_or_tentative());
    }

    #[test]
    fn test_active_to_lost() {
        let mut lc = default_lifecycle();
        // Confirm the track first
        lc.hit();
        lc.hit();
        assert!(matches!(lc.state(), TrackState::Active));

        // Default: max_active_misses = 3
        lc.miss();
        assert!(matches!(lc.state(), TrackState::Active));
        lc.miss();
        assert!(matches!(lc.state(), TrackState::Active));
        lc.miss();
        // 3 misses → Lost
        assert!(lc.is_lost());
        assert!(!lc.is_active_or_tentative());
    }

    #[test]
    fn test_lost_to_active_via_hit() {
        let mut lc = default_lifecycle();
        lc.hit();
        lc.hit();
        // Drive to Lost
        lc.miss();
        lc.miss();
        lc.miss();
        assert!(lc.is_lost());

        // Hit while Lost → Active
        lc.hit();
        assert!(matches!(lc.state(), TrackState::Active));
        assert!(lc.is_active_or_tentative());
    }

    #[test]
    fn test_lost_expiry() {
        let mut lc = default_lifecycle();
        lc.hit();
        lc.hit();
        lc.miss();
        lc.miss();
        lc.miss();
        assert!(lc.is_lost());

        // Simulate expiry: use an Instant far in the past for lost_since
        // by calling check_lost_expiry with a "now" that is 31 seconds ahead
        // We need to get the lost_since from the state and fake expiry.
        // Since Instant is opaque, we call check_lost_expiry with a now
        // that is at least max_lost_age_secs after lost_since.
        // We achieve this by sleeping briefly then using a future-shifted now.
        let future_now = Instant::now() + Duration::from_secs(31);
        lc.check_lost_expiry(future_now, 30.0);
        assert!(matches!(lc.state(), TrackState::Terminated));
        assert!(lc.is_terminal());
    }

    #[test]
    fn test_rescue() {
        let mut lc = default_lifecycle();
        lc.hit();
        lc.hit();
        assert!(matches!(lc.state(), TrackState::Active));

        lc.rescue();
        assert!(matches!(lc.state(), TrackState::Rescued));
        assert!(lc.is_terminal());
    }
}
