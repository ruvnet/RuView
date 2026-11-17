//! Enforceable sensing evidence and claim gate (ADR-328).
//!
//! RuView already distinguishes measured, synthetic, and claimed metrics, and
//! [`crate::occupancy_bench`] refuses accuracy claims from mock data. The
//! remaining release gap was an artifact boundary: a maintainer could still
//! publish a production or safety claim without a machine-readable record of
//! hardware provenance, a held-out environment, a minimum sample count, or a
//! pre-registered threshold.
//!
//! This module validates an untrusted JSON claim manifest against a separate,
//! repository-owned policy. Claim authors report observations; they do not get
//! to choose their own release thresholds. Every evaluation produces a
//! deterministic receipt with input digests and one result per rule. Synthetic
//! evidence may support a research-only statement, but it can never release a
//! production or safety-critical claim.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Claim-manifest schema understood by this implementation.
pub const CLAIM_SCHEMA_VERSION: &str = "ruview.sensing-claim-evidence/v1";

/// Policy schema understood by this implementation.
pub const POLICY_SCHEMA_VERSION: &str = "ruview.sensing-claim-policy/v1";

/// Receipt schema emitted by this implementation.
pub const RECEIPT_SCHEMA_VERSION: &str = "ruview.sensing-claim-receipt/v1";

/// Maximum accepted manifest or policy size. Evidence files contain metadata
/// and digests, not raw CSI or personal data.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;

/// Errors at the JSON and receipt-construction boundary.
#[derive(Debug, Error)]
pub enum ClaimGateError {
    /// A manifest exceeded the metadata-only size bound.
    #[error("claim manifest is {actual} bytes; maximum is {maximum}")]
    ManifestTooLarge {
        /// Observed input length.
        actual: usize,
        /// Maximum permitted input length.
        maximum: usize,
    },
    /// A policy exceeded the metadata-only size bound.
    #[error("claim policy is {actual} bytes; maximum is {maximum}")]
    PolicyTooLarge {
        /// Observed input length.
        actual: usize,
        /// Maximum permitted input length.
        maximum: usize,
    },
    /// The claim manifest was not valid strict JSON for the v1 contract.
    #[error("invalid claim manifest JSON: {0}")]
    ManifestJson(#[source] serde_json::Error),
    /// The policy was not valid strict JSON for the v1 contract.
    #[error("invalid claim policy JSON: {0}")]
    PolicyJson(#[source] serde_json::Error),
    /// A deterministic receipt could not be serialized.
    #[error("could not serialize claim receipt: {0}")]
    ReceiptJson(#[source] serde_json::Error),
}

/// The consequence of releasing a sensing statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimClass {
    /// Research result with explicit limitations. Synthetic input is allowed.
    Research,
    /// Product-facing capability or accuracy statement.
    Production,
    /// A statement that could authorize or justify a safety-critical action.
    SafetyCritical,
}

impl ClaimClass {
    fn tag(self) -> &'static str {
        match self {
            ClaimClass::Research => "research",
            ClaimClass::Production => "production",
            ClaimClass::SafetyCritical => "safety_critical",
        }
    }

    fn requires_thresholds(self) -> bool {
        !matches!(self, ClaimClass::Research)
    }
}

/// Origin of the evaluated RF samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSourceKind {
    /// Generated mathematical or learned synthetic samples.
    Synthetic,
    /// Frames emitted by a simulator rather than a radio.
    Simulator,
    /// Mock or stub samples used by tests.
    Mock,
    /// A replay of a capture that was recorded on physical hardware.
    RecordedHardware,
    /// Frames evaluated while arriving from physical hardware.
    LiveHardware,
}

impl EvidenceSourceKind {
    fn tag(self) -> &'static str {
        match self {
            EvidenceSourceKind::Synthetic => "synthetic",
            EvidenceSourceKind::Simulator => "simulator",
            EvidenceSourceKind::Mock => "mock",
            EvidenceSourceKind::RecordedHardware => "recorded_hardware",
            EvidenceSourceKind::LiveHardware => "live_hardware",
        }
    }

    fn is_real_hardware(self) -> bool {
        matches!(
            self,
            EvidenceSourceKind::RecordedHardware | EvidenceSourceKind::LiveHardware
        )
    }
}

/// Evaluation split used to compute the submitted metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationProtocol {
    /// Training and testing share the same environment distribution.
    InDomain,
    /// Test subjects do not occur in training.
    CrossSubject,
    /// Test environments do not occur in training.
    HeldOutEnvironment,
}

/// Content-addressed reference to an evidence artifact. The artifact itself is
/// deliberately not embedded in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDigest {
    /// Stable local, object-store, or registry reference.
    pub uri: String,
    /// Lower-case hexadecimal SHA-256 digest.
    pub sha256: String,
}

/// Provenance needed to distinguish simulation from physical capture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEvidence {
    /// Source category.
    pub kind: EvidenceSourceKind,
    /// Hardware family, required for hardware evidence.
    pub device_family: Option<String>,
    /// Stable enrolled or pseudonymous device identifier.
    pub device_id: Option<String>,
    /// Firmware version or immutable build identifier.
    pub firmware_version: Option<String>,
    /// Raw or derived capture bundle digest.
    pub capture_artifact: Option<ArtifactDigest>,
    /// Independently recorded ground-truth bundle digest.
    pub ground_truth_artifact: Option<ArtifactDigest>,
}

/// Leakage and sample accounting for one evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationEvidence {
    /// Evaluation protocol.
    pub protocol: EvaluationProtocol,
    /// Environments used in training or calibration.
    pub train_environment_ids: Vec<String>,
    /// Environments used only for testing.
    pub test_environment_ids: Vec<String>,
    /// Per-environment sample accounting and metrics. Required when the class
    /// requires held-out environments so aggregate metrics cannot hide a weak
    /// or empty room.
    pub environment_results: Vec<EnvironmentEvidence>,
    /// Subjects used in training or calibration, when applicable.
    pub train_subject_ids: Vec<String>,
    /// Subjects used only for testing, when applicable.
    pub test_subject_ids: Vec<String>,
    /// Total number of held-out test samples.
    pub test_samples: usize,
    /// Positive samples for binary capabilities such as presence.
    pub positive_test_samples: Option<usize>,
    /// Negative samples for binary capabilities such as presence.
    pub negative_test_samples: Option<usize>,
}

/// Metrics and sample accounting for one held-out environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentEvidence {
    /// Identifier matching one entry in `test_environment_ids`.
    pub environment_id: String,
    /// Number of samples evaluated in this environment.
    pub test_samples: usize,
    /// Positive samples for binary capabilities.
    pub positive_test_samples: Option<usize>,
    /// Negative samples for binary capabilities.
    pub negative_test_samples: Option<usize>,
    /// Environment-specific metrics using the same policy-owned metric names.
    pub metrics: BTreeMap<String, MetricEvidence>,
}

/// One measured metric and its confidence interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricEvidence {
    /// Point estimate.
    pub point_estimate: f64,
    /// Lower confidence bound computed by the reproducer.
    pub ci_lower: Option<f64>,
    /// Upper confidence bound computed by the reproducer.
    pub ci_upper: Option<f64>,
}

/// Independent review reference required by some policy classes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependentValidation {
    /// Reviewing organization or laboratory.
    pub reviewer: String,
    /// Content-addressed review report.
    pub report_artifact: ArtifactDigest,
}

/// Immutable inputs bound into an evaluation result. This metadata is still
/// subject to manual artifact retrieval and trust-root verification; it is not
/// treated as a signature or witness by this gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationBinding {
    /// Evaluator identifier expected by the repository policy.
    pub evaluator_id: String,
    /// Named confidence-interval method, for example `bootstrap_percentile_95`.
    pub confidence_method: String,
    /// Exact source commit evaluated, as a 40- or 64-character Git object id.
    pub source_commit_sha: String,
    /// Content-addressed model artifact.
    pub model_artifact: ArtifactDigest,
    /// Content-addressed train/test split manifest.
    pub split_manifest_artifact: ArtifactDigest,
    /// Content-addressed machine-readable evaluation report.
    pub evaluation_report_artifact: ArtifactDigest,
    /// Reproducer argv. It is never executed by this metadata gate.
    pub reproducer_argv: Vec<String>,
}

