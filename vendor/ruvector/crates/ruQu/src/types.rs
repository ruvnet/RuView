//! Core domain types for the ruQu coherence gate system
//!
//! This module defines the fundamental types used throughout the coherence
//! gate, including decisions, masks, identifiers, and result structures.

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
// Identifier Types
// ═══════════════════════════════════════════════════════════════════════════

/// Cycle identifier - monotonically increasing per measurement cycle
pub type CycleId = u64;

/// Round identifier - syndrome measurement round within a cycle
pub type RoundId = u64;

/// Tile identifier (0 = TileZero coordinator, 1-255 = worker tiles)
pub type TileId = u8;

/// Vertex identifier in the operational graph
pub type VertexId = u64;

/// Edge identifier in the operational graph
pub type EdgeId = u64;

/// Action identifier for permit tokens
pub type ActionId = String;

/// Decision sequence number
pub type SequenceId = u64;

// ═══════════════════════════════════════════════════════════════════════════
// Gate Decision Types
// ═══════════════════════════════════════════════════════════════════════════

/// The three-valued gate decision outcome
///
/// This is the primary output of the coherence assessment:
/// - `Safe`: System is coherent, action is authorized
/// - `Cautious`: Uncertainty detected, elevated monitoring recommended
/// - `Unsafe`: Incoherence detected, region should be quarantined
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateDecision {
    /// System is coherent enough to trust action
    ///
    /// All three filters passed with sufficient margin.
    /// A permit token will be issued.
    Safe,

    /// Uncertainty detected, proceed with caution
    ///
    /// One or more filters are in warning range but not failing.
    /// Elevated monitoring and conservative decoder recommended.
    Cautious,

    /// Incoherence detected, quarantine region
    ///
    /// At least one filter has hard-failed.
    /// The affected region should be isolated from action.
    Unsafe,
}

impl GateDecision {
    /// Check if decision permits action
    #[inline]
    pub fn permits_action(&self) -> bool {
        matches!(self, GateDecision::Safe)
    }

    /// Check if decision requires escalation
    #[inline]
    pub fn requires_escalation(&self) -> bool {
        matches!(self, GateDecision::Cautious | GateDecision::Unsafe)
    }

    /// Check if decision requires quarantine
    #[inline]
    pub fn requires_quarantine(&self) -> bool {
        matches!(self, GateDecision::Unsafe)
    }

    /// Convert to cognitum-gate-tilezero compatible decision
    #[cfg(feature = "tilezero")]
    pub fn to_tilezero(&self) -> cognitum_gate_tilezero::GateDecision {
        match self {
            GateDecision::Safe => cognitum_gate_tilezero::GateDecision::Permit,
            GateDecision::Cautious => cognitum_gate_tilezero::GateDecision::Defer,
            GateDecision::Unsafe => cognitum_gate_tilezero::GateDecision::Deny,
        }
    }
}

impl std::fmt::Display for GateDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateDecision::Safe => write!(f, "SAFE"),
            GateDecision::Cautious => write!(f, "CAUTIOUS"),
            GateDecision::Unsafe => write!(f, "UNSAFE"),
        }
    }
}

impl Default for GateDecision {
    fn default() -> Self {
        GateDecision::Cautious // Conservative default
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Verdict (Simplified Decision)
// ═══════════════════════════════════════════════════════════════════════════

/// Simplified verdict for filter outcomes
///
/// Used internally by individual filters before combining into GateDecision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Verdict {
    /// Filter passed, action permitted
    Permit,
    /// Filter inconclusive, defer to human/stronger model
    Defer,
    /// Filter failed, action denied
    Deny,
}

impl Verdict {
    /// Convert verdict to gate decision
    pub fn to_gate_decision(&self) -> GateDecision {
        match self {
            Verdict::Permit => GateDecision::Safe,
            Verdict::Defer => GateDecision::Cautious,
            Verdict::Deny => GateDecision::Unsafe,
        }
    }
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Verdict::Permit => write!(f, "permit"),
            Verdict::Defer => write!(f, "defer"),
            Verdict::Deny => write!(f, "deny"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Region Mask (256-bit)
// ═══════════════════════════════════════════════════════════════════════════

/// 256-bit mask identifying affected tiles/regions
///
/// Each bit corresponds to a tile ID (0-255). Used to indicate which
/// regions are affected by a decision, which need quarantine, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RegionMask {
    /// Four 64-bit words covering 256 bits
    bits: [u64; 4],
}

impl RegionMask {
    /// Create a mask with all bits clear (no regions)
    #[inline]
    pub const fn none() -> Self {
        Self { bits: [0; 4] }
    }

