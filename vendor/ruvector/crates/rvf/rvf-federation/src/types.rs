//! Federation segment payload types.
//!
//! Four new RVF segment types (0x33-0x36) defined in ADR-057.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// ── Segment type constants ──────────────────────────────────────────

/// Segment type discriminator for FederatedManifest.
pub const SEG_FEDERATED_MANIFEST: u8 = 0x33;
/// Segment type discriminator for DiffPrivacyProof.
pub const SEG_DIFF_PRIVACY_PROOF: u8 = 0x34;
/// Segment type discriminator for RedactionLog.
pub const SEG_REDACTION_LOG: u8 = 0x35;
/// Segment type discriminator for AggregateWeights.
pub const SEG_AGGREGATE_WEIGHTS: u8 = 0x36;

// ── FederatedManifest (0x33) ────────────────────────────────────────

/// Describes a federated learning export.
///
/// Attached as the first segment in every federation RVF file.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FederatedManifest {
    /// Format version (currently 1).
    pub format_version: u32,
    /// Pseudonym of the contributor (never the real identity).
    pub contributor_pseudonym: String,
    /// UNIX timestamp (seconds) when the export was created.
    pub export_timestamp_s: u64,
    /// Segment IDs included in this export.
    pub included_segment_ids: Vec<u64>,
    /// Cumulative differential privacy budget spent (epsilon).
    pub privacy_budget_spent: f64,
    /// Domain identifier this export applies to.
    pub domain_id: String,
    /// RVF format version compatibility tag.
    pub rvf_version_tag: String,
    /// Number of trajectories summarized in the exported learning.
    pub trajectory_count: u64,
    /// Average quality score of exported trajectories.
    pub avg_quality_score: f64,
}

impl FederatedManifest {
    /// Create a new manifest with required fields.
    pub fn new(contributor_pseudonym: String, domain_id: String) -> Self {
        Self {
            format_version: 1,
            contributor_pseudonym,
            export_timestamp_s: 0,
            included_segment_ids: Vec::new(),
            privacy_budget_spent: 0.0,
            domain_id,
            rvf_version_tag: String::from("rvf-v1"),
            trajectory_count: 0,
            avg_quality_score: 0.0,
        }
    }
}

// ── DiffPrivacyProof (0x34) ─────────────────────────────────────────

/// Noise mechanism used for differential privacy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NoiseMechanism {
    /// Gaussian noise for (epsilon, delta)-DP.
    Gaussian,
    /// Laplace noise for epsilon-DP.
    Laplace,
}

/// Differential privacy attestation.
///
/// Records the privacy parameters and noise applied during export.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DiffPrivacyProof {
    /// Privacy loss parameter.
    pub epsilon: f64,
    /// Probability of privacy failure.
    pub delta: f64,
    /// Noise mechanism applied.
    pub mechanism: NoiseMechanism,
    /// L2 sensitivity bound used for noise calibration.
    pub sensitivity: f64,
    /// Gradient clipping norm.
    pub clipping_norm: f64,
    /// Noise scale (sigma for Gaussian, b for Laplace).
    pub noise_scale: f64,
    /// Number of parameters that had noise added.
    pub noised_parameter_count: u64,
}

impl DiffPrivacyProof {
    /// Create a new proof for Gaussian mechanism.
    pub fn gaussian(epsilon: f64, delta: f64, sensitivity: f64, clipping_norm: f64) -> Self {
        let sigma = sensitivity * (2.0_f64 * (1.25_f64 / delta).ln()).sqrt() / epsilon;
        Self {
            epsilon,
            delta,
            mechanism: NoiseMechanism::Gaussian,
            sensitivity,
            clipping_norm,
            noise_scale: sigma,
            noised_parameter_count: 0,
        }
    }

    /// Create a new proof for Laplace mechanism.
    pub fn laplace(epsilon: f64, sensitivity: f64, clipping_norm: f64) -> Self {
        let b = sensitivity / epsilon;
        Self {
            epsilon,
            delta: 0.0,
            mechanism: NoiseMechanism::Laplace,
            sensitivity,
            clipping_norm,
            noise_scale: b,
            noised_parameter_count: 0,
        }
    }
}

// ── RedactionLog (0x35) ─────────────────────────────────────────────