/// Machine-readable input submitted by a claim author.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimManifest {
    /// Exact manifest schema version.
    pub schema_version: String,
    /// Stable identifier for this proposed claim.
    pub claim_id: String,
    /// Capability name, for example `presence` or `pose`.
    pub capability: String,
    /// Intended release class.
    pub claim_class: ClaimClass,
    /// Human-readable statement that the evidence is intended to support.
    pub statement: String,
    /// Exact repository paths where the statement is presented publicly.
    /// Production and safety manifests must bind at least one protected path.
    pub claim_surface_paths: Vec<String>,
    /// Input provenance.
    pub source: SourceEvidence,
    /// Split and sample accounting.
    pub evaluation: EvaluationEvidence,
    /// Metrics keyed by policy-owned metric name.
    pub metrics: BTreeMap<String, MetricEvidence>,
    /// Commit, model, split, evaluator, report, and reproducer binding.
    pub evaluation_binding: Option<EvaluationBinding>,
    /// Optional independent review.
    pub independent_validation: Option<IndependentValidation>,
}

/// Confidence statistic compared with a policy threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdStatistic {
    /// Point estimate. Forbidden for production and safety claims.
    PointEstimate,
    /// Lower confidence bound, normally used for minimum thresholds.
    LowerConfidenceBound,
    /// Upper confidence bound, normally used for maximum thresholds.
    UpperConfidenceBound,
}

/// Pre-registered threshold for a named metric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricThreshold {
    /// Minimum permitted value. Exactly one of `minimum` and `maximum` is set.
    pub minimum: Option<f64>,
    /// Maximum permitted value. Exactly one of `minimum` and `maximum` is set.
    pub maximum: Option<f64>,
    /// Which statistic is compared to the limit.
    pub statistic: ThresholdStatistic,
}

/// Capability-specific release requirements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityPolicy {
    /// Whether positive and negative sample counts are mandatory.
    pub require_binary_class_counts: bool,
    /// Minimum positive samples when binary counts are required.
    pub min_positive_samples: usize,
    /// Minimum negative samples when binary counts are required.
    pub min_negative_samples: usize,
    /// Minimum positive samples required in each held-out environment.
    pub min_positive_samples_per_environment: usize,
    /// Minimum negative samples required in each held-out environment.
    pub min_negative_samples_per_environment: usize,
    /// Required metrics and their pre-registered thresholds.
    pub metrics: BTreeMap<String, MetricThreshold>,
}

/// Requirements for one claim class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimClassPolicy {
    /// Global switch. Disabled classes always fail closed.
    pub enabled: bool,
    /// Whether simulator, synthetic, or mock evidence is permitted.
    pub allow_non_hardware: bool,
    /// Whether complete physical capture metadata is mandatory.
    pub require_hardware_evidence: bool,
    /// Minimum held-out test samples.
    pub min_test_samples: usize,
    /// Minimum distinct held-out environments.
    pub min_test_environments: usize,
    /// Minimum samples required in each held-out environment.
    pub min_samples_per_environment: usize,
    /// Whether the protocol must be `held_out_environment`.
    pub require_held_out_environment: bool,
    /// Whether training and test subjects must also be disjoint.
    pub require_subject_disjoint: bool,
    /// Whether a reproducer is mandatory.
    pub require_reproducer: bool,
    /// Whether a content-addressed independent report is mandatory.
    pub require_independent_validation: bool,
    /// Capability policies. `*` is an optional research fallback only.
    pub capabilities: BTreeMap<String, CapabilityPolicy>,
}

impl ClaimClassPolicy {
    fn deny_all() -> Self {
        Self {
            enabled: false,
            allow_non_hardware: false,
            require_hardware_evidence: true,
            min_test_samples: usize::MAX,
            min_test_environments: usize::MAX,
            min_samples_per_environment: usize::MAX,
            require_held_out_environment: true,
            require_subject_disjoint: true,
            require_reproducer: true,
            require_independent_validation: true,
            capabilities: BTreeMap::new(),
        }
    }
}

/// Repository-owned policy. The claim manifest references no thresholds;
/// thresholds come exclusively from this independently reviewed input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimPolicy {
    /// Exact policy schema version.
    pub schema_version: String,
    /// Stable, versioned policy identifier.
    pub policy_id: String,
    /// Exact manifest-byte SHA-256 values registered by claim id. Production
    /// and safety metadata must match this reviewer-controlled registry before
    /// it can reach `metadata_attested`. Research fixtures need no registration.
    pub registered_manifest_sha256: BTreeMap<String, String>,
    /// Evaluator identifiers permitted to submit production metadata. This is
    /// an allowlist, not authentication; a future signed trust root must prove
    /// possession before production release can be authorized.
    pub trusted_evaluators: Vec<String>,
    /// Exact reproducer argv vectors accepted by policy. Matching only the
    /// executable name would permit an unrelated `cargo` or `python3` command.
    pub allowed_reproducer_argv: Vec<Vec<String>>,
    /// Registered confidence-interval methods accepted for production metadata.
    pub allowed_confidence_methods: Vec<String>,
    /// Independent reviewers permitted by policy. The reviewer must differ
    /// from the primary evaluator for classes that require corroboration.
    pub trusted_independent_reviewers: Vec<String>,
    /// Policies keyed by `research`, `production`, and `safety_critical`.
    pub claim_classes: BTreeMap<String, ClaimClassPolicy>,
}

/// One auditable gate decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleResult {
    /// Stable rule identifier.
    pub rule_id: String,
    /// Whether the rule passed.
    pub passed: bool,
    /// Concrete reason with observed and required values.
    pub detail: String,
}

/// Release decision encoded in a receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateDecision {
    /// Production metadata cleared the lint policy but remains blocked pending
    /// artifact retrieval, authenticated evaluator attestation, and review.
    MetadataAttested,
    /// A research statement may proceed only with research limitations.
    ResearchOnly,
    /// The proposed claim is blocked.
    Denied,
}

/// Deterministic, machine-readable evidence receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateReceipt {
    /// Exact receipt schema version.
    pub schema_version: String,
    /// Stable claim identifier.
    pub claim_id: String,
    /// Capability evaluated.
    pub capability: String,
    /// Claim class evaluated.
    pub claim_class: ClaimClass,
    /// Source category seen by the gate.
    pub source_kind: EvidenceSourceKind,
    /// SHA-256 of the exact manifest bytes.
    pub manifest_sha256: String,
    /// Policy identifier.
    pub policy_id: String,
    /// SHA-256 of the exact policy bytes.
    pub policy_sha256: String,
    /// Final release decision.
    pub decision: GateDecision,
    /// True when every metadata rule passed. This is not release authority.
    pub metadata_gate_passed: bool,
    /// True only when the statement is releasable without further authority.
    /// Production and safety receipts emitted by v1 always set this to false.
    pub claim_releasable: bool,
    /// Ordered rule results.
    pub rules: Vec<RuleResult>,
}

/// Receipt plus a digest over its canonical compact JSON representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptEnvelope {
    /// The evaluated receipt.
    pub receipt: GateReceipt,
    /// SHA-256 of `serde_json::to_vec(receipt)`.
    pub receipt_sha256: String,
}

impl ReceiptEnvelope {
    fn new(receipt: GateReceipt) -> Result<Self, ClaimGateError> {
        let canonical = serde_json::to_vec(&receipt).map_err(ClaimGateError::ReceiptJson)?;
        Ok(Self {
            receipt_sha256: sha256_hex(&canonical),
            receipt,
        })
    }
}

struct RuleCollector {
    rules: Vec<RuleResult>,
}

impl RuleCollector {
    fn new() -> Self {
        Self { rules: Vec::new() }
    }

    fn push(&mut self, rule_id: impl Into<String>, passed: bool, detail: impl Into<String>) {
        self.rules.push(RuleResult {
            rule_id: rule_id.into(),
            passed,
            detail: detail.into(),
        });
    }

    fn passed(&self) -> bool {
        self.rules.iter().all(|r| r.passed)
    }
}