    /// Create a mask with all bits set (all regions)
    #[inline]
    pub const fn all() -> Self {
        Self {
            bits: [u64::MAX; 4],
        }
    }

    /// Create a mask from a slice of tile IDs
    pub fn from_tiles(tiles: &[TileId]) -> Self {
        let mut mask = Self::none();
        for &tile in tiles {
            mask.set(tile);
        }
        mask
    }

    /// Create a mask from raw bits
    #[inline]
    pub const fn from_bits(bits: [u64; 4]) -> Self {
        Self { bits }
    }

    /// Get the raw bits
    #[inline]
    pub const fn bits(&self) -> [u64; 4] {
        self.bits
    }

    /// Set a specific tile bit
    #[inline]
    pub fn set(&mut self, tile: TileId) {
        let word = (tile / 64) as usize;
        let bit = tile % 64;
        self.bits[word] |= 1u64 << bit;
    }

    /// Clear a specific tile bit
    #[inline]
    pub fn clear(&mut self, tile: TileId) {
        let word = (tile / 64) as usize;
        let bit = tile % 64;
        self.bits[word] &= !(1u64 << bit);
    }

    /// Check if a specific tile is set
    #[inline]
    pub fn is_set(&self, tile: TileId) -> bool {
        let word = (tile / 64) as usize;
        let bit = tile % 64;
        (self.bits[word] & (1u64 << bit)) != 0
    }

    /// Count the number of set bits
    #[inline]
    pub fn count(&self) -> u32 {
        self.bits.iter().map(|w| w.count_ones()).sum()
    }

    /// Check if mask is empty (no tiles set)
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|&w| w == 0)
    }

    /// Check if mask is full (all tiles set)
    #[inline]
    pub fn is_full(&self) -> bool {
        self.bits.iter().all(|&w| w == u64::MAX)
    }

    /// Compute union (OR) with another mask
    #[inline]
    pub fn union(&self, other: &RegionMask) -> RegionMask {
        RegionMask {
            bits: [
                self.bits[0] | other.bits[0],
                self.bits[1] | other.bits[1],
                self.bits[2] | other.bits[2],
                self.bits[3] | other.bits[3],
            ],
        }
    }

    /// Compute intersection (AND) with another mask
    #[inline]
    pub fn intersection(&self, other: &RegionMask) -> RegionMask {
        RegionMask {
            bits: [
                self.bits[0] & other.bits[0],
                self.bits[1] & other.bits[1],
                self.bits[2] & other.bits[2],
                self.bits[3] & other.bits[3],
            ],
        }
    }

    /// Check if this mask intersects with another
    #[inline]
    pub fn intersects(&self, other: &RegionMask) -> bool {
        !self.intersection(other).is_empty()
    }

    /// Compute complement (NOT) of this mask
    #[inline]
    pub fn complement(&self) -> RegionMask {
        RegionMask {
            bits: [!self.bits[0], !self.bits[1], !self.bits[2], !self.bits[3]],
        }
    }

    /// Iterate over set tile IDs
    pub fn iter_set(&self) -> impl Iterator<Item = TileId> + '_ {
        (0u8..=255).filter(|&t| self.is_set(t))
    }
}

impl Default for RegionMask {
    fn default() -> Self {
        Self::none()
    }
}

