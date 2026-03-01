//! Quality envelope types for ADR-033 progressive indexing hardening.
//!
//! Defines the mandatory outer return type (`QualityEnvelope`) for all query
//! APIs, along with retrieval-level and response-level quality signals,
//! budget reporting, and degradation diagnostics.

/// Quality confidence for a single retrieval candidate.
///
/// Attached per-candidate during the search pipeline. Internal use only;
/// consumers see `ResponseQuality` via the `QualityEnvelope`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum RetrievalQuality {
    /// Full index traversed, high confidence in candidate set.
    Full = 0x00,
    /// Partial index (Layer A+B), good confidence.
    Partial = 0x01,
    /// Layer A only, moderate confidence.
    LayerAOnly = 0x02,
    /// Degenerate distribution detected, low confidence.
    DegenerateDetected = 0x03,
    /// Brute-force fallback used within budget, exact over scanned region.
    BruteForceBudgeted = 0x04,
}

/// Response-level quality signal returned to the caller at the API boundary.
///
/// This is the field that consumers (RAG pipelines, agent tool chains,
/// MCP clients) **must** inspect before using results.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ResponseQuality {
    /// All results from full index. Trust fully.
    Verified = 0x00,
    /// Results from partial index. Usable but may miss neighbors.
    Usable = 0x01,
    /// Degraded retrieval detected. Results are best-effort.
    Degraded = 0x02,
    /// Insufficient candidates found. Results are unreliable.
    Unreliable = 0x03,
}

/// Caller hint for quality vs latency trade-off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum QualityPreference {
    /// Runtime decides. Default. Fastest path that meets internal thresholds.
    Auto = 0x00,
    /// Caller prefers quality over latency. Runtime may widen n_probe,
    /// extend budgets up to 4x, and block until Layer B loads.
    PreferQuality = 0x01,
    /// Caller prefers latency over quality. Runtime may skip safety net,
    /// reduce n_probe. ResponseQuality honestly reports what it gets.
    PreferLatency = 0x02,
    /// Caller explicitly accepts degraded results. Required to proceed
    /// when ResponseQuality would be Degraded or Unreliable under Auto.
    AcceptDegraded = 0x03,
}

impl Default for QualityPreference {
    fn default() -> Self {
        Self::Auto
    }
}

/// Which index layers were available and used during a query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndexLayersUsed {
    pub layer_a: bool,
    pub layer_b: bool,
    pub layer_c: bool,
    pub hot_cache: bool,
}

/// Evidence chain: what index state was actually used for a query.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SearchEvidenceSummary {
    /// Which index layers were available and used.
    pub layers_used: IndexLayersUsed,
    /// Effective n_probe (after any adaptive widening).
    pub n_probe_effective: u32,
    /// Whether degenerate distribution was detected.
    pub degenerate_detected: bool,
    /// Coefficient of variation of top-K centroid distances.
    pub centroid_distance_cv: f32,
    /// Number of candidates found by HNSW before safety net.
    pub hnsw_candidate_count: u32,
    /// Number of candidates added by safety net scan.
    pub safety_net_candidate_count: u32,
}

impl Default for SearchEvidenceSummary {
    fn default() -> Self {
        Self {
            layers_used: IndexLayersUsed::default(),
            n_probe_effective: 0,
            degenerate_detected: false,
            centroid_distance_cv: 0.0,
            hnsw_candidate_count: 0,
            safety_net_candidate_count: 0,
        }
    }
}

/// Resource consumption report for a single query.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BudgetReport {
    /// Wall-clock time for centroid routing (microseconds).
    pub centroid_routing_us: u64,
    /// Wall-clock time for HNSW traversal (microseconds).
    pub hnsw_traversal_us: u64,
    /// Wall-clock time for safety net scan (microseconds).
    pub safety_net_scan_us: u64,
    /// Wall-clock time for reranking (microseconds).
    pub reranking_us: u64,
    /// Total wall-clock time (microseconds).
    pub total_us: u64,
    /// Distance evaluations performed.
    pub distance_ops: u64,
    /// Distance evaluations budget.
    pub distance_ops_budget: u64,
    /// Bytes read from storage.
    pub bytes_read: u64,
    /// Candidates scanned in safety net.
    pub linear_scan_count: u64,
    /// Candidate scan budget.
    pub linear_scan_budget: u64,
}

