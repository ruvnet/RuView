//! # Compute Ladder: Escalation Logic for Coherence-Gated Execution
//!
//! Implements the compute ladder from ADR-014, providing threshold-based escalation
//! from low-latency reflex operations to human-in-the-loop review.
//!
//! ## Design Principle
//!
//! > Most updates stay in low-latency reflex lane (<1ms); sustained/growing
//! > incoherence triggers escalation.
//!
//! The compute ladder is not about being smart - it's about knowing when to stop
//! and when to ask for help.
//!
//! ## Lanes
//!
//! | Lane | Name | Latency | Description |
//! |------|------|---------|-------------|
//! | 0 | Reflex | <1ms | Local residual updates, simple aggregates |
//! | 1 | Retrieval | ~10ms | Evidence fetching, lightweight reasoning |
//! | 2 | Heavy | ~100ms | Multi-step planning, spectral analysis |
//! | 3 | Human | async | Human escalation for sustained incoherence |

use serde::{Deserialize, Serialize};
use std::fmt;

/// Compute lanes for escalating complexity.
///
/// CRITICAL: Most updates stay in Lane 0 (Reflex).
/// Escalation only occurs on sustained/growing incoherence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ComputeLane {
    /// Lane 0: Local residual updates, simple aggregates (<1ms)
    /// THE DEFAULT - most updates stay here
    Reflex = 0,

    /// Lane 1: Evidence fetching, lightweight reasoning (~10ms)
    /// Triggered by: transient energy spike
    Retrieval = 1,

    /// Lane 2: Multi-step planning, spectral analysis (~100ms)
    /// Triggered by: sustained incoherence above threshold
    Heavy = 2,

    /// Lane 3: Human escalation for sustained incoherence
    /// Triggered by: persistent incoherence that automated systems cannot resolve
    Human = 3,
}

impl ComputeLane {
    /// Get the expected latency budget for this lane in microseconds.
    #[inline]
    pub const fn latency_budget_us(&self) -> u64 {
        match self {
            ComputeLane::Reflex => 1_000,     // 1ms
            ComputeLane::Retrieval => 10_000, // 10ms
            ComputeLane::Heavy => 100_000,    // 100ms
            ComputeLane::Human => u64::MAX,   // No limit (async)
        }
    }

    /// Get the expected latency budget for this lane in milliseconds.
    #[inline]
    pub const fn latency_budget_ms(&self) -> u64 {
        match self {
            ComputeLane::Reflex => 1,
            ComputeLane::Retrieval => 10,
            ComputeLane::Heavy => 100,
            ComputeLane::Human => u64::MAX,
        }
    }

    /// Whether this lane allows automatic action execution.
    ///
    /// Returns `false` only for Human lane, which requires explicit approval.
    #[inline]
    pub const fn allows_automatic_execution(&self) -> bool {
        !matches!(self, ComputeLane::Human)
    }

    /// Whether this lane is the default low-latency lane.
    #[inline]
    pub const fn is_reflex(&self) -> bool {
        matches!(self, ComputeLane::Reflex)
    }

    /// Whether this lane requires escalation (not reflex).
    #[inline]
    pub const fn is_escalated(&self) -> bool {
        !matches!(self, ComputeLane::Reflex)
    }

    /// Get the next escalation level, if any.
    pub const fn escalate(&self) -> Option<ComputeLane> {
        match self {
            ComputeLane::Reflex => Some(ComputeLane::Retrieval),
            ComputeLane::Retrieval => Some(ComputeLane::Heavy),
            ComputeLane::Heavy => Some(ComputeLane::Human),
            ComputeLane::Human => None,
        }
    }

    /// Get the previous de-escalation level, if any.
    pub const fn deescalate(&self) -> Option<ComputeLane> {
        match self {
            ComputeLane::Reflex => None,
            ComputeLane::Retrieval => Some(ComputeLane::Reflex),
            ComputeLane::Heavy => Some(ComputeLane::Retrieval),
            ComputeLane::Human => Some(ComputeLane::Heavy),
        }
    }

    /// Parse from u8 value.
    pub const fn from_u8(value: u8) -> Option<ComputeLane> {
        match value {
            0 => Some(ComputeLane::Reflex),
            1 => Some(ComputeLane::Retrieval),
            2 => Some(ComputeLane::Heavy),
            3 => Some(ComputeLane::Human),
            _ => None,
        }
    }