impl std::fmt::Display for RegionMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RegionMask({} tiles)", self.count())
    }
}

impl std::ops::BitOr for RegionMask {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(&rhs)
    }
}

impl std::ops::BitAnd for RegionMask {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        self.intersection(&rhs)
    }
}

impl std::ops::Not for RegionMask {
    type Output = Self;
    fn not(self) -> Self::Output {
        self.complement()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Permit Token
// ═══════════════════════════════════════════════════════════════════════════

/// A signed permit token authorizing action on coherent regions
///
/// Tokens are issued by TileZero when the coherence gate decides SAFE.
/// They include cryptographic proof of the decision for audit purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermitToken {
    /// Decision that led to this permit
    pub decision: GateDecision,
    /// Action being permitted
    pub action_id: ActionId,
    /// Regions covered by this permit
    pub region_mask: RegionMask,
    /// Timestamp of issuance (nanoseconds since epoch)
    pub issued_at: u64,
    /// Expiration timestamp (nanoseconds since epoch)
    pub expires_at: u64,
    /// Sequence number for ordering
    pub sequence: SequenceId,
    /// Blake3 hash of the witness data
    #[serde(with = "hex_array")]
    pub witness_hash: [u8; 32],
    /// Ed25519 signature (64 bytes)
    #[serde(with = "hex_array")]
    pub signature: [u8; 64],
}

impl PermitToken {
    /// Check if token is currently valid (not expired)
    pub fn is_valid(&self, now_ns: u64) -> bool {
        now_ns >= self.issued_at && now_ns < self.expires_at
    }

    /// Get time-to-live in nanoseconds
    pub fn ttl_ns(&self) -> u64 {
        self.expires_at.saturating_sub(self.issued_at)
    }

    /// Check if token covers a specific tile
    pub fn covers_tile(&self, tile: TileId) -> bool {
        self.region_mask.is_set(tile)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Filter Results
// ═══════════════════════════════════════════════════════════════════════════

/// Combined results from all three filters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterResults {
    /// Structural filter (min-cut) result
    pub structural: StructuralResult,
    /// Shift filter (drift detection) result
    pub shift: ShiftResult,
    /// Evidence filter (e-value) result
    pub evidence: EvidenceResult,
}

impl FilterResults {
    /// Compute overall verdict from filter results
    pub fn verdict(&self) -> Verdict {
        // If any filter denies, deny
        if self.structural.verdict == Verdict::Deny
            || self.shift.verdict == Verdict::Deny
            || self.evidence.verdict == Verdict::Deny
        {
            return Verdict::Deny;
        }

        // If any filter defers, defer
        if self.structural.verdict == Verdict::Defer
            || self.shift.verdict == Verdict::Defer
            || self.evidence.verdict == Verdict::Defer
        {
            return Verdict::Defer;
        }

        // All filters permit
        Verdict::Permit
    }

    /// Compute overall confidence (0.0 - 1.0)
    pub fn confidence(&self) -> f64 {
        (self.structural.confidence + self.shift.confidence + self.evidence.confidence) / 3.0
    }
}

/// Result from structural (min-cut) filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralResult {
    /// Filter verdict
    pub verdict: Verdict,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Computed min-cut value
    pub cut_value: f64,
    /// Threshold used for comparison
    pub threshold: f64,
    /// Edges in the min-cut (boundary)
    pub boundary_edges: Vec<EdgeId>,
}

/// Result from shift (drift detection) filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShiftResult {
    /// Filter verdict
    pub verdict: Verdict,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Computed shift pressure
    pub shift_pressure: f64,
    /// Threshold used for comparison
    pub threshold: f64,
    /// Regions with elevated shift
    pub affected_regions: RegionMask,
}

/// Result from evidence (e-value) filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceResult {
    /// Filter verdict
    pub verdict: Verdict,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Computed e-value
    pub e_value: f64,
    /// Deny threshold (τ_deny)
    pub tau_deny: f64,
    /// Permit threshold (τ_permit)
    pub tau_permit: f64,
}