/// Which fallback path was chosen during query execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum FallbackPath {
    /// Normal HNSW traversal, no fallback needed.
    None = 0x00,
    /// Adaptive n_probe widening due to epoch drift.
    NProbeWidened = 0x01,
    /// Adaptive n_probe widening due to degenerate distribution.
    DegenerateWidened = 0x02,
    /// Selective safety net scan on hot cache.
    SafetyNetSelective = 0x03,
    /// Safety net budget exhausted before completion.
    SafetyNetBudgetExhausted = 0x04,
}

/// Structured reason for quality degradation.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DegradationReason {
    /// Centroid epoch drift exceeded threshold.
    CentroidDrift { epoch_drift: u32, max_drift: u32 },
    /// Degenerate distance distribution detected.
    DegenerateDistribution { cv: f32, threshold: f32 },
    /// Budget exhausted during safety net scan.
    BudgetExhausted {
        scanned: u64,
        total: u64,
        budget_type: BudgetType,
    },
    /// Index layer not yet loaded.
    IndexNotLoaded { available: IndexLayersUsed },
}

/// Which budget cap was hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum BudgetType {
    Time = 0x00,
    Candidates = 0x01,
    DistanceOps = 0x02,
}

/// Why quality is degraded — full diagnostic report.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DegradationReport {
    /// Which fallback path was chosen.
    pub fallback_path: FallbackPath,
    /// Why it was chosen (structured, not prose).
    pub reason: DegradationReason,
    /// What guarantee is lost relative to Full quality.
    pub guarantee_lost: &'static str,
}

/// Budget caps for the brute-force safety net.
///
/// All three are enforced simultaneously. The scan stops at whichever hits
/// first. These are runtime limits, not caller-adjustable above the defaults
/// (unless `QualityPreference::PreferQuality`, which extends to 4x).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SafetyNetBudget {
    /// Maximum wall-clock time for the safety net scan (microseconds).
    pub max_scan_time_us: u64,
    /// Maximum number of candidate vectors to scan.
    pub max_scan_candidates: u64,
    /// Maximum number of distance evaluations.
    pub max_distance_ops: u64,
}

impl SafetyNetBudget {
    /// Layer A only defaults: tight budget for instant first query.
    pub const LAYER_A: Self = Self {
        max_scan_time_us: 2_000, // 2 ms
        max_scan_candidates: 10_000,
        max_distance_ops: 10_000,
    };

    /// Partial index defaults: moderate budget.
    pub const PARTIAL: Self = Self {
        max_scan_time_us: 5_000, // 5 ms
        max_scan_candidates: 50_000,
        max_distance_ops: 50_000,
    };

    /// Full index: generous budget.
    pub const FULL: Self = Self {
        max_scan_time_us: 10_000, // 10 ms
        max_scan_candidates: 100_000,
        max_distance_ops: 100_000,
    };

    /// Disabled: all zeros. Safety net will not scan anything.
    pub const DISABLED: Self = Self {
        max_scan_time_us: 0,
        max_scan_candidates: 0,
        max_distance_ops: 0,
    };

    /// Extend all budgets by 4x for PreferQuality mode.
    /// Uses saturating arithmetic to prevent overflow.
    pub const fn extended_4x(&self) -> Self {
        Self {
            max_scan_time_us: self.max_scan_time_us.saturating_mul(4),
            max_scan_candidates: self.max_scan_candidates.saturating_mul(4),
            max_distance_ops: self.max_distance_ops.saturating_mul(4),
        }
    }

    /// Check if all budgets are zero (disabled).
    pub const fn is_disabled(&self) -> bool {
        self.max_scan_time_us == 0 && self.max_scan_candidates == 0 && self.max_distance_ops == 0
    }
}

impl Default for SafetyNetBudget {
    fn default() -> Self {
        Self::LAYER_A
    }
}