/// A single redaction event.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RedactionEntry {
    /// Category of PII detected (e.g. "path", "ip", "email", "api_key").
    pub category: String,
    /// Number of occurrences redacted.
    pub count: u32,
    /// Rule identifier that triggered the redaction.
    pub rule_id: String,
}

/// PII stripping attestation.
///
/// Proves that PII scanning was performed without revealing the original content.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RedactionLog {
    /// Individual redaction entries by category.
    pub entries: Vec<RedactionEntry>,
    /// SHAKE-256 hash of the pre-redaction content (32 bytes).
    pub pre_redaction_hash: [u8; 32],
    /// Total number of fields scanned.
    pub fields_scanned: u64,
    /// Total number of redactions applied.
    pub total_redactions: u64,
    /// UNIX timestamp (seconds) when redaction was performed.
    pub timestamp_s: u64,
}

impl RedactionLog {
    /// Create an empty redaction log.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            pre_redaction_hash: [0u8; 32],
            fields_scanned: 0,
            total_redactions: 0,
            timestamp_s: 0,
        }
    }

    /// Add a redaction entry.
    pub fn add_entry(&mut self, category: &str, count: u32, rule_id: &str) {
        self.total_redactions += count as u64;
        self.entries.push(RedactionEntry {
            category: category.to_string(),
            count,
            rule_id: rule_id.to_string(),
        });
    }
}

impl Default for RedactionLog {
    fn default() -> Self {
        Self::new()
    }
}

// ── AggregateWeights (0x36) ─────────────────────────────────────────

/// Federated-averaged weight vector with metadata.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AggregateWeights {
    /// Federated averaging round number.
    pub round: u64,
    /// Number of participants in this round.
    pub participation_count: u32,
    /// Aggregated LoRA delta weights (flattened).
    pub lora_deltas: Vec<f64>,
    /// Per-weight confidence scores.
    pub confidences: Vec<f64>,
    /// Mean loss across participants.
    pub mean_loss: f64,
    /// Loss variance across participants.
    pub loss_variance: f64,
    /// Domain identifier.
    pub domain_id: String,
    /// Whether Byzantine outlier removal was applied.
    pub byzantine_filtered: bool,
    /// Number of contributions removed as outliers.
    pub outliers_removed: u32,
}

impl AggregateWeights {
    /// Create empty aggregate weights for a domain.
    pub fn new(domain_id: String, round: u64) -> Self {
        Self {
            round,
            participation_count: 0,
            lora_deltas: Vec::new(),
            confidences: Vec::new(),
            mean_loss: 0.0,
            loss_variance: 0.0,
            domain_id,
            byzantine_filtered: false,
            outliers_removed: 0,
        }
    }
}

// ── BetaParams (local copy for federation) ──────────────────────────

/// Beta distribution parameters for Thompson Sampling priors.
///
/// Mirrors the type in `ruvector-domain-expansion` to avoid cross-crate dependency.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BetaParams {
    /// Alpha (success count + 1).
    pub alpha: f64,
    /// Beta (failure count + 1).
    pub beta: f64,
}

impl BetaParams {
    /// Create new Beta parameters.
    pub fn new(alpha: f64, beta: f64) -> Self {
        Self { alpha, beta }
    }

    /// Uniform (uninformative) prior.
    pub fn uniform() -> Self {
        Self { alpha: 1.0, beta: 1.0 }
    }

    /// Mean of the Beta distribution.
    pub fn mean(&self) -> f64 {
        self.alpha / (self.alpha + self.beta)
    }

    /// Total observations (alpha + beta - 2 for a Beta(1,1) prior).
    pub fn observations(&self) -> f64 {
        self.alpha + self.beta - 2.0
    }

    /// Merge two Beta posteriors by summing parameters and subtracting the uniform prior.
    pub fn merge(&self, other: &BetaParams) -> BetaParams {
        BetaParams {
            alpha: self.alpha + other.alpha - 1.0,
            beta: self.beta + other.beta - 1.0,
        }
    }

    /// Dampen this prior by mixing with a uniform prior using sqrt-scaling.
    pub fn dampen(&self, factor: f64) -> BetaParams {
        let f = factor.clamp(0.0, 1.0);
        BetaParams {
            alpha: 1.0 + (self.alpha - 1.0) * f,
            beta: 1.0 + (self.beta - 1.0) * f,
        }
    }
}

impl Default for BetaParams {
    fn default() -> Self {
        Self::uniform()
    }
}

// ── TransferPrior (local copy for federation) ───────────────────────