/// Parse two strict JSON inputs while binding the manifest to a release class
/// selected by a protected caller such as a CI directory or release workflow.
/// This prevents a production surface from self-labelling its evidence as
/// research.
pub fn evaluate_claim_json_for_class(
    manifest_json: &[u8],
    policy_json: &[u8],
    required_class: ClaimClass,
) -> Result<ReceiptEnvelope, ClaimGateError> {
    if manifest_json.len() > MAX_INPUT_BYTES {
        return Err(ClaimGateError::ManifestTooLarge {
            actual: manifest_json.len(),
            maximum: MAX_INPUT_BYTES,
        });
    }
    if policy_json.len() > MAX_INPUT_BYTES {
        return Err(ClaimGateError::PolicyTooLarge {
            actual: policy_json.len(),
            maximum: MAX_INPUT_BYTES,
        });
    }

    let manifest: ClaimManifest =
        serde_json::from_slice(manifest_json).map_err(ClaimGateError::ManifestJson)?;
    let policy: ClaimPolicy =
        serde_json::from_slice(policy_json).map_err(ClaimGateError::PolicyJson)?;
    evaluate_claim(
        &manifest,
        &policy,
        sha256_hex(manifest_json),
        sha256_hex(policy_json),
        required_class,
    )
}

fn evaluate_claim(
    manifest: &ClaimManifest,
    policy: &ClaimPolicy,
    manifest_sha256: String,
    policy_sha256: String,
    required_class: ClaimClass,
) -> Result<ReceiptEnvelope, ClaimGateError> {
    let mut gate = RuleCollector::new();

    gate.push(
        "manifest_schema",
        manifest.schema_version == CLAIM_SCHEMA_VERSION,
        format!(
            "observed `{}`, required `{CLAIM_SCHEMA_VERSION}`",
            manifest.schema_version
        ),
    );
    gate.push(
        "policy_schema",
        policy.schema_version == POLICY_SCHEMA_VERSION,
        format!(
            "observed `{}`, required `{POLICY_SCHEMA_VERSION}`",
            policy.schema_version
        ),
    );
    gate.push(
        "claim_identity",
        canonical_token(&manifest.claim_id)
            && canonical_token(&manifest.capability)
            && canonical_token(&policy.policy_id)
            && clean_text(&manifest.statement),
        "claim_id, capability, and policy_id must be canonical lowercase tokens; statement must be trimmed, non-blank, and control-free",
    );
    let class_binding_pass = required_class == manifest.claim_class;
    gate.push(
        "claim_class_binding",
        class_binding_pass,
        format!(
            "manifest class `{}`; protected caller requires {:?}",
            manifest.claim_class.tag(),
            required_class.tag()
        ),
    );
    let claim_surface_paths: BTreeSet<&str> = manifest
        .claim_surface_paths
        .iter()
        .map(String::as_str)
        .collect();
    let claim_surfaces_pass = claim_surface_paths.len() == manifest.claim_surface_paths.len()
        && manifest
            .claim_surface_paths
            .iter()
            .all(|path| valid_claim_surface(path))
        && (matches!(manifest.claim_class, ClaimClass::Research)
            || !manifest.claim_surface_paths.is_empty());
    gate.push(
        "claim_surface_binding",
        claim_surfaces_pass,
        format!(
            "{} unique protected surface paths; production/safety require at least one",
            claim_surface_paths.len()
        ),
    );

    let trusted_evaluators = normalized_policy_tokens(&policy.trusted_evaluators);
    let allowed_reproducer_argv = normalized_policy_argv(&policy.allowed_reproducer_argv);
    let allowed_confidence_methods = normalized_policy_tokens(&policy.allowed_confidence_methods);
    let trusted_independent_reviewers =
        normalized_policy_tokens(&policy.trusted_independent_reviewers);
    let release_class_enabled = ["production", "safety_critical"].iter().any(|class| {
        policy
            .claim_classes
            .get(*class)
            .is_some_and(|class_policy| class_policy.enabled)
    });
    let policy_allowlists_valid = trusted_evaluators.is_some()
        && allowed_reproducer_argv
            .as_ref()
            .is_some_and(|allowed| !release_class_enabled || !allowed.is_empty())
        && allowed_confidence_methods.is_some()
        && trusted_independent_reviewers.is_some();
    gate.push(
        "policy_allowlists",
        policy_allowlists_valid,
        "trusted evaluators, independent reviewers, exact reproducer argv vectors, and confidence methods must be non-empty, well-formed, and duplicate-free",
    );

    let manifest_registry_valid = policy
        .registered_manifest_sha256
        .iter()
        .all(|(claim_id, digest)| canonical_token(claim_id) && valid_sha256(digest));
    gate.push(
        "policy_manifest_registry",
        manifest_registry_valid,
        "registered claim ids must be canonical and every registered manifest digest must be a non-placeholder lower-case SHA-256",
    );
    let expected_manifest_sha256 = policy
        .registered_manifest_sha256
        .get(&manifest.claim_id)
        .map(String::as_str);
    let manifest_registration_pass = matches!(manifest.claim_class, ClaimClass::Research)
        || (manifest_registry_valid && expected_manifest_sha256 == Some(manifest_sha256.as_str()));
    gate.push(
        "registered_manifest",
        manifest_registration_pass,
        format!(
            "required for class `{}`; expected digest {:?}; observed digest `{manifest_sha256}`",
            manifest.claim_class.tag(),
            expected_manifest_sha256
        ),
    );

    let fallback = ClaimClassPolicy::deny_all();
    let class_policy = policy
        .claim_classes
        .get(manifest.claim_class.tag())
        .unwrap_or(&fallback);
    let class_exists = policy
        .claim_classes
        .contains_key(manifest.claim_class.tag());
    gate.push(
        "claim_class_enabled",
        class_exists && class_policy.enabled,
        format!(
            "claim class `{}` exists: {class_exists}; enabled: {}",
            manifest.claim_class.tag(),
            class_policy.enabled
        ),
    );

    let real_hardware = manifest.source.kind.is_real_hardware();
    let source_allowed = real_hardware || class_policy.allow_non_hardware;
    gate.push(
        "source_class",
        source_allowed,
        format!(
            "source `{}`; non-hardware allowed for `{}`: {}",
            manifest.source.kind.tag(),
            manifest.claim_class.tag(),
            class_policy.allow_non_hardware
        ),
    );

    let source_artifacts_distinct = match (
        &manifest.source.capture_artifact,
        &manifest.source.ground_truth_artifact,
    ) {
        (Some(capture), Some(ground_truth)) => capture.sha256 != ground_truth.sha256,
        _ => false,
    };
    let hardware_metadata_valid = clean_text_opt(&manifest.source.device_family)
        && manifest
            .source
            .device_id
            .as_deref()
            .is_some_and(canonical_token)
        && clean_text_opt(&manifest.source.firmware_version)
        && valid_artifact_opt(&manifest.source.capture_artifact)
        && valid_artifact_opt(&manifest.source.ground_truth_artifact)
        && source_artifacts_distinct;
    let hardware_pass = if real_hardware {
        hardware_metadata_valid
    } else {
        !class_policy.require_hardware_evidence
    };
    gate.push(
        "hardware_provenance",
        hardware_pass,
        if real_hardware {
            format!(
                "physical source metadata complete and capture/ground-truth digests valid and distinct: {hardware_metadata_valid}"
            )
        } else {
            format!(
                "source is not physical hardware; class requires hardware evidence: {}",
                class_policy.require_hardware_evidence
            )
        },
    );

    let train_envs = normalized_id_set(&manifest.evaluation.train_environment_ids);
    let test_envs = normalized_id_set(&manifest.evaluation.test_environment_ids);
    let env_lists_valid = train_envs.is_some() && test_envs.is_some();
    let env_disjoint = match (&train_envs, &test_envs) {
        (Some(train), Some(test)) => train.is_disjoint(test),
        _ => false,
    };
    let held_out_protocol = matches!(
        manifest.evaluation.protocol,
        EvaluationProtocol::HeldOutEnvironment
    );
    let protocol_pass = !class_policy.require_held_out_environment || held_out_protocol;
    gate.push(
        "held_out_environment_protocol",
        protocol_pass,
        format!(
            "protocol is held_out_environment: {held_out_protocol}; required: {}",
            class_policy.require_held_out_environment
        ),
    );
    let leakage_pass = if held_out_protocol {
        env_lists_valid
            && train_envs.as_ref().is_some_and(|ids| !ids.is_empty())
            && test_envs.as_ref().is_some_and(|ids| !ids.is_empty())
            && env_disjoint
    } else {
        env_lists_valid
    };
    gate.push(
        "environment_leakage",
        leakage_pass,
        format!(
            "environment identifiers unique/non-blank: {env_lists_valid}; train/test disjoint: {env_disjoint}"
        ),
    );
    let test_env_count = test_envs.as_ref().map_or(0, BTreeSet::len);
    let env_count_pass = test_env_count >= class_policy.min_test_environments;
    gate.push(
        "held_out_environment_count",
        env_count_pass,
        format!(
            "{} distinct test environments; minimum {}",
            test_env_count, class_policy.min_test_environments
        ),
    );

    let sample_count_pass = manifest.evaluation.test_samples >= class_policy.min_test_samples;
    gate.push(
        "minimum_test_samples",
        sample_count_pass,
        format!(
            "{} test samples; minimum {}",
            manifest.evaluation.test_samples, class_policy.min_test_samples
        ),
    );

    let subject_pass = if class_policy.require_subject_disjoint {
        let train_subjects = normalized_id_set(&manifest.evaluation.train_subject_ids);
        let test_subjects = normalized_id_set(&manifest.evaluation.test_subject_ids);
        match (train_subjects, test_subjects) {
            (Some(train), Some(test)) => {
                !train.is_empty() && !test.is_empty() && train.is_disjoint(&test)
            }
            _ => false,
        }
    } else {
        normalized_optional_id_set(&manifest.evaluation.train_subject_ids).is_some()
            && normalized_optional_id_set(&manifest.evaluation.test_subject_ids).is_some()
    };
    gate.push(
        "subject_leakage",
        subject_pass,
        format!(
            "subject-disjoint split required: {}; requirement satisfied: {subject_pass}",
            class_policy.require_subject_disjoint
        ),
    );

    let capability_policy = class_policy
        .capabilities
        .get(&manifest.capability)
        .or_else(|| {
            if matches!(manifest.claim_class, ClaimClass::Research) {
                class_policy.capabilities.get("*")
            } else {
                None
            }
        });

    let threshold_policy_pass = capability_policy.is_some()
        && (!manifest.claim_class.requires_thresholds()
            || capability_policy.is_some_and(|p| !p.metrics.is_empty()));
    gate.push(
        "pre_registered_thresholds",
        threshold_policy_pass,
        format!(
            "capability policy found: {}; non-empty thresholds required: {}",
            capability_policy.is_some(),
            manifest.claim_class.requires_thresholds()
        ),
    );

    let binary_counts_pass = capability_policy.is_none_or(|capability| {
        if !capability.require_binary_class_counts {
            return true;
        }
        match (
            manifest.evaluation.positive_test_samples,
            manifest.evaluation.negative_test_samples,
        ) {
            (Some(positive), Some(negative)) => {
                positive >= capability.min_positive_samples
                    && negative >= capability.min_negative_samples
                    && positive.checked_add(negative) == Some(manifest.evaluation.test_samples)
            }
            _ => false,
        }
    });
    gate.push(
        "binary_class_coverage",
        binary_counts_pass,
        match capability_policy {
            Some(capability) if capability.require_binary_class_counts => format!(
                "positive {:?} (minimum {}), negative {:?} (minimum {}), total {}",
                manifest.evaluation.positive_test_samples,
                capability.min_positive_samples,
                manifest.evaluation.negative_test_samples,
                capability.min_negative_samples,
                manifest.evaluation.test_samples
            ),
            _ => "binary class accounting not required by this capability policy".to_string(),
        },
    );

    let all_submitted_metrics_valid = manifest
        .metrics
        .iter()
        .all(|(name, metric)| canonical_token(name) && valid_metric(metric));
    gate.push(
        "metric_evidence_format",
        all_submitted_metrics_valid,
        "all submitted rate metrics must be finite in [0,1] and satisfy ci_lower <= point_estimate <= ci_upper when bounds are present",
    );
    let submitted_metric_names: BTreeSet<&str> =
        manifest.metrics.keys().map(String::as_str).collect();
    let registered_metric_names: BTreeSet<&str> = capability_policy
        .map(|capability| capability.metrics.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let metric_policy_binding_pass = matches!(manifest.claim_class, ClaimClass::Research)
        || submitted_metric_names == registered_metric_names;
    gate.push(
        "metric_policy_binding",
        metric_policy_binding_pass,
        format!(
            "submitted metrics {:?}; policy metrics {:?}; production/safety require an exact set",
            submitted_metric_names, registered_metric_names
        ),
    );

    let presence_consistency = if manifest.capability == "presence" {
        presence_metrics_consistent(&manifest.metrics)
    } else {
        true
    };
    gate.push(
        "metric_relationships",
        presence_consistency,
        "presence specificity + false_positive_rate must equal 1 within 1e-6 when both are submitted",
    );

    if let Some(capability) = capability_policy {
        for (metric_name, threshold) in &capability.metrics {
            let threshold_well_formed = valid_threshold(threshold)
                && (!manifest.claim_class.requires_thresholds()
                    || matches!(
                        (threshold.minimum, threshold.maximum, threshold.statistic),
                        (Some(_), None, ThresholdStatistic::LowerConfidenceBound)
                            | (None, Some(_), ThresholdStatistic::UpperConfidenceBound)
                    ));
            gate.push(
                format!("threshold_definition:{metric_name}"),
                threshold_well_formed,
                format!(
                    "minimum {:?}, maximum {:?}, statistic {:?}; production/safety require a confidence bound",
                    threshold.minimum, threshold.maximum, threshold.statistic
                ),
            );

            let metric = manifest.metrics.get(metric_name);
            let selected = metric.and_then(|m| selected_statistic(m, threshold.statistic));
            let observed_pass = threshold_well_formed
                && metric.is_some_and(valid_metric)
                && selected.is_some_and(|value| threshold_accepts(threshold, value));
            gate.push(
                format!("metric_threshold:{metric_name}"),
                observed_pass,
                format!(
                    "selected value {:?}; required minimum {:?}, maximum {:?}",
                    selected, threshold.minimum, threshold.maximum
                ),
            );
        }
    }

    let environment_result_ids = normalized_id_set(
        &manifest
            .evaluation
            .environment_results
            .iter()
            .map(|result| result.environment_id.clone())
            .collect::<Vec<_>>(),
    );
    let environment_ids_match = match (&environment_result_ids, &test_envs) {
        (Some(observed), Some(expected)) => observed == expected,
        _ => false,
    };
    let environment_sample_total = manifest
        .evaluation
        .environment_results
        .iter()
        .try_fold(0usize, |total, result| {
            total.checked_add(result.test_samples)
        });
    let environment_sample_counts_pass = environment_sample_total
        == Some(manifest.evaluation.test_samples)
        && manifest
            .evaluation
            .environment_results
            .iter()
            .all(|result| result.test_samples >= class_policy.min_samples_per_environment);
    let environment_binary_counts_pass = capability_policy.is_none_or(|capability| {
        if !capability.require_binary_class_counts {
            return true;
        }
        let positive_total = manifest
            .evaluation
            .environment_results
            .iter()
            .try_fold(0usize, |total, result| {
                total.checked_add(result.positive_test_samples?)
            });
        let negative_total = manifest
            .evaluation
            .environment_results
            .iter()
            .try_fold(0usize, |total, result| {
                total.checked_add(result.negative_test_samples?)
            });
        manifest
            .evaluation
            .environment_results
            .iter()
            .all(
                |result| match (result.positive_test_samples, result.negative_test_samples) {
                    (Some(positive), Some(negative)) => {
                        positive >= capability.min_positive_samples_per_environment
                            && negative >= capability.min_negative_samples_per_environment
                            && positive.checked_add(negative) == Some(result.test_samples)
                    }
                    _ => false,
                },
            )
            && positive_total == manifest.evaluation.positive_test_samples
            && negative_total == manifest.evaluation.negative_test_samples
    });
    let environment_metrics_pass = manifest
        .evaluation
        .environment_results
        .iter()
        .all(|result| {
            let metric_names: BTreeSet<&str> = result.metrics.keys().map(String::as_str).collect();
            result
                .metrics
                .iter()
                .all(|(name, metric)| canonical_token(name) && valid_metric(metric))
                && (!manifest.claim_class.requires_thresholds()
                    || metric_names == registered_metric_names)
                && (manifest.capability != "presence"
                    || presence_metrics_consistent(&result.metrics))
                && capability_policy.is_none_or(|capability| {
                    capability.metrics.iter().all(|(metric_name, threshold)| {
                        valid_threshold(threshold)
                            && result.metrics.get(metric_name).is_some_and(|metric| {
                                valid_metric(metric)
                                    && selected_statistic(metric, threshold.statistic)
                                        .is_some_and(|value| threshold_accepts(threshold, value))
                            })
                    })
                })
        });
    let per_environment_required = class_policy.require_held_out_environment;
    let per_environment_pass = if manifest.evaluation.environment_results.is_empty() {
        !per_environment_required
    } else {
        environment_ids_match
            && environment_sample_counts_pass
            && environment_binary_counts_pass
            && environment_metrics_pass
    };
    gate.push(
        "per_environment_evidence",
        per_environment_pass,
        format!(
            "required: {per_environment_required}; environments match held-out ids: {environment_ids_match}; sample total {:?} matches {} with minimum {} per environment: {environment_sample_counts_pass}; binary accounting: {environment_binary_counts_pass}; per-environment metrics: {environment_metrics_pass}",
            environment_sample_total,
            manifest.evaluation.test_samples,
            class_policy.min_samples_per_environment
        ),
    );

    let binding = manifest.evaluation_binding.as_ref();
    let binding_required = class_policy.require_reproducer || real_hardware;
    let binding_shape_pass = binding.is_some_and(|binding| {
        let artifact_digests = BTreeSet::from([
            binding.model_artifact.sha256.as_str(),
            binding.split_manifest_artifact.sha256.as_str(),
            binding.evaluation_report_artifact.sha256.as_str(),
        ]);
        canonical_token(&binding.evaluator_id)
            && canonical_token(&binding.confidence_method)
            && valid_git_sha(&binding.source_commit_sha)
            && valid_artifact(&binding.model_artifact)
            && valid_artifact(&binding.split_manifest_artifact)
            && valid_artifact(&binding.evaluation_report_artifact)
            && artifact_digests.len() == 3
            && valid_reproducer_argv(&binding.reproducer_argv)
    });
    let binding_pass = if binding_required {
        binding_shape_pass
    } else {
        binding.is_none() || binding_shape_pass
    };
    gate.push(
        "evaluation_binding",
        binding_pass,
        format!(
            "binding required: {binding_required}; commit, distinct model/split/report artifacts, evaluator, confidence method, and argv structurally valid: {binding_shape_pass}"
        ),
    );

    let evaluator_allowed = binding.is_some_and(|binding| {
        trusted_evaluators
            .as_ref()
            .is_some_and(|ids| ids.contains(&binding.evaluator_id))
    });
    let reproducer_allowed = binding.is_some_and(|binding| {
        allowed_reproducer_argv
            .as_ref()
            .is_some_and(|allowed| allowed.contains(&binding.reproducer_argv))
    });
    let confidence_method_allowed = binding.is_some_and(|binding| {
        allowed_confidence_methods
            .as_ref()
            .is_some_and(|methods| methods.contains(&binding.confidence_method))
    });
    let allowlist_pass = if binding_required {
        evaluator_allowed && reproducer_allowed && confidence_method_allowed
    } else {
        true
    };
    gate.push(
        "evaluation_allowlist",
        allowlist_pass,
        format!(
            "evaluator allowed: {evaluator_allowed}; exact reproducer argv allowed: {reproducer_allowed}; confidence method allowed: {confidence_method_allowed}"
        ),
    );

    let independent_pass = match &manifest.independent_validation {
        Some(validation) => {
            let review_digest = validation.report_artifact.sha256.as_str();
            let review_artifact_distinct = manifest
                .source
                .capture_artifact
                .as_ref()
                .is_none_or(|artifact| artifact.sha256.as_str() != review_digest)
                && manifest
                    .source
                    .ground_truth_artifact
                    .as_ref()
                    .is_none_or(|artifact| artifact.sha256.as_str() != review_digest)
                && binding.is_none_or(|binding| {
                    binding.model_artifact.sha256.as_str() != review_digest
                        && binding.split_manifest_artifact.sha256.as_str() != review_digest
                        && binding.evaluation_report_artifact.sha256.as_str() != review_digest
                });
            canonical_token(&validation.reviewer)
                && valid_artifact(&validation.report_artifact)
                && review_artifact_distinct
                && trusted_independent_reviewers
                    .as_ref()
                    .is_some_and(|reviewers| reviewers.contains(&validation.reviewer))
                && binding.is_none_or(|binding| binding.evaluator_id != validation.reviewer)
        }
        None => !class_policy.require_independent_validation,
    };
    gate.push(
        "independent_validation",
        independent_pass,
        format!(
            "independent validation required: {}; provided: {}",
            class_policy.require_independent_validation,
            manifest.independent_validation.is_some()
        ),
    );

    let mut role_digests = Vec::new();
    if let Some(artifact) = &manifest.source.capture_artifact {
        role_digests.push(artifact.sha256.as_str());
    }
    if let Some(artifact) = &manifest.source.ground_truth_artifact {
        role_digests.push(artifact.sha256.as_str());
    }
    if let Some(binding) = binding {
        role_digests.push(binding.model_artifact.sha256.as_str());
        role_digests.push(binding.split_manifest_artifact.sha256.as_str());
        role_digests.push(binding.evaluation_report_artifact.sha256.as_str());
    }
    if let Some(validation) = &manifest.independent_validation {
        role_digests.push(validation.report_artifact.sha256.as_str());
    }
    let distinct_role_digests: BTreeSet<&str> = role_digests.iter().copied().collect();
    let all_artifact_roles_distinct = distinct_role_digests.len() == role_digests.len();
    gate.push(
        "artifact_role_separation",
        all_artifact_roles_distinct,
        format!(
            "{} artifact roles supplied with {} distinct content digests",
            role_digests.len(),
            distinct_role_digests.len()
        ),
    );

    let metadata_gate_passed = gate.passed();
    let decision = if !metadata_gate_passed {
        GateDecision::Denied
    } else if matches!(manifest.claim_class, ClaimClass::Research) {
        GateDecision::ResearchOnly
    } else {
        GateDecision::MetadataAttested
    };
    // V1 has no authenticated evaluator signature or artifact retrieval path.
    // Production and safety metadata therefore never authorizes release.
    let claim_releasable =
        metadata_gate_passed && matches!(manifest.claim_class, ClaimClass::Research);

    ReceiptEnvelope::new(GateReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION.to_string(),
        claim_id: manifest.claim_id.clone(),
        capability: manifest.capability.clone(),
        claim_class: manifest.claim_class,
        source_kind: manifest.source.kind,
        manifest_sha256,
        policy_id: policy.policy_id.clone(),
        policy_sha256,
        decision,
        metadata_gate_passed,
        claim_releasable,
        rules: gate.rules,
    })
}

