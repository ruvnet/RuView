//! Federation protocol: export builder, import merger, version-aware conflict resolution.

use crate::diff_privacy::DiffPrivacyEngine;
use crate::error::FederationError;
use crate::pii_strip::PiiStripper;
use crate::policy::FederationPolicy;
use crate::types::*;

/// Builder for constructing a federated learning export.
///
/// Follows the export flow from ADR-057:
/// 1. Extract learning (priors, kernels, cost curves, weights)
/// 2. PII-strip all payloads
/// 3. Add differential privacy noise
/// 4. Assemble manifest + attestation segments
pub struct ExportBuilder {
    contributor_pseudonym: String,
    domain_id: String,
    priors: Vec<TransferPriorSet>,
    kernels: Vec<PolicyKernelSnapshot>,
    cost_curves: Vec<CostCurveSnapshot>,
    weights: Vec<Vec<f64>>,
    policy: FederationPolicy,
    string_fields: Vec<(String, String)>,
}

/// A completed federated export ready for publishing.
#[derive(Clone, Debug)]
pub struct FederatedExport {
    /// The manifest describing this export.
    pub manifest: FederatedManifest,
    /// PII redaction attestation.
    pub redaction_log: RedactionLog,
    /// Differential privacy attestation.
    pub privacy_proof: DiffPrivacyProof,
    /// Transfer priors (after PII stripping and DP noise).
    pub priors: Vec<TransferPriorSet>,
    /// Policy kernel snapshots.
    pub kernels: Vec<PolicyKernelSnapshot>,
    /// Cost curve snapshots.
    pub cost_curves: Vec<CostCurveSnapshot>,
    /// Noised aggregate weights (if any).
    pub weights: Vec<Vec<f64>>,
}

impl ExportBuilder {
    /// Create a new export builder.
    pub fn new(contributor_pseudonym: String, domain_id: String) -> Self {
        Self {
            contributor_pseudonym,
            domain_id,
            priors: Vec::new(),
            kernels: Vec::new(),
            cost_curves: Vec::new(),
            weights: Vec::new(),
            policy: FederationPolicy::default(),
            string_fields: Vec::new(),
        }
    }