    /// Convert to u8 value.
    #[inline]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Get a human-readable name for this lane.
    pub const fn name(&self) -> &'static str {
        match self {
            ComputeLane::Reflex => "Reflex",
            ComputeLane::Retrieval => "Retrieval",
            ComputeLane::Heavy => "Heavy",
            ComputeLane::Human => "Human",
        }
    }

    /// Get a description of what triggers this lane.
    pub const fn trigger_description(&self) -> &'static str {
        match self {
            ComputeLane::Reflex => "Default lane - low energy, no trigger needed",
            ComputeLane::Retrieval => "Transient energy spike above reflex threshold",
            ComputeLane::Heavy => "Sustained incoherence above retrieval threshold",
            ComputeLane::Human => "Persistent incoherence exceeding all automatic thresholds",
        }
    }
}

impl Default for ComputeLane {
    fn default() -> Self {
        ComputeLane::Reflex
    }
}

impl fmt::Display for ComputeLane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Lane {} ({})", self.as_u8(), self.name())
    }
}

/// Threshold configuration for compute lane escalation.
///
/// These thresholds determine when energy levels trigger lane transitions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LaneThresholds {
    /// Energy threshold for Lane 0 (Reflex) - stay in reflex if below this
    pub reflex: f32,

    /// Energy threshold for Lane 1 (Retrieval) - escalate to retrieval if above reflex
    pub retrieval: f32,

    /// Energy threshold for Lane 2 (Heavy) - escalate to heavy if above retrieval
    pub heavy: f32,
}

impl LaneThresholds {
    /// Create thresholds with explicit values.
    pub const fn new(reflex: f32, retrieval: f32, heavy: f32) -> Self {
        Self {
            reflex,
            retrieval,
            heavy,
        }
    }

    /// Create conservative thresholds (prefer escalation).
    pub const fn conservative() -> Self {
        Self {
            reflex: 0.1,
            retrieval: 0.3,
            heavy: 0.6,
        }
    }

    /// Create aggressive thresholds (prefer staying in reflex).
    pub const fn aggressive() -> Self {
        Self {
            reflex: 0.5,
            retrieval: 0.8,
            heavy: 0.95,
        }
    }

    /// Validate that thresholds are properly ordered.
    pub fn validate(&self) -> Result<(), ThresholdError> {
        if self.reflex < 0.0 || self.reflex > 1.0 {
            return Err(ThresholdError::OutOfRange {
                name: "reflex",
                value: self.reflex,
            });
        }
        if self.retrieval < 0.0 || self.retrieval > 1.0 {
            return Err(ThresholdError::OutOfRange {
                name: "retrieval",
                value: self.retrieval,
            });
        }
        if self.heavy < 0.0 || self.heavy > 1.0 {
            return Err(ThresholdError::OutOfRange {
                name: "heavy",
                value: self.heavy,
            });
        }
        if self.reflex >= self.retrieval {
            return Err(ThresholdError::InvalidOrdering {
                lower: "reflex",
                upper: "retrieval",
            });
        }
        if self.retrieval >= self.heavy {
            return Err(ThresholdError::InvalidOrdering {
                lower: "retrieval",
                upper: "heavy",
            });
        }
        Ok(())
    }

    /// Determine which lane an energy level requires.
    ///
    /// Optimized with branchless comparison using conditional moves
    /// for better branch prediction on modern CPUs.
    #[inline]
    pub fn lane_for_energy(&self, energy: f32) -> ComputeLane {
        // Use branchless comparison for better performance
        // The compiler can convert this to conditional moves (CMOVcc)
        let is_above_reflex = (energy >= self.reflex) as u8;
        let is_above_retrieval = (energy >= self.retrieval) as u8;
        let is_above_heavy = (energy >= self.heavy) as u8;

        // Sum determines the lane: 0=Reflex, 1=Retrieval, 2=Heavy, 3=Human
        let lane_index = is_above_reflex + is_above_retrieval + is_above_heavy;

        // SAFETY: lane_index is guaranteed to be 0-3
        match lane_index {
            0 => ComputeLane::Reflex,
            1 => ComputeLane::Retrieval,
            2 => ComputeLane::Heavy,
            _ => ComputeLane::Human,
        }
    }