fn selected_statistic(metric: &MetricEvidence, statistic: ThresholdStatistic) -> Option<f64> {
    match statistic {
        ThresholdStatistic::PointEstimate => Some(metric.point_estimate),
        ThresholdStatistic::LowerConfidenceBound => metric.ci_lower,
        ThresholdStatistic::UpperConfidenceBound => metric.ci_upper,
    }
}

fn threshold_accepts(threshold: &MetricThreshold, value: f64) -> bool {
    threshold.minimum.is_none_or(|minimum| value >= minimum)
        && threshold.maximum.is_none_or(|maximum| value <= maximum)
}

fn valid_threshold(threshold: &MetricThreshold) -> bool {
    let one_boundary = threshold.minimum.is_some() ^ threshold.maximum.is_some();
    one_boundary
        && threshold.minimum.is_none_or(f64::is_finite)
        && threshold.maximum.is_none_or(f64::is_finite)
        && threshold
            .minimum
            .is_none_or(|value| (0.0..=1.0).contains(&value))
        && threshold
            .maximum
            .is_none_or(|value| (0.0..=1.0).contains(&value))
}

fn valid_metric(metric: &MetricEvidence) -> bool {
    if !metric.point_estimate.is_finite()
        || metric.ci_lower.is_some_and(|v| !v.is_finite())
        || metric.ci_upper.is_some_and(|v| !v.is_finite())
        || !(0.0..=1.0).contains(&metric.point_estimate)
        || metric
            .ci_lower
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        || metric
            .ci_upper
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
    {
        return false;
    }
    if metric
        .ci_lower
        .is_some_and(|lower| lower > metric.point_estimate)
        || metric
            .ci_upper
            .is_some_and(|upper| upper < metric.point_estimate)
    {
        return false;
    }
    match (metric.ci_lower, metric.ci_upper) {
        (Some(lower), Some(upper)) => lower <= upper,
        _ => true,
    }
}