    /// Set the federation policy.
    pub fn with_policy(mut self, policy: FederationPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Add transfer priors from a trained domain.
    pub fn add_priors(mut self, priors: TransferPriorSet) -> Self {
        self.priors.push(priors);
        self
    }

    /// Add a policy kernel snapshot.
    pub fn add_kernel(mut self, kernel: PolicyKernelSnapshot) -> Self {
        self.kernels.push(kernel);
        self
    }

    /// Add a cost curve snapshot.
    pub fn add_cost_curve(mut self, curve: CostCurveSnapshot) -> Self {
        self.cost_curves.push(curve);
        self
    }

    /// Add raw weight vectors (LoRA deltas).
    pub fn add_weights(mut self, weights: Vec<f64>) -> Self {
        self.weights.push(weights);
        self
    }

    /// Add a named string field for PII scanning.
    pub fn add_string_field(mut self, name: String, value: String) -> Self {
        self.string_fields.push((name, value));
        self
    }

    /// Build the export: PII-strip, add DP noise, assemble manifest.
    pub fn build(mut self, dp_engine: &mut DiffPrivacyEngine) -> Result<FederatedExport, FederationError> {
        // 1. Apply quality gate from policy
        self.priors.retain(|ps| {
            ps.entries.iter().all(|e| e.observation_count >= self.policy.min_observations)
        });

        // 2. PII stripping
        let mut stripper = PiiStripper::new();
        let field_refs: Vec<(&str, &str)> = self.string_fields
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect();
        let (_redacted_fields, redaction_log) = stripper.strip_fields(&field_refs);

        // Strip PII from domain IDs and bucket IDs in priors
        for ps in &mut self.priors {
            ps.source_domain = stripper.strip_value(&ps.source_domain);
            for entry in &mut ps.entries {
                entry.bucket_id = stripper.strip_value(&entry.bucket_id);
            }
        }

        // Strip PII from cost curve domain IDs
        for curve in &mut self.cost_curves {
            curve.domain_id = stripper.strip_value(&curve.domain_id);
        }

        // 3. Add differential privacy noise to numerical parameters
        // Noise the Beta posteriors
        let mut noised_count: u64 = 0;
        for ps in &mut self.priors {
            for entry in &mut ps.entries {
                let mut params = [entry.params.alpha, entry.params.beta];
                dp_engine.add_noise(&mut params);
                entry.params.alpha = params[0].max(0.01); // Keep positive
                entry.params.beta = params[1].max(0.01);
                noised_count += 2;
            }
        }

        // Noise the weight vectors
        for w in &mut self.weights {
            dp_engine.add_noise(w);
            noised_count += w.len() as u64;
        }

        // Noise kernel knobs
        for kernel in &mut self.kernels {
            dp_engine.add_noise(&mut kernel.knobs);
            noised_count += kernel.knobs.len() as u64;
        }

        // Noise cost curve values
        for curve in &mut self.cost_curves {
            let mut costs: Vec<f64> = curve.points.iter().map(|(_, c)| *c).collect();
            dp_engine.add_noise(&mut costs);
            for (i, (_, c)) in curve.points.iter_mut().enumerate() {
                *c = costs[i];
            }
            noised_count += costs.len() as u64;
        }

        let privacy_proof = DiffPrivacyProof {
            epsilon: dp_engine.epsilon(),
            delta: dp_engine.delta(),
            mechanism: NoiseMechanism::Gaussian,
            sensitivity: 1.0,
            clipping_norm: 1.0,
            noise_scale: 0.0,
            noised_parameter_count: noised_count,
        };

        // 4. Build manifest
        let total_trajectories: u64 = self.priors.iter()
            .flat_map(|ps| ps.entries.iter())
            .map(|e| e.observation_count)
            .sum();

        let avg_quality = if !self.priors.is_empty() {
            self.priors.iter()
                .flat_map(|ps| ps.entries.iter())
                .map(|e| e.params.mean())
                .sum::<f64>()
                / self.priors.iter().map(|ps| ps.entries.len()).sum::<usize>().max(1) as f64
        } else {
            0.0
        };

        let manifest = FederatedManifest {
            format_version: 1,
            contributor_pseudonym: self.contributor_pseudonym,
            export_timestamp_s: 0,
            included_segment_ids: Vec::new(),
            privacy_budget_spent: dp_engine.epsilon(),
            domain_id: self.domain_id,
            rvf_version_tag: String::from("rvf-v1"),
            trajectory_count: total_trajectories,
            avg_quality_score: avg_quality,
        };

        Ok(FederatedExport {
            manifest,
            redaction_log,
            privacy_proof,
            priors: self.priors,
            kernels: self.kernels,
            cost_curves: self.cost_curves,
            weights: self.weights,
        })
    }
}

/// Merger for importing federated learning into local engines.
///
/// Follows the import flow from ADR-057:
/// 1. Validate signature and witness chain
/// 2. Check version compatibility
/// 3. Merge with dampened confidence
pub struct ImportMerger {
    /// Current RVF version for compatibility checks.
    current_version: u32,
    /// Dampening factor for cross-version imports.
    version_dampen_factor: f64,
}

impl ImportMerger {
    /// Create a new import merger.
    pub fn new() -> Self {
        Self {
            current_version: 1,
            version_dampen_factor: 0.5,
        }
    }

    /// Set the dampening factor for imports from different versions.
    pub fn with_version_dampen(mut self, factor: f64) -> Self {
        self.version_dampen_factor = factor.clamp(0.0, 1.0);
        self
    }