    /// Fast lane check using array lookup (alternative implementation)
    #[inline]
    pub fn lane_for_energy_lookup(&self, energy: f32) -> ComputeLane {
        // Store thresholds in array for potential SIMD comparison
        let thresholds = [self.reflex, self.retrieval, self.heavy];

        // Count how many thresholds are exceeded
        let mut lane = 0u8;
        for &t in &thresholds {
            lane += (energy >= t) as u8;
        }

        // SAFETY: lane is 0-3
        ComputeLane::from_u8(lane).unwrap_or(ComputeLane::Human)
    }

    /// Get the threshold for a specific lane transition.
    pub fn threshold_for_lane(&self, lane: ComputeLane) -> f32 {
        match lane {
            ComputeLane::Reflex => 0.0, // Always accessible
            ComputeLane::Retrieval => self.reflex,
            ComputeLane::Heavy => self.retrieval,
            ComputeLane::Human => self.heavy,
        }
    }
}

impl Default for LaneThresholds {
    fn default() -> Self {
        Self {
            reflex: 0.2,
            retrieval: 0.5,
            heavy: 0.8,
        }
    }
}

/// Error type for threshold validation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ThresholdError {
    #[error("Threshold '{name}' value {value} is out of range [0.0, 1.0]")]
    OutOfRange { name: &'static str, value: f32 },

    #[error("Invalid threshold ordering: {lower} must be less than {upper}")]
    InvalidOrdering {
        lower: &'static str,
        upper: &'static str,
    },
}

/// Escalation reason describing why a lane transition occurred.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EscalationReason {
    /// Energy exceeded threshold for current lane.
    EnergyThreshold {
        /// The measured energy level.
        energy: u32, // Fixed point (energy * 1000)
        /// The threshold that was exceeded.
        threshold: u32,
    },

    /// Persistent incoherence detected (energy above threshold for duration).
    PersistentIncoherence {
        /// Duration in milliseconds that energy was elevated.
        duration_ms: u64,
        /// Configured persistence window in milliseconds.
        window_ms: u64,
    },

    /// Growing incoherence trend detected.
    GrowingIncoherence {
        /// Energy growth rate per second.
        growth_rate: i32, // Fixed point (rate * 1000)
    },

    /// External trigger requested escalation.
    ExternalTrigger {
        /// Source of the trigger.
        source: String,
    },

    /// System override (e.g., maintenance mode).
    SystemOverride {
        /// Reason for override.
        reason: String,
    },
}

impl EscalationReason {
    /// Create an energy threshold escalation.
    pub fn energy(energy: f32, threshold: f32) -> Self {
        Self::EnergyThreshold {
            energy: (energy * 1000.0) as u32,
            threshold: (threshold * 1000.0) as u32,
        }
    }

    /// Create a persistent incoherence escalation.
    pub fn persistent(duration_ms: u64, window_ms: u64) -> Self {
        Self::PersistentIncoherence {
            duration_ms,
            window_ms,
        }
    }

    /// Create a growing incoherence escalation.
    pub fn growing(growth_rate: f32) -> Self {
        Self::GrowingIncoherence {
            growth_rate: (growth_rate * 1000.0) as i32,
        }
    }

    /// Is this a persistence-based escalation?
    pub fn is_persistence_based(&self) -> bool {
        matches!(self, Self::PersistentIncoherence { .. })
    }

    /// Is this an external trigger?
    pub fn is_external(&self) -> bool {
        matches!(
            self,
            Self::ExternalTrigger { .. } | Self::SystemOverride { .. }
        )
    }
}

impl fmt::Display for EscalationReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnergyThreshold { energy, threshold } => {
                write!(
                    f,
                    "Energy {:.3} exceeded threshold {:.3}",
                    *energy as f32 / 1000.0,
                    *threshold as f32 / 1000.0
                )
            }
            Self::PersistentIncoherence {
                duration_ms,
                window_ms,
            } => {
                write!(
                    f,
                    "Persistent incoherence for {}ms (window: {}ms)",
                    duration_ms, window_ms
                )
            }
            Self::GrowingIncoherence { growth_rate } => {
                write!(
                    f,
                    "Growing incoherence at {:.3}/s",
                    *growth_rate as f32 / 1000.0
                )
            }
            Self::ExternalTrigger { source } => {
                write!(f, "External trigger from: {}", source)
            }
            Self::SystemOverride { reason } => {
                write!(f, "System override: {}", reason)
            }
        }
    }
}