fn presence_metrics_consistent(metrics: &BTreeMap<String, MetricEvidence>) -> bool {
    match (
        metrics.get("specificity"),
        metrics.get("false_positive_rate"),
    ) {
        (Some(specificity), Some(false_positive_rate)) => {
            (specificity.point_estimate + false_positive_rate.point_estimate - 1.0).abs() <= 1e-6
        }
        _ => true,
    }
}

fn normalized_id_set(values: &[String]) -> Option<BTreeSet<String>> {
    let mut normalized = BTreeSet::new();
    for value in values {
        if !clean_identifier(value) {
            return None;
        }
        if !normalized.insert(value.to_ascii_lowercase()) {
            return None;
        }
    }
    Some(normalized)
}

fn normalized_optional_id_set(values: &[String]) -> Option<BTreeSet<String>> {
    normalized_id_set(values)
}

fn normalized_policy_tokens(values: &[String]) -> Option<BTreeSet<String>> {
    if values.is_empty() || values.iter().any(|value| !canonical_token(value)) {
        return None;
    }
    let unique: BTreeSet<String> = values.iter().cloned().collect();
    (unique.len() == values.len()).then_some(unique)
}

fn normalized_policy_argv(values: &[Vec<String>]) -> Option<BTreeSet<Vec<String>>> {
    if values.iter().any(|argv| !valid_reproducer_argv(argv)) {
        return None;
    }
    let unique: BTreeSet<Vec<String>> = values.iter().cloned().collect();
    (unique.len() == values.len()).then_some(unique)
}