/// Derive `ResponseQuality` from the worst `RetrievalQuality` in the result set.
///
/// Empty input returns `Unreliable` — zero results means zero confidence.
pub fn derive_response_quality(retrieval_qualities: &[RetrievalQuality]) -> ResponseQuality {
    if retrieval_qualities.is_empty() {
        return ResponseQuality::Unreliable;
    }

    let worst = retrieval_qualities
        .iter()
        .copied()
        .max_by_key(|q| *q as u8)
        .unwrap_or(RetrievalQuality::Full);

    match worst {
        RetrievalQuality::Full => ResponseQuality::Verified,
        RetrievalQuality::Partial => ResponseQuality::Usable,
        RetrievalQuality::LayerAOnly => ResponseQuality::Usable,
        RetrievalQuality::DegenerateDetected => ResponseQuality::Degraded,
        RetrievalQuality::BruteForceBudgeted => ResponseQuality::Degraded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retrieval_quality_ordering() {
        assert!(RetrievalQuality::Full < RetrievalQuality::BruteForceBudgeted);
        assert!(RetrievalQuality::Partial < RetrievalQuality::DegenerateDetected);
    }

    #[test]
    fn response_quality_ordering() {
        assert!(ResponseQuality::Verified < ResponseQuality::Unreliable);
        assert!(ResponseQuality::Usable < ResponseQuality::Degraded);
    }

    #[test]
    fn derive_quality_full() {
        let q = derive_response_quality(&[RetrievalQuality::Full, RetrievalQuality::Full]);
        assert_eq!(q, ResponseQuality::Verified);
    }

    #[test]
    fn derive_quality_mixed() {
        let q = derive_response_quality(&[
            RetrievalQuality::Full,
            RetrievalQuality::DegenerateDetected,
        ]);
        assert_eq!(q, ResponseQuality::Degraded);
    }

    #[test]
    fn derive_quality_empty_is_unreliable() {
        let q = derive_response_quality(&[]);
        assert_eq!(q, ResponseQuality::Unreliable);
    }

    #[test]
    fn derive_quality_layer_a() {
        let q = derive_response_quality(&[RetrievalQuality::LayerAOnly]);
        assert_eq!(q, ResponseQuality::Usable);
    }

    #[test]
    fn derive_quality_brute_force() {
        let q = derive_response_quality(&[RetrievalQuality::BruteForceBudgeted]);
        assert_eq!(q, ResponseQuality::Degraded);
    }

    #[test]
    fn safety_net_budget_layer_a() {
        let b = SafetyNetBudget::LAYER_A;
        assert_eq!(b.max_scan_time_us, 2_000);
        assert_eq!(b.max_scan_candidates, 10_000);
        assert_eq!(b.max_distance_ops, 10_000);
        assert!(!b.is_disabled());
    }

    #[test]
    fn safety_net_budget_extended() {
        let b = SafetyNetBudget::LAYER_A.extended_4x();
        assert_eq!(b.max_scan_time_us, 8_000);
        assert_eq!(b.max_scan_candidates, 40_000);
        assert_eq!(b.max_distance_ops, 40_000);
    }

    #[test]
    fn safety_net_budget_disabled() {
        let b = SafetyNetBudget::DISABLED;
        assert!(b.is_disabled());
        assert_eq!(b.max_scan_time_us, 0);
    }

    #[test]
    fn quality_preference_default_is_auto() {
        assert_eq!(QualityPreference::default(), QualityPreference::Auto);
    }

    #[test]
    fn quality_repr_values() {
        assert_eq!(RetrievalQuality::Full as u8, 0x00);
        assert_eq!(RetrievalQuality::BruteForceBudgeted as u8, 0x04);
        assert_eq!(ResponseQuality::Verified as u8, 0x00);
        assert_eq!(ResponseQuality::Unreliable as u8, 0x03);
        assert_eq!(QualityPreference::Auto as u8, 0x00);
        assert_eq!(QualityPreference::AcceptDegraded as u8, 0x03);
    }

    #[test]
    fn fallback_path_repr() {
        assert_eq!(FallbackPath::None as u8, 0x00);
        assert_eq!(FallbackPath::SafetyNetBudgetExhausted as u8, 0x04);
    }

    #[test]
    fn budget_report_default_is_zero() {
        let r = BudgetReport::default();
        assert_eq!(r.total_us, 0);
        assert_eq!(r.distance_ops, 0);
    }

    #[test]
    fn degradation_report_construction() {
        let report = DegradationReport {
            fallback_path: FallbackPath::SafetyNetBudgetExhausted,
            reason: DegradationReason::BudgetExhausted {
                scanned: 5000,
                total: 10000,
                budget_type: BudgetType::DistanceOps,
            },
            guarantee_lost: "recall may be below target",
        };
        assert_eq!(report.fallback_path, FallbackPath::SafetyNetBudgetExhausted);
    }

    #[test]
    fn evidence_summary_default() {
        let e = SearchEvidenceSummary::default();
        assert!(!e.degenerate_detected);
        assert_eq!(e.n_probe_effective, 0);
        assert_eq!(e.centroid_distance_cv, 0.0);
    }

    #[test]
    fn index_layers_default_all_false() {
        let l = IndexLayersUsed::default();
        assert!(!l.layer_a);
        assert!(!l.layer_b);
        assert!(!l.layer_c);
        assert!(!l.hot_cache);
    }
}