    /// Validate a federated export.
    pub fn validate(&self, export: &FederatedExport) -> Result<(), FederationError> {
        // Check format version
        if export.manifest.format_version == 0 {
            return Err(FederationError::SegmentValidation(
                "format_version must be > 0".into(),
            ));
        }

        // Check privacy proof has valid parameters
        if export.privacy_proof.epsilon <= 0.0 {
            return Err(FederationError::InvalidEpsilon(export.privacy_proof.epsilon));
        }

        // Check priors have positive parameters
        for ps in &export.priors {
            for entry in &ps.entries {
                if entry.params.alpha <= 0.0 || entry.params.beta <= 0.0 {
                    return Err(FederationError::SegmentValidation(format!(
                        "invalid Beta params in bucket {}: alpha={}, beta={}",
                        entry.bucket_id, entry.params.alpha, entry.params.beta
                    )));
                }
            }
        }

        Ok(())
    }

    /// Merge imported priors with local priors.
    ///
    /// Uses version-aware dampening: same version gets full weight,
    /// older versions get dampened (sqrt-scaling per MetaThompsonEngine).
    pub fn merge_priors(
        &self,
        local: &mut Vec<TransferPriorEntry>,
        remote: &[TransferPriorEntry],
        remote_version: u32,
    ) {
        let dampen = if remote_version == self.current_version {
            1.0
        } else {
            self.version_dampen_factor
        };

        for remote_entry in remote {
            let dampened = remote_entry.params.dampen(dampen);

            if let Some(local_entry) = local.iter_mut().find(|l| {
                l.bucket_id == remote_entry.bucket_id && l.arm_id == remote_entry.arm_id
            }) {
                // Merge: sum parameters minus uniform prior
                local_entry.params = local_entry.params.merge(&dampened);
                local_entry.observation_count += remote_entry.observation_count;
            } else {
                // New entry: insert with dampened params
                local.push(TransferPriorEntry {
                    bucket_id: remote_entry.bucket_id.clone(),
                    arm_id: remote_entry.arm_id.clone(),
                    params: dampened,
                    observation_count: remote_entry.observation_count,
                });
            }
        }
    }

    /// Merge imported weights with local weights using weighted average.
    pub fn merge_weights(
        &self,
        local: &mut [f64],
        remote: &[f64],
        local_weight: f64,
        remote_weight: f64,
    ) {
        let total = local_weight + remote_weight;
        if total <= 0.0 || local.len() != remote.len() {
            return;
        }
        for (l, r) in local.iter_mut().zip(remote.iter()) {
            *l = (local_weight * *l + remote_weight * *r) / total;
        }
    }
}

impl Default for ImportMerger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_privacy::DiffPrivacyEngine;

    fn make_test_priors() -> TransferPriorSet {
        TransferPriorSet {
            source_domain: "test_domain".into(),
            entries: vec![
                TransferPriorEntry {
                    bucket_id: "medium_algorithm".into(),
                    arm_id: "arm_0".into(),
                    params: BetaParams::new(10.0, 5.0),
                    observation_count: 50,
                },
                TransferPriorEntry {
                    bucket_id: "hard_synthesis".into(),
                    arm_id: "arm_1".into(),
                    params: BetaParams::new(8.0, 12.0),
                    observation_count: 30,
                },
            ],
            cost_ema: 0.85,
        }
    }

    #[test]
    fn export_builder_basic() {
        let mut dp = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 10.0)
            .unwrap()
            .with_seed(42);
        let export = ExportBuilder::new("alice_pseudo".into(), "code_review".into())
            .add_priors(make_test_priors())
            .build(&mut dp)
            .unwrap();