fn clean_identifier(value: &str) -> bool {
    value == value.trim()
        && !value.is_empty()
        && value.len() <= 256
        && value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
        })
}

fn canonical_token(value: &str) -> bool {
    clean_identifier(value)
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
        })
}

fn clean_text(value: &str) -> bool {
    value == value.trim()
        && !value.is_empty()
        && value.len() <= 4096
        && !value.chars().any(char::is_control)
}

fn clean_text_opt(value: &Option<String>) -> bool {
    value.as_deref().is_some_and(clean_text)
}

fn valid_claim_surface(value: &str) -> bool {
    clean_identifier(value)
        && !value.starts_with('/')
        && !value.contains('\\')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
        && (value == "README.md"
            || value.starts_with("benchmarks/")
            || value.starts_with("docs/benchmarks/")
            || value.starts_with("docs/releases/")
            || value.starts_with("docs/huggingface/"))
}

fn valid_artifact_opt(value: &Option<ArtifactDigest>) -> bool {
    value.as_ref().is_some_and(valid_artifact)
}

fn valid_artifact(value: &ArtifactDigest) -> bool {
    clean_text(&value.uri) && valid_sha256(&value.sha256)
}

fn valid_sha256(value: &str) -> bool {
    let alphabet: BTreeSet<u8> = value.bytes().collect();
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && alphabet.len() >= 8
}

fn valid_git_sha(value: &str) -> bool {
    let alphabet: BTreeSet<u8> = value.bytes().collect();
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && alphabet.len() >= 8
}