/// Compact summary of learned priors for a single context bucket.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TransferPriorEntry {
    /// Context bucket identifier.
    pub bucket_id: String,
    /// Arm identifier.
    pub arm_id: String,
    /// Beta posterior parameters.
    pub params: BetaParams,
    /// Number of observations backing this prior.
    pub observation_count: u64,
}

/// Collection of transfer priors from a trained domain.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TransferPriorSet {
    /// Source domain identifier.
    pub source_domain: String,
    /// Individual prior entries.
    pub entries: Vec<TransferPriorEntry>,
    /// EMA cost at time of extraction.
    pub cost_ema: f64,
}

// ── PolicyKernelSnapshot ────────────────────────────────────────────

/// Snapshot of a policy kernel configuration for federation export.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PolicyKernelSnapshot {
    /// Kernel identifier.
    pub kernel_id: String,
    /// Tunable knob values.
    pub knobs: Vec<f64>,
    /// Fitness score.
    pub fitness: f64,
    /// Generation number.
    pub generation: u64,
}

// ── CostCurveSnapshot ───────────────────────────────────────────────

/// Snapshot of cost curve data for federation export.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CostCurveSnapshot {
    /// Domain identifier.
    pub domain_id: String,
    /// Ordered (step, cost) points.
    pub points: Vec<(u64, f64)>,
    /// Acceleration factor (> 1.0 means transfer helped).
    pub acceleration: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_type_constants() {
        assert_eq!(SEG_FEDERATED_MANIFEST, 0x33);
        assert_eq!(SEG_DIFF_PRIVACY_PROOF, 0x34);
        assert_eq!(SEG_REDACTION_LOG, 0x35);
        assert_eq!(SEG_AGGREGATE_WEIGHTS, 0x36);
    }

    #[test]
    fn federated_manifest_new() {
        let m = FederatedManifest::new("alice".into(), "genomics".into());
        assert_eq!(m.format_version, 1);
        assert_eq!(m.contributor_pseudonym, "alice");
        assert_eq!(m.domain_id, "genomics");
        assert_eq!(m.trajectory_count, 0);
    }

    #[test]
    fn diff_privacy_proof_gaussian() {
        let p = DiffPrivacyProof::gaussian(1.0, 1e-5, 1.0, 1.0);
        assert_eq!(p.mechanism, NoiseMechanism::Gaussian);
        assert!(p.noise_scale > 0.0);
        assert_eq!(p.epsilon, 1.0);
    }

    #[test]
    fn diff_privacy_proof_laplace() {
        let p = DiffPrivacyProof::laplace(1.0, 1.0, 1.0);
        assert_eq!(p.mechanism, NoiseMechanism::Laplace);
        assert!((p.noise_scale - 1.0).abs() < 1e-10);
    }

    #[test]
    fn redaction_log_add_entry() {
        let mut log = RedactionLog::new();
        log.add_entry("path", 3, "rule_path_unix");
        log.add_entry("ip", 2, "rule_ipv4");
        assert_eq!(log.entries.len(), 2);
        assert_eq!(log.total_redactions, 5);
    }

    #[test]
    fn aggregate_weights_new() {
        let w = AggregateWeights::new("code_review".into(), 1);
        assert_eq!(w.round, 1);
        assert_eq!(w.participation_count, 0);
        assert!(!w.byzantine_filtered);
    }

    #[test]
    fn beta_params_merge() {
        let a = BetaParams::new(10.0, 5.0);
        let b = BetaParams::new(8.0, 3.0);
        let merged = a.merge(&b);
        assert!((merged.alpha - 17.0).abs() < 1e-10);
        assert!((merged.beta - 7.0).abs() < 1e-10);
    }

    #[test]
    fn beta_params_dampen() {
        let p = BetaParams::new(10.0, 5.0);
        let dampened = p.dampen(0.25);
        // alpha = 1 + (10-1)*0.25 = 1 + 2.25 = 3.25
        assert!((dampened.alpha - 3.25).abs() < 1e-10);
        // beta = 1 + (5-1)*0.25 = 1 + 1.0 = 2.0
        assert!((dampened.beta - 2.0).abs() < 1e-10);
    }

    #[test]
    fn beta_params_mean() {
        let p = BetaParams::new(10.0, 10.0);
        assert!((p.mean() - 0.5).abs() < 1e-10);
    }
}