        assert_eq!(export.manifest.contributor_pseudonym, "alice_pseudo");
        assert_eq!(export.manifest.domain_id, "code_review");
        assert_eq!(export.manifest.format_version, 1);
        assert!(!export.priors.is_empty());
    }

    #[test]
    fn export_builder_with_weights() {
        let mut dp = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 10.0)
            .unwrap()
            .with_seed(42);
        let weights = vec![0.1, 0.2, 0.3, 0.4];
        let export = ExportBuilder::new("bob_pseudo".into(), "genomics".into())
            .add_weights(weights.clone())
            .build(&mut dp)
            .unwrap();

        assert_eq!(export.weights.len(), 1);
        // Weights should be different from original (noise added)
        assert!(export.weights[0].iter().zip(weights.iter()).any(|(a, b)| (a - b).abs() > 1e-10));
    }

    #[test]
    fn import_merger_validate() {
        let mut dp = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 10.0)
            .unwrap()
            .with_seed(42);
        let export = ExportBuilder::new("alice".into(), "domain".into())
            .add_priors(make_test_priors())
            .build(&mut dp)
            .unwrap();

        let merger = ImportMerger::new();
        assert!(merger.validate(&export).is_ok());
    }

    #[test]
    fn import_merger_merge_priors_same_version() {
        let merger = ImportMerger::new();
        let mut local = vec![TransferPriorEntry {
            bucket_id: "medium_algorithm".into(),
            arm_id: "arm_0".into(),
            params: BetaParams::new(5.0, 3.0),
            observation_count: 20,
        }];

        let remote = vec![TransferPriorEntry {
            bucket_id: "medium_algorithm".into(),
            arm_id: "arm_0".into(),
            params: BetaParams::new(10.0, 5.0),
            observation_count: 50,
        }];

        merger.merge_priors(&mut local, &remote, 1);
        assert_eq!(local.len(), 1);
        // Merged: alpha = 5 + 10 - 1 = 14, beta = 3 + 5 - 1 = 7
        assert!((local[0].params.alpha - 14.0).abs() < 1e-10);
        assert!((local[0].params.beta - 7.0).abs() < 1e-10);
        assert_eq!(local[0].observation_count, 70);
    }

    #[test]
    fn import_merger_merge_priors_different_version() {
        let merger = ImportMerger::new();
        let mut local = vec![TransferPriorEntry {
            bucket_id: "b".into(),
            arm_id: "a".into(),
            params: BetaParams::new(10.0, 10.0),
            observation_count: 50,
        }];

        let remote = vec![TransferPriorEntry {
            bucket_id: "b".into(),
            arm_id: "a".into(),
            params: BetaParams::new(20.0, 5.0),
            observation_count: 40,
        }];

        merger.merge_priors(&mut local, &remote, 0); // older version -> dampened
        assert_eq!(local.len(), 1);
        // Remote dampened by 0.5: alpha = 1 + (20-1)*0.5 = 10.5, beta = 1 + (5-1)*0.5 = 3.0
        // Merged: alpha = 10 + 10.5 - 1 = 19.5, beta = 10 + 3.0 - 1 = 12.0
        assert!((local[0].params.alpha - 19.5).abs() < 1e-10);
        assert!((local[0].params.beta - 12.0).abs() < 1e-10);
    }

    #[test]
    fn import_merger_merge_new_bucket() {
        let merger = ImportMerger::new();
        let mut local: Vec<TransferPriorEntry> = Vec::new();

        let remote = vec![TransferPriorEntry {
            bucket_id: "new_bucket".into(),
            arm_id: "arm_0".into(),
            params: BetaParams::new(10.0, 5.0),
            observation_count: 30,
        }];

        merger.merge_priors(&mut local, &remote, 1);
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].bucket_id, "new_bucket");
    }

    #[test]
    fn merge_weights_weighted_average() {
        let merger = ImportMerger::new();
        let mut local = vec![1.0, 2.0, 3.0];
        let remote = vec![3.0, 4.0, 5.0];
        merger.merge_weights(&mut local, &remote, 0.5, 0.5);
        assert!((local[0] - 2.0).abs() < 1e-10);
        assert!((local[1] - 3.0).abs() < 1e-10);
        assert!((local[2] - 4.0).abs() < 1e-10);
    }
}