fn valid_reproducer_argv(argv: &[String]) -> bool {
    (2..=64).contains(&argv.len())
        && argv.iter().all(|arg| {
            !arg.is_empty()
                && arg.len() <= 1024
                && arg == arg.trim()
                && !arg.chars().any(char::is_control)
        })
        && argv.first().is_some_and(|command| canonical_token(command))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(name: &str) -> ArtifactDigest {
        ArtifactDigest {
            uri: format!("evidence://{name}"),
            sha256: sha256_hex(name.as_bytes()),
        }
    }

    fn metric(point: f64, lower: f64, upper: f64) -> MetricEvidence {
        MetricEvidence {
            point_estimate: point,
            ci_lower: Some(lower),
            ci_upper: Some(upper),
        }
    }

    fn presence_metrics() -> BTreeMap<String, MetricEvidence> {
        BTreeMap::from([
            ("roc_auc".to_string(), metric(0.96, 0.93, 0.98)),
            ("false_positive_rate".to_string(), metric(0.04, 0.02, 0.07)),
        ])
    }

    fn presence_policy() -> CapabilityPolicy {
        CapabilityPolicy {
            require_binary_class_counts: true,
            min_positive_samples: 60,
            min_negative_samples: 60,
            min_positive_samples_per_environment: 20,
            min_negative_samples_per_environment: 20,
            metrics: BTreeMap::from([
                (
                    "roc_auc".to_string(),
                    MetricThreshold {
                        minimum: Some(0.90),
                        maximum: None,
                        statistic: ThresholdStatistic::LowerConfidenceBound,
                    },
                ),
                (
                    "false_positive_rate".to_string(),
                    MetricThreshold {
                        minimum: None,
                        maximum: Some(0.10),
                        statistic: ThresholdStatistic::UpperConfidenceBound,
                    },
                ),
            ]),
        }
    }

    fn policy() -> ClaimPolicy {
        ClaimPolicy {
            schema_version: POLICY_SCHEMA_VERSION.to_string(),
            policy_id: "test-policy-v1".to_string(),
            registered_manifest_sha256: BTreeMap::new(),
            trusted_evaluators: vec!["ruview-ci".to_string()],
            allowed_reproducer_argv: vec![vec![
                "cargo".to_string(),
                "run".to_string(),
                "--bin".to_string(),
                "evaluate-presence".to_string(),
            ]],
            allowed_confidence_methods: vec!["bootstrap-percentile-95".to_string()],
            trusted_independent_reviewers: vec!["independent-lab".to_string()],
            claim_classes: BTreeMap::from([
                (
                    "research".to_string(),
                    ClaimClassPolicy {
                        enabled: true,
                        allow_non_hardware: true,
                        require_hardware_evidence: false,
                        min_test_samples: 30,
                        min_test_environments: 1,
                        min_samples_per_environment: 0,
                        require_held_out_environment: false,
                        require_subject_disjoint: false,
                        require_reproducer: false,
                        require_independent_validation: false,
                        capabilities: BTreeMap::from([(
                            "*".to_string(),
                            CapabilityPolicy {
                                require_binary_class_counts: false,
                                min_positive_samples: 0,
                                min_negative_samples: 0,
                                min_positive_samples_per_environment: 0,
                                min_negative_samples_per_environment: 0,
                                metrics: BTreeMap::new(),
                            },
                        )]),
                    },
                ),
                (
                    "production".to_string(),
                    ClaimClassPolicy {
                        enabled: true,
                        allow_non_hardware: false,
                        require_hardware_evidence: true,
                        min_test_samples: 300,
                        min_test_environments: 2,
                        min_samples_per_environment: 100,
                        require_held_out_environment: true,
                        require_subject_disjoint: false,
                        require_reproducer: true,
                        require_independent_validation: true,
                        capabilities: BTreeMap::from([("presence".to_string(), presence_policy())]),
                    },
                ),
                (
                    "safety_critical".to_string(),
                    ClaimClassPolicy {
                        enabled: false,
                        allow_non_hardware: false,
                        require_hardware_evidence: true,
                        min_test_samples: 1000,
                        min_test_environments: 3,
                        min_samples_per_environment: 250,
                        require_held_out_environment: true,
                        require_subject_disjoint: true,
                        require_reproducer: true,
                        require_independent_validation: true,
                        capabilities: BTreeMap::new(),
                    },
                ),
            ]),
        }
    }

    fn manifest(claim_class: ClaimClass, kind: EvidenceSourceKind) -> ClaimManifest {
        let is_hardware = kind.is_real_hardware();
        ClaimManifest {
            schema_version: CLAIM_SCHEMA_VERSION.to_string(),
            claim_id: "presence-heldout-001".to_string(),
            capability: "presence".to_string(),
            claim_class,
            statement: "Presence gate meets the registered production policy".to_string(),
            claim_surface_paths: (!matches!(claim_class, ClaimClass::Research))
                .then(|| vec!["README.md".to_string()])
                .unwrap_or_default(),
            source: SourceEvidence {
                kind,
                device_family: is_hardware.then(|| "esp32-s3".to_string()),
                device_id: is_hardware.then(|| "test-device-01".to_string()),
                firmware_version: is_hardware.then(|| "0.7.0".to_string()),
                capture_artifact: is_hardware.then(|| artifact("capture")),
                ground_truth_artifact: is_hardware.then(|| artifact("truth")),
            },
            evaluation: EvaluationEvidence {
                protocol: EvaluationProtocol::HeldOutEnvironment,
                train_environment_ids: vec!["room-a".into(), "room-b".into()],
                test_environment_ids: vec!["room-c".into(), "room-d".into()],
                environment_results: if matches!(claim_class, ClaimClass::Research) {
                    Vec::new()
                } else {
                    vec![
                        EnvironmentEvidence {
                            environment_id: "room-c".into(),
                            test_samples: 150,
                            positive_test_samples: Some(75),
                            negative_test_samples: Some(75),
                            metrics: presence_metrics(),
                        },
                        EnvironmentEvidence {
                            environment_id: "room-d".into(),
                            test_samples: 150,
                            positive_test_samples: Some(75),
                            negative_test_samples: Some(75),
                            metrics: presence_metrics(),
                        },
                    ]
                },
                train_subject_ids: vec!["subject-a".into()],
                test_subject_ids: vec!["subject-b".into()],
                test_samples: 300,
                positive_test_samples: Some(150),
                negative_test_samples: Some(150),
            },
            metrics: presence_metrics(),
            evaluation_binding: is_hardware.then(|| EvaluationBinding {
                evaluator_id: "ruview-ci".to_string(),
                confidence_method: "bootstrap-percentile-95".to_string(),
                source_commit_sha: sha256_hex(b"commit")[..40].to_string(),
                model_artifact: artifact("model"),
                split_manifest_artifact: artifact("split"),
                evaluation_report_artifact: artifact("report"),
                reproducer_argv: vec![
                    "cargo".to_string(),
                    "run".to_string(),
                    "--bin".to_string(),
                    "evaluate-presence".to_string(),
                ],
            }),
            independent_validation: is_hardware.then(|| IndependentValidation {
                reviewer: "independent-lab".to_string(),
                report_artifact: artifact("independent-review"),
            }),
        }
    }

    fn evaluate_struct(manifest: &ClaimManifest, policy: &ClaimPolicy) -> ReceiptEnvelope {
        let manifest_json = serde_json::to_vec(manifest).unwrap();
        let policy_json = serde_json::to_vec(policy).unwrap();
        evaluate_claim_json_for_class(&manifest_json, &policy_json, manifest.claim_class).unwrap()
    }

    fn policy_for(manifest: &ClaimManifest) -> ClaimPolicy {
        let mut policy = policy();
        if !matches!(manifest.claim_class, ClaimClass::Research) {
            let manifest_json = serde_json::to_vec(manifest).unwrap();
            policy
                .registered_manifest_sha256
                .insert(manifest.claim_id.clone(), sha256_hex(&manifest_json));
        }
        policy
    }

    fn evaluate_registered(manifest: &ClaimManifest) -> ReceiptEnvelope {
        evaluate_struct(manifest, &policy_for(manifest))
    }

    fn failed(receipt: &GateReceipt, rule_id: &str) -> bool {
        receipt
            .rules
            .iter()
            .any(|rule| rule.rule_id == rule_id && !rule.passed)
    }

    #[test]
    fn real_hardware_production_metadata_requires_manual_review() {
        let candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let receipt = evaluate_registered(&candidate);
        assert_eq!(receipt.receipt.decision, GateDecision::MetadataAttested);
        assert!(receipt.receipt.metadata_gate_passed);
        assert!(!receipt.receipt.claim_releasable);
    }

    #[test]
    fn unregistered_production_manifest_is_denied() {
        let candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let receipt = evaluate_struct(&candidate, &policy());
        assert!(failed(&receipt.receipt, "registered_manifest"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn protected_class_binding_rejects_research_downgrade() {
        let mut candidate = manifest(ClaimClass::Research, EvidenceSourceKind::Synthetic);
        candidate.evaluation.test_samples = 30;
        candidate.evaluation.test_environment_ids = vec!["sim-room".into()];
        candidate.metrics.clear();
        candidate.evaluation_binding = None;
        let manifest_json = serde_json::to_vec(&candidate).unwrap();
        let policy_json = serde_json::to_vec(&policy()).unwrap();
        let receipt =
            evaluate_claim_json_for_class(&manifest_json, &policy_json, ClaimClass::Production)
                .unwrap();
        assert!(failed(&receipt.receipt, "claim_class_binding"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn simulator_cannot_release_production_claim_even_with_perfect_metrics() {
        let mut candidate = manifest(ClaimClass::Production, EvidenceSourceKind::Simulator);
        candidate
            .metrics
            .insert("roc_auc".into(), metric(1.0, 1.0, 1.0));
        candidate
            .metrics
            .insert("false_positive_rate".into(), metric(0.0, 0.0, 0.0));
        let receipt = evaluate_registered(&candidate);
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
        assert!(failed(&receipt.receipt, "source_class"));
        assert!(failed(&receipt.receipt, "hardware_provenance"));
    }

    #[test]
    fn simulated_research_result_is_explicitly_research_only() {
        let mut candidate = manifest(ClaimClass::Research, EvidenceSourceKind::Synthetic);
        candidate.evaluation.test_samples = 30;
        candidate.evaluation.test_environment_ids = vec!["sim-room".into()];
        candidate.metrics.clear();
        candidate.evaluation_binding = None;
        let receipt = evaluate_registered(&candidate);
        assert_eq!(receipt.receipt.decision, GateDecision::ResearchOnly);
        assert!(receipt.receipt.claim_releasable);
    }

    #[test]
    fn held_out_environment_overlap_is_rejected() {
        let mut candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        candidate.evaluation.test_environment_ids = vec!["room-b".into(), "room-c".into()];
        let receipt = evaluate_registered(&candidate);
        assert!(failed(&receipt.receipt, "environment_leakage"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn environment_case_and_whitespace_aliases_are_rejected() {
        let mut case_alias = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        case_alias.evaluation.train_environment_ids = vec!["Room-A".into(), "room-b".into()];
        case_alias.evaluation.test_environment_ids = vec!["room-a".into(), "room-c".into()];
        let receipt = evaluate_registered(&case_alias);
        assert!(failed(&receipt.receipt, "environment_leakage"));

        let mut whitespace_alias =
            manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        whitespace_alias.evaluation.test_environment_ids = vec![" room-c".into(), "room-d".into()];
        let receipt = evaluate_registered(&whitespace_alias);
        assert!(failed(&receipt.receipt, "environment_leakage"));

        let mut unicode_alias =
            manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        unicode_alias.evaluation.test_environment_ids = vec!["røom-c".into(), "room-d".into()];
        let receipt = evaluate_registered(&unicode_alias);
        assert!(failed(&receipt.receipt, "environment_leakage"));
    }

    #[test]
    fn production_claim_requires_a_protected_surface_path() {
        let mut candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        candidate.claim_surface_paths.clear();
        let receipt = evaluate_registered(&candidate);
        assert!(failed(&receipt.receipt, "claim_surface_binding"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn small_sample_is_rejected() {
        let mut candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        candidate.evaluation.test_samples = 120;
        candidate.evaluation.positive_test_samples = Some(60);
        candidate.evaluation.negative_test_samples = Some(60);
        let receipt = evaluate_registered(&candidate);
        assert!(failed(&receipt.receipt, "minimum_test_samples"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn held_out_room_metrics_cannot_be_omitted_or_hidden_by_aggregate() {
        let mut missing = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        missing.evaluation.environment_results.clear();
        let receipt = evaluate_registered(&missing);
        assert!(failed(&receipt.receipt, "per_environment_evidence"));

        let mut weak_room = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        weak_room.evaluation.environment_results[0]
            .metrics
            .insert("roc_auc".into(), metric(0.80, 0.70, 0.90));
        let receipt = evaluate_registered(&weak_room);
        assert!(failed(&receipt.receipt, "per_environment_evidence"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);

        let mut split_classes =
            manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        split_classes.evaluation.environment_results[0].positive_test_samples = Some(150);
        split_classes.evaluation.environment_results[0].negative_test_samples = Some(0);
        split_classes.evaluation.environment_results[1].positive_test_samples = Some(0);
        split_classes.evaluation.environment_results[1].negative_test_samples = Some(150);
        let receipt = evaluate_registered(&split_classes);
        assert!(failed(&receipt.receipt, "per_environment_evidence"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn confidence_bound_not_point_estimate_controls_release() {
        let mut candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        candidate
            .metrics
            .insert("roc_auc".into(), metric(0.95, 0.89, 0.98));
        let receipt = evaluate_registered(&candidate);
        assert!(failed(&receipt.receipt, "metric_threshold:roc_auc"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn production_capability_without_policy_thresholds_fails_closed() {
        let candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let mut no_thresholds = policy_for(&candidate);
        no_thresholds
            .claim_classes
            .get_mut("production")
            .unwrap()
            .capabilities
            .get_mut("presence")
            .unwrap()
            .metrics
            .clear();
        let receipt = evaluate_struct(&candidate, &no_thresholds);
        assert!(failed(&receipt.receipt, "pre_registered_thresholds"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn relabelled_hardware_with_placeholder_digests_is_rejected() {
        let mut candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        candidate.source.capture_artifact.as_mut().unwrap().sha256 = "a".repeat(64);
        let receipt = evaluate_registered(&candidate);
        assert!(failed(&receipt.receipt, "hardware_provenance"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn artifact_roles_require_distinct_content() {
        let mut source_alias =
            manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let capture_digest = source_alias
            .source
            .capture_artifact
            .as_ref()
            .unwrap()
            .sha256
            .clone();
        source_alias
            .source
            .ground_truth_artifact
            .as_mut()
            .unwrap()
            .sha256 = capture_digest;
        let receipt = evaluate_registered(&source_alias);
        assert!(failed(&receipt.receipt, "hardware_provenance"));

        let mut review_alias =
            manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let evaluation_digest = review_alias
            .evaluation_binding
            .as_ref()
            .unwrap()
            .evaluation_report_artifact
            .sha256
            .clone();
        review_alias
            .independent_validation
            .as_mut()
            .unwrap()
            .report_artifact
            .sha256 = evaluation_digest;
        let receipt = evaluate_registered(&review_alias);
        assert!(failed(&receipt.receipt, "independent_validation"));

        let mut cross_role_alias =
            manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let capture_digest = cross_role_alias
            .source
            .capture_artifact
            .as_ref()
            .unwrap()
            .sha256
            .clone();
        cross_role_alias
            .evaluation_binding
            .as_mut()
            .unwrap()
            .model_artifact
            .sha256 = capture_digest;
        let receipt = evaluate_registered(&cross_role_alias);
        assert!(failed(&receipt.receipt, "artifact_role_separation"));
    }

    #[test]
    fn echo_ok_reproducer_is_rejected_even_with_trusted_evaluator() {
        let mut candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let binding = candidate.evaluation_binding.as_mut().unwrap();
        binding.reproducer_argv = vec!["echo".into(), "ok".into()];
        let receipt = evaluate_registered(&candidate);
        assert!(failed(&receipt.receipt, "evaluation_allowlist"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn altered_model_digest_is_rejected() {
        let baseline = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let registered_policy = policy_for(&baseline);
        let mut candidate = baseline.clone();
        candidate
            .evaluation_binding
            .as_mut()
            .unwrap()
            .model_artifact
            .sha256 = artifact("different-model").sha256;
        let receipt = evaluate_struct(&candidate, &registered_policy);
        assert!(!failed(&receipt.receipt, "evaluation_binding"));
        assert!(failed(&receipt.receipt, "registered_manifest"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn altered_metrics_and_sample_counts_break_registration() {
        let baseline = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let registered_policy = policy_for(&baseline);
        let mut candidate = baseline.clone();
        candidate.evaluation.test_samples = 320;
        candidate.evaluation.positive_test_samples = Some(160);
        candidate.evaluation.negative_test_samples = Some(160);
        candidate
            .metrics
            .insert("roc_auc".into(), metric(0.97, 0.94, 0.99));
        let receipt = evaluate_struct(&candidate, &registered_policy);
        assert!(failed(&receipt.receipt, "registered_manifest"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn exaggerated_statement_breaks_protected_registration() {
        let baseline = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let registered_policy = policy_for(&baseline);
        let mut candidate = baseline.clone();
        candidate.statement = "Presence accuracy is 99.999 percent everywhere".into();
        let receipt = evaluate_struct(&candidate, &registered_policy);
        assert!(failed(&receipt.receipt, "registered_manifest"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
        assert!(!receipt.receipt.claim_releasable);
    }

    #[test]
    fn unsigned_witness_hash_is_not_accepted_as_evidence() {
        let candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let mut value = serde_json::to_value(candidate).unwrap();
        value["witness_sha256"] = serde_json::json!(sha256_hex(b"unsigned-witness"));
        let policy_json = serde_json::to_vec(&policy()).unwrap();
        let error = evaluate_claim_json_for_class(
            &serde_json::to_vec(&value).unwrap(),
            &policy_json,
            ClaimClass::Production,
        )
        .unwrap_err();
        assert!(matches!(error, ClaimGateError::ManifestJson(_)));
    }

    #[test]
    fn out_of_domain_metric_is_rejected() {
        let mut candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        candidate
            .metrics
            .insert("roc_auc".into(), metric(1.1, 0.95, 1.2));
        let receipt = evaluate_registered(&candidate);
        assert!(failed(&receipt.receipt, "metric_evidence_format"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn unregistered_metric_cannot_be_used_for_a_production_statement() {
        let mut candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        candidate
            .metrics
            .insert("accuracy".into(), metric(1.0, 1.0, 1.0));
        let receipt = evaluate_registered(&candidate);
        assert!(failed(&receipt.receipt, "metric_policy_binding"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn safety_claims_are_disabled_by_default_policy() {
        let candidate = manifest(ClaimClass::SafetyCritical, EvidenceSourceKind::LiveHardware);
        let receipt = evaluate_registered(&candidate);
        assert!(failed(&receipt.receipt, "claim_class_enabled"));
        assert_eq!(receipt.receipt.decision, GateDecision::Denied);
    }

    #[test]
    fn receipt_is_deterministic_and_content_addressed() {
        let candidate = manifest(ClaimClass::Production, EvidenceSourceKind::RecordedHardware);
        let registered_policy = policy_for(&candidate);
        let first = evaluate_struct(&candidate, &registered_policy);
        let second = evaluate_struct(&candidate, &registered_policy);
        assert_eq!(first, second);
        let receipt_json = serde_json::to_vec(&first.receipt).unwrap();
        assert_eq!(first.receipt_sha256, sha256_hex(&receipt_json));
    }

    #[test]
    fn repository_policy_and_fixture_parse_and_gate() {
        let policy_json =
            include_bytes!("../../../../evidence/policies/sensing-claim-policy-v1.json");
        let manifest_json = include_bytes!("../../../../evidence/fixtures/research-synthetic.json");
        let receipt =
            evaluate_claim_json_for_class(manifest_json, policy_json, ClaimClass::Research)
                .unwrap();
        assert_eq!(receipt.receipt.decision, GateDecision::ResearchOnly);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let candidate = manifest(ClaimClass::Research, EvidenceSourceKind::Synthetic);
        let mut value = serde_json::to_value(candidate).unwrap();
        value["unsupported_override"] = serde_json::json!(true);
        let policy_json = serde_json::to_vec(&policy()).unwrap();
        let error = evaluate_claim_json_for_class(
            &serde_json::to_vec(&value).unwrap(),
            &policy_json,
            ClaimClass::Research,
        )
        .unwrap_err();
        assert!(matches!(error, ClaimGateError::ManifestJson(_)));
    }
}