// ═══════════════════════════════════════════════════════════════════════════
// Thresholds Configuration
// ═══════════════════════════════════════════════════════════════════════════

/// Threshold configuration for the coherence gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateThresholds {
    // Structural filter thresholds
    /// Minimum cut value for structural stability
    pub min_cut: f64,

    // Shift filter thresholds
    /// Maximum shift pressure before deferral
    pub max_shift: f64,

    // Evidence filter thresholds
    /// E-value threshold for denial (below this = deny)
    pub tau_deny: f64,
    /// E-value threshold for permit (above this = permit)
    pub tau_permit: f64,

    // Timing configuration
    /// Permit token TTL in nanoseconds
    pub permit_ttl_ns: u64,
    /// Decision budget in nanoseconds
    pub decision_budget_ns: u64,
}

impl Default for GateThresholds {
    fn default() -> Self {
        Self {
            min_cut: 5.0,
            max_shift: 0.5,
            tau_deny: 0.01,
            tau_permit: 100.0,
            permit_ttl_ns: 60_000_000_000, // 60 seconds
            decision_budget_ns: 4_000,     // 4 microseconds
        }
    }
}

/// Minimum permit TTL in nanoseconds (1 millisecond)
const MIN_PERMIT_TTL_NS: u64 = 1_000_000;

/// Maximum permit TTL in nanoseconds (1 hour)
const MAX_PERMIT_TTL_NS: u64 = 3_600_000_000_000;

/// Minimum decision budget in nanoseconds (100 nanoseconds)
const MIN_DECISION_BUDGET_NS: u64 = 100;

/// Maximum decision budget in nanoseconds (1 second)
const MAX_DECISION_BUDGET_NS: u64 = 1_000_000_000;

impl GateThresholds {
    /// Validate thresholds
    ///
    /// Checks that all threshold values are within acceptable bounds.
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.min_cut <= 0.0 {
            return Err(crate::error::RuQuError::InvalidThreshold {
                name: "min_cut".to_string(),
                value: self.min_cut,
                constraint: "> 0".to_string(),
            });
        }
        if self.max_shift <= 0.0 || self.max_shift > 1.0 {
            return Err(crate::error::RuQuError::InvalidThreshold {
                name: "max_shift".to_string(),
                value: self.max_shift,
                constraint: "in (0, 1]".to_string(),
            });
        }
        if self.tau_deny <= 0.0 {
            return Err(crate::error::RuQuError::InvalidThreshold {
                name: "tau_deny".to_string(),
                value: self.tau_deny,
                constraint: "> 0".to_string(),
            });
        }
        if self.tau_permit <= self.tau_deny {
            return Err(crate::error::RuQuError::InvalidThreshold {
                name: "tau_permit".to_string(),
                value: self.tau_permit,
                constraint: format!("> tau_deny ({})", self.tau_deny),
            });
        }