/// Lane transition record for audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaneTransition {
    /// Previous lane.
    pub from_lane: ComputeLane,

    /// New lane.
    pub to_lane: ComputeLane,

    /// Reason for transition.
    pub reason: EscalationReason,

    /// Timestamp of transition (Unix millis).
    pub timestamp_ms: u64,

    /// Energy at time of transition.
    pub energy: f32,
}

impl LaneTransition {
    /// Create a new lane transition record.
    pub fn new(
        from_lane: ComputeLane,
        to_lane: ComputeLane,
        reason: EscalationReason,
        energy: f32,
    ) -> Self {
        Self {
            from_lane,
            to_lane,
            reason,
            timestamp_ms: Self::current_timestamp_ms(),
            energy,
        }
    }

    /// Get current timestamp in milliseconds.
    fn current_timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Whether this is an escalation (moving to higher lane).
    pub fn is_escalation(&self) -> bool {
        self.to_lane > self.from_lane
    }

    /// Whether this is a de-escalation (moving to lower lane).
    pub fn is_deescalation(&self) -> bool {
        self.to_lane < self.from_lane
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lane_ordering() {
        assert!(ComputeLane::Reflex < ComputeLane::Retrieval);
        assert!(ComputeLane::Retrieval < ComputeLane::Heavy);
        assert!(ComputeLane::Heavy < ComputeLane::Human);
    }

    #[test]
    fn test_lane_escalation() {
        assert_eq!(ComputeLane::Reflex.escalate(), Some(ComputeLane::Retrieval));
        assert_eq!(ComputeLane::Retrieval.escalate(), Some(ComputeLane::Heavy));
        assert_eq!(ComputeLane::Heavy.escalate(), Some(ComputeLane::Human));
        assert_eq!(ComputeLane::Human.escalate(), None);
    }

    #[test]
    fn test_lane_deescalation() {
        assert_eq!(ComputeLane::Reflex.deescalate(), None);
        assert_eq!(
            ComputeLane::Retrieval.deescalate(),
            Some(ComputeLane::Reflex)
        );
        assert_eq!(
            ComputeLane::Heavy.deescalate(),
            Some(ComputeLane::Retrieval)
        );
        assert_eq!(ComputeLane::Human.deescalate(), Some(ComputeLane::Heavy));
    }

    #[test]
    fn test_lane_automatic_execution() {
        assert!(ComputeLane::Reflex.allows_automatic_execution());
        assert!(ComputeLane::Retrieval.allows_automatic_execution());
        assert!(ComputeLane::Heavy.allows_automatic_execution());
        assert!(!ComputeLane::Human.allows_automatic_execution());
    }

    #[test]
    fn test_default_thresholds() {
        let thresholds = LaneThresholds::default();
        assert!(thresholds.validate().is_ok());
    }

    #[test]
    fn test_threshold_validation() {
        // Valid thresholds
        let valid = LaneThresholds::new(0.1, 0.5, 0.9);
        assert!(valid.validate().is_ok());

        // Invalid ordering
        let invalid = LaneThresholds::new(0.5, 0.3, 0.9);
        assert!(invalid.validate().is_err());

        // Out of range
        let out_of_range = LaneThresholds::new(-0.1, 0.5, 0.9);
        assert!(out_of_range.validate().is_err());
    }

    #[test]
    fn test_lane_for_energy() {
        let thresholds = LaneThresholds::new(0.2, 0.5, 0.8);

        assert_eq!(thresholds.lane_for_energy(0.1), ComputeLane::Reflex);
        assert_eq!(thresholds.lane_for_energy(0.3), ComputeLane::Retrieval);
        assert_eq!(thresholds.lane_for_energy(0.6), ComputeLane::Heavy);
        assert_eq!(thresholds.lane_for_energy(0.9), ComputeLane::Human);
    }

    #[test]
    fn test_escalation_reason_display() {
        let reason = EscalationReason::energy(0.75, 0.5);
        assert!(reason.to_string().contains("exceeded threshold"));

        let persistent = EscalationReason::persistent(5000, 3000);
        assert!(persistent.to_string().contains("5000ms"));
    }

    #[test]
    fn test_lane_transition() {
        let transition = LaneTransition::new(
            ComputeLane::Reflex,
            ComputeLane::Retrieval,
            EscalationReason::energy(0.3, 0.2),
            0.3,
        );

        assert!(transition.is_escalation());
        assert!(!transition.is_deescalation());
    }
}