        // SECURITY: Validate timing parameters to prevent DoS or overflow
        if self.permit_ttl_ns < MIN_PERMIT_TTL_NS || self.permit_ttl_ns > MAX_PERMIT_TTL_NS {
            return Err(crate::error::RuQuError::InvalidThreshold {
                name: "permit_ttl_ns".to_string(),
                value: self.permit_ttl_ns as f64,
                constraint: format!("in [{}, {}]", MIN_PERMIT_TTL_NS, MAX_PERMIT_TTL_NS),
            });
        }
        if self.decision_budget_ns < MIN_DECISION_BUDGET_NS
            || self.decision_budget_ns > MAX_DECISION_BUDGET_NS
        {
            return Err(crate::error::RuQuError::InvalidThreshold {
                name: "decision_budget_ns".to_string(),
                value: self.decision_budget_ns as f64,
                constraint: format!(
                    "in [{}, {}]",
                    MIN_DECISION_BUDGET_NS, MAX_DECISION_BUDGET_NS
                ),
            });
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper modules for serde
// ═══════════════════════════════════════════════════════════════════════════

mod hex_array {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S, const N: usize>(bytes: &[u8; N], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_string: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
        serializer.serialize_str(&hex_string)
    }

    pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        // SECURITY: Validate hex string length to prevent panic on odd-length strings
        if s.len() % 2 != 0 {
            return Err(serde::de::Error::custom(format!(
                "hex string must have even length, got {}",
                s.len()
            )));
        }

        let bytes: Vec<u8> = (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)?;

        bytes.try_into().map_err(|_| {
            serde::de::Error::custom(format!("expected {} bytes, got {}", N, s.len() / 2))
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Structural Signal with Dynamics
// ═══════════════════════════════════════════════════════════════════════════

/// Structural signal with cut dynamics (velocity and curvature)
///
/// This captures not just the absolute min-cut value, but also its rate of change.
/// Most early warnings come from **consistent decline** (negative velocity),
/// not just low absolute value. Tracking dynamics improves lead time without
/// increasing false alarms.
///
/// # Example
///
/// ```rust
/// use ruqu::types::StructuralSignal;
///
/// let signal = StructuralSignal {
///     cut: 4.5,
///     velocity: -0.3,  // Declining
///     curvature: -0.1, // Accelerating decline
///     baseline_mean: 6.0,
///     baseline_std: 0.5,
/// };
///
/// // Warning triggers on trend, not threshold alone
/// let is_declining = signal.velocity < 0.0;
/// let is_below_baseline = signal.cut < signal.baseline_mean - 2.0 * signal.baseline_std;
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StructuralSignal {
    /// Current min-cut value (λ)
    pub cut: f64,
    /// Rate of change (Δλ) - positive = improving, negative = degrading
    pub velocity: f64,
    /// Acceleration of change (Δ²λ) - second derivative
    pub curvature: f64,
    /// Baseline mean from warmup period
    pub baseline_mean: f64,
    /// Baseline standard deviation
    pub baseline_std: f64,
}

impl StructuralSignal {
    /// Check if signal indicates degradation (negative trend)
    #[inline]
    pub fn is_degrading(&self) -> bool {
        self.velocity < 0.0
    }

    /// Check if signal is below adaptive threshold (μ - kσ)
    #[inline]
    pub fn is_below_threshold(&self, k: f64) -> bool {
        self.cut < self.baseline_mean - k * self.baseline_std
    }

    /// Compute z-score relative to baseline
    #[inline]
    pub fn z_score(&self) -> f64 {
        if self.baseline_std == 0.0 {
            return 0.0;
        }
        (self.cut - self.baseline_mean) / self.baseline_std
    }

    /// Estimate time to threshold crossing (in cycles)
    ///
    /// Returns `None` if not degrading or velocity is zero.
    pub fn time_to_threshold(&self, threshold: f64) -> Option<f64> {
        if self.velocity >= 0.0 || self.cut <= threshold {
            return None;
        }
        Some((self.cut - threshold) / (-self.velocity))
    }
}

impl std::fmt::Display for StructuralSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let trend = if self.velocity > 0.1 {
            "↑"
        } else if self.velocity < -0.1 {
            "↓"
        } else {
            "→"
        };
        write!(
            f,
            "λ={:.2}{} (v={:+.2}, z={:+.1}σ)",
            self.cut,
            trend,
            self.velocity,
            self.z_score()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_decision() {
        assert!(GateDecision::Safe.permits_action());
        assert!(!GateDecision::Cautious.permits_action());
        assert!(!GateDecision::Unsafe.permits_action());

        assert!(!GateDecision::Safe.requires_escalation());
        assert!(GateDecision::Cautious.requires_escalation());
        assert!(GateDecision::Unsafe.requires_escalation());

        assert!(!GateDecision::Safe.requires_quarantine());
        assert!(!GateDecision::Cautious.requires_quarantine());
        assert!(GateDecision::Unsafe.requires_quarantine());
    }

    #[test]
    fn test_region_mask_basic() {
        let mut mask = RegionMask::none();
        assert!(mask.is_empty());
        assert_eq!(mask.count(), 0);

        mask.set(0);
        mask.set(127);
        mask.set(255);
        assert_eq!(mask.count(), 3);
        assert!(mask.is_set(0));
        assert!(mask.is_set(127));
        assert!(mask.is_set(255));
        assert!(!mask.is_set(1));

        mask.clear(127);
        assert_eq!(mask.count(), 2);
        assert!(!mask.is_set(127));
    }

    #[test]
    fn test_region_mask_from_tiles() {
        let mask = RegionMask::from_tiles(&[1, 5, 10, 200]);
        assert_eq!(mask.count(), 4);
        assert!(mask.is_set(1));
        assert!(mask.is_set(5));
        assert!(mask.is_set(10));
        assert!(mask.is_set(200));
        assert!(!mask.is_set(0));
    }

    #[test]
    fn test_region_mask_operations() {
        let a = RegionMask::from_tiles(&[1, 2, 3]);
        let b = RegionMask::from_tiles(&[2, 3, 4]);

        let union = a | b;
        assert_eq!(union.count(), 4);
        assert!(union.is_set(1));
        assert!(union.is_set(4));

        let intersection = a & b;
        assert_eq!(intersection.count(), 2);
        assert!(intersection.is_set(2));
        assert!(intersection.is_set(3));
        assert!(!intersection.is_set(1));

        assert!(a.intersects(&b));
    }

    #[test]
    fn test_region_mask_all_none() {
        let all = RegionMask::all();
        assert!(all.is_full());
        assert_eq!(all.count(), 256);
        assert!(all.is_set(0));
        assert!(all.is_set(255));

        let none = RegionMask::none();
        assert!(none.is_empty());
        assert_eq!(none.count(), 0);

        let complement = !none;
        assert!(complement.is_full());
    }

    #[test]
    fn test_gate_thresholds_default() {
        let thresholds = GateThresholds::default();
        assert!(thresholds.validate().is_ok());
    }

    #[test]
    fn test_gate_thresholds_invalid() {
        let mut thresholds = GateThresholds::default();
        thresholds.min_cut = -1.0;
        assert!(thresholds.validate().is_err());

        let mut thresholds = GateThresholds::default();
        thresholds.tau_permit = 0.001; // Less than tau_deny
        assert!(thresholds.validate().is_err());
    }

    #[test]
    fn test_filter_results_verdict() {
        let results = FilterResults {
            structural: StructuralResult {
                verdict: Verdict::Permit,
                confidence: 1.0,
                cut_value: 10.0,
                threshold: 5.0,
                boundary_edges: vec![],
            },
            shift: ShiftResult {
                verdict: Verdict::Permit,
                confidence: 0.9,
                shift_pressure: 0.1,
                threshold: 0.5,
                affected_regions: RegionMask::none(),
            },
            evidence: EvidenceResult {
                verdict: Verdict::Permit,
                confidence: 0.95,
                e_value: 150.0,
                tau_deny: 0.01,
                tau_permit: 100.0,
            },
        };

        assert_eq!(results.verdict(), Verdict::Permit);
        assert!(results.confidence() > 0.9);
    }

    #[test]
    fn test_permit_token_validity() {
        let token = PermitToken {
            decision: GateDecision::Safe,
            action_id: "test-action".to_string(),
            region_mask: RegionMask::all(),
            issued_at: 1000,
            expires_at: 2000,
            sequence: 0,
            witness_hash: [0u8; 32],
            signature: [0u8; 64],
        };

        assert!(token.is_valid(1500));
        assert!(!token.is_valid(500));
        assert!(!token.is_valid(2500));
        assert_eq!(token.ttl_ns(), 1000);
    }
}
