//! RVF Integration Bridge
//!
//! Connects the domain expansion engine to the RuVector Format (RVF):
//! - Serializes `TransferPrior`, `PolicyKernel`, `CostCurve` into RVF segments
//! - Creates SHAKE-256 witness chains for transfer verification
//! - Packages domain expansion artifacts into AGI container TLV entries
//! - Bridges priors to/from the rvf-solver-wasm `PolicyKernel`
//!
//! Requires the `rvf` feature to be enabled.

use rvf_types::{SegmentFlags, SegmentType};
use rvf_wire::reader::{read_segment, validate_segment};
use rvf_wire::writer::write_segment;

use crate::cost_curve::{AccelerationScoreboard, CostCurve};
use crate::domain::DomainId;
use crate::policy_kernel::PolicyKernel;
use crate::transfer::{ArmId, BetaParams, ContextBucket, MetaThompsonEngine, TransferPrior};

// ─── Wire-format wrappers ───────────────────────────────────────────────────
//
// JSON requires string keys for objects. TransferPrior uses HashMap<ContextBucket, _>
// which can't be directly serialized. These wrappers convert to/from Vec<(K,V)> form.

/// Wire-format representation of a TransferPrior (JSON-safe).
#[derive(serde::Serialize, serde::Deserialize)]
struct WireTransferPrior {
    source_domain: DomainId,
    bucket_priors: Vec<(ContextBucket, Vec<(ArmId, BetaParams)>)>,
    cost_ema_priors: Vec<(ContextBucket, f32)>,
    training_cycles: u64,
    witness_hash: String,
}

impl From<&TransferPrior> for WireTransferPrior {
    fn from(p: &TransferPrior) -> Self {
        Self {
            source_domain: p.source_domain.clone(),
            bucket_priors: p
                .bucket_priors
                .iter()
                .map(|(b, arms)| {
                    let arm_vec: Vec<(ArmId, BetaParams)> =
                        arms.iter().map(|(a, bp)| (a.clone(), bp.clone())).collect();
                    (b.clone(), arm_vec)
                })
                .collect(),
            cost_ema_priors: p
                .cost_ema_priors
                .iter()
                .map(|(b, c)| (b.clone(), *c))
                .collect(),
            training_cycles: p.training_cycles,
            witness_hash: p.witness_hash.clone(),
        }
    }
}

impl From<WireTransferPrior> for TransferPrior {
    fn from(w: WireTransferPrior) -> Self {
        let mut bucket_priors = std::collections::HashMap::new();
        for (bucket, arms) in w.bucket_priors {
            let arm_map: std::collections::HashMap<ArmId, BetaParams> = arms.into_iter().collect();
            bucket_priors.insert(bucket, arm_map);
        }
        let cost_ema_priors: std::collections::HashMap<ContextBucket, f32> =
            w.cost_ema_priors.into_iter().collect();
        Self {
            source_domain: w.source_domain,
            bucket_priors,
            cost_ema_priors,
            training_cycles: w.training_cycles,
            witness_hash: w.witness_hash,
        }
    }
}

/// Wire-format representation of a PolicyKernel (JSON-safe).
#[derive(serde::Serialize, serde::Deserialize)]
struct WirePolicyKernel {
    id: String,
    knobs: crate::policy_kernel::PolicyKnobs,
    holdout_scores: Vec<(DomainId, f32)>,
    total_cost: f32,
    cycles: u64,
    generation: u32,
    parent_id: Option<String>,
    replay_verified: bool,
}

impl From<&PolicyKernel> for WirePolicyKernel {
    fn from(k: &PolicyKernel) -> Self {
        Self {
            id: k.id.clone(),
            knobs: k.knobs.clone(),
            holdout_scores: k
                .holdout_scores
                .iter()
                .map(|(d, s)| (d.clone(), *s))
                .collect(),
            total_cost: k.total_cost,
            cycles: k.cycles,
            generation: k.generation,
            parent_id: k.parent_id.clone(),
            replay_verified: k.replay_verified,
        }
    }
}

impl From<WirePolicyKernel> for PolicyKernel {
    fn from(w: WirePolicyKernel) -> Self {
        Self {
            id: w.id,
            knobs: w.knobs,
            holdout_scores: w.holdout_scores.into_iter().collect(),
            total_cost: w.total_cost,
            cycles: w.cycles,
            generation: w.generation,
            parent_id: w.parent_id,
            replay_verified: w.replay_verified,
        }
    }
}

// ─── Segment serialization ──────────────────────────────────────────────────

/// Serialize a `TransferPrior` into an RVF TRANSFER_PRIOR segment.
///
/// Wire format: JSON payload (using Vec-of-tuples for map keys) inside a
/// 64-byte-aligned RVF segment. Type: `SegmentType::TransferPrior` (0x30).
pub fn transfer_prior_to_segment(prior: &TransferPrior, segment_id: u64) -> Vec<u8> {
    let wire: WireTransferPrior = prior.into();
    let payload = serde_json::to_vec(&wire).expect("WireTransferPrior serialization cannot fail");
    write_segment(
        SegmentType::TransferPrior as u8,
        &payload,
        SegmentFlags::empty(),
        segment_id,
    )
}

/// Deserialize a `TransferPrior` from an RVF segment's raw bytes.
///
/// Validates the segment header, checks the content hash, and deserializes
/// the JSON payload.
pub fn transfer_prior_from_segment(data: &[u8]) -> Result<TransferPrior, RvfBridgeError> {
    let (header, payload) = read_segment(data).map_err(RvfBridgeError::Rvf)?;
    if header.seg_type != SegmentType::TransferPrior as u8 {
        return Err(RvfBridgeError::WrongSegmentType {
            expected: SegmentType::TransferPrior as u8,
            got: header.seg_type,
        });
    }
    validate_segment(&header, payload).map_err(RvfBridgeError::Rvf)?;
    let wire: WireTransferPrior = serde_json::from_slice(payload).map_err(RvfBridgeError::Json)?;
    Ok(wire.into())
}

/// Serialize a `PolicyKernel` into an RVF POLICY_KERNEL segment.
pub fn policy_kernel_to_segment(kernel: &PolicyKernel, segment_id: u64) -> Vec<u8> {
    let wire: WirePolicyKernel = kernel.into();
    let payload = serde_json::to_vec(&wire).expect("WirePolicyKernel serialization cannot fail");
    write_segment(
        SegmentType::PolicyKernel as u8,
        &payload,
        SegmentFlags::empty(),
        segment_id,
    )
}

/// Deserialize a `PolicyKernel` from an RVF segment.
pub fn policy_kernel_from_segment(data: &[u8]) -> Result<PolicyKernel, RvfBridgeError> {
    let (header, payload) = read_segment(data).map_err(RvfBridgeError::Rvf)?;
    if header.seg_type != SegmentType::PolicyKernel as u8 {
        return Err(RvfBridgeError::WrongSegmentType {
            expected: SegmentType::PolicyKernel as u8,
            got: header.seg_type,
        });
    }
    validate_segment(&header, payload).map_err(RvfBridgeError::Rvf)?;
    let wire: WirePolicyKernel = serde_json::from_slice(payload).map_err(RvfBridgeError::Json)?;
    Ok(wire.into())
}

/// Serialize a `CostCurve` into an RVF COST_CURVE segment.
pub fn cost_curve_to_segment(curve: &CostCurve, segment_id: u64) -> Vec<u8> {
    let payload = serde_json::to_vec(curve).expect("CostCurve serialization cannot fail");
    write_segment(
        SegmentType::CostCurve as u8,
        &payload,
        SegmentFlags::empty(),
        segment_id,
    )
}

/// Deserialize a `CostCurve` from an RVF segment.
pub fn cost_curve_from_segment(data: &[u8]) -> Result<CostCurve, RvfBridgeError> {
    let (header, payload) = read_segment(data).map_err(RvfBridgeError::Rvf)?;
    if header.seg_type != SegmentType::CostCurve as u8 {
        return Err(RvfBridgeError::WrongSegmentType {
            expected: SegmentType::CostCurve as u8,
            got: header.seg_type,
        });
    }
    validate_segment(&header, payload).map_err(RvfBridgeError::Rvf)?;
    serde_json::from_slice(payload).map_err(RvfBridgeError::Json)
}

// ─── Witness chain ──────────────────────────────────────────────────────────

/// Witness type constants for domain expansion operations.
pub const WITNESS_TRANSFER: u8 = 0x10;
/// Witness type for policy kernel promotion.
pub const WITNESS_POLICY_PROMOTION: u8 = 0x11;
/// Witness type for cost curve convergence checkpoint.
pub const WITNESS_CONVERGENCE: u8 = 0x12;

/// Create a SHAKE-256 witness hash for a transfer prior.
///
/// The witness hash covers: source domain, training cycles, and the serialized
/// bucket priors. This replaces the old string-based `witness_hash` field.
pub fn compute_transfer_witness_hash(prior: &TransferPrior) -> [u8; 32] {
    let wire: WireTransferPrior = prior.into();
    let payload = serde_json::to_vec(&wire).expect("WireTransferPrior serialization cannot fail");
    rvf_crypto::shake256_256(&payload)
}

/// Build witness entries for a transfer verification event.
///
/// Returns entries suitable for `rvf_crypto::create_witness_chain()`.
pub fn build_transfer_witness_entries(
    prior: &TransferPrior,
    source: &DomainId,
    target: &DomainId,
    acceleration_factor: f32,
    timestamp_ns: u64,
) -> Vec<rvf_crypto::WitnessEntry> {
    let mut entries = Vec::with_capacity(2);

    // Entry 1: Transfer prior hash
    let prior_hash = compute_transfer_witness_hash(prior);
    entries.push(rvf_crypto::WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: prior_hash,
        timestamp_ns,
        witness_type: WITNESS_TRANSFER,
    });

    // Entry 2: Acceleration verification (hash of source→target + factor)
    let accel_payload = format!(
        "{}->{}:accel={:.6}",
        source.0, target.0, acceleration_factor
    );
    let accel_hash = rvf_crypto::shake256_256(accel_payload.as_bytes());
    entries.push(rvf_crypto::WitnessEntry {
        prev_hash: [0u8; 32], // chaining handled by create_witness_chain
        action_hash: accel_hash,
        timestamp_ns: timestamp_ns + 1,
        witness_type: WITNESS_CONVERGENCE,
    });

    entries
}

// ─── AGI Container TLV packaging ────────────────────────────────────────────

/// A TLV (Tag-Length-Value) entry for AGI container manifest packaging.
#[derive(Debug, Clone)]
pub struct AgiTlvEntry {
    /// TLV tag (see `AGI_TAG_*` constants in rvf-types).
    pub tag: u16,
    /// Serialized value payload.
    pub value: Vec<u8>,
}

/// Package domain expansion artifacts into AGI container TLV entries.
///
/// Returns a vector of TLV entries ready for inclusion in an AGI container
/// manifest segment. Each entry uses the corresponding `AGI_TAG_*` constant.
pub fn package_for_agi_container(
    priors: &[TransferPrior],
    kernels: &[PolicyKernel],
    scoreboard: &AccelerationScoreboard,
) -> Vec<AgiTlvEntry> {
    let mut entries = Vec::new();

    // Transfer priors (use wire format for JSON-safe serialization)
    for prior in priors {
        let wire: WireTransferPrior = prior.into();
        let value = serde_json::to_vec(&wire).expect("WireTransferPrior serialization cannot fail");
        entries.push(AgiTlvEntry {
            tag: rvf_types::AGI_TAG_TRANSFER_PRIOR,
            value,
        });
    }

    // Policy kernels (use wire format for JSON-safe serialization)
    for kernel in kernels {
        let wire: WirePolicyKernel = kernel.into();
        let value = serde_json::to_vec(&wire).expect("WirePolicyKernel serialization cannot fail");
        entries.push(AgiTlvEntry {
            tag: rvf_types::AGI_TAG_POLICY_KERNEL,
            value,
        });
    }

    // Cost curves from the scoreboard
    for curve in scoreboard.curves.values() {
        let value = serde_json::to_vec(curve).expect("CostCurve serialization cannot fail");
        entries.push(AgiTlvEntry {
            tag: rvf_types::AGI_TAG_COST_CURVE,
            value,
        });
    }

    entries
}

/// Encode TLV entries into a binary payload for inclusion in a META segment.
///
/// Wire format per entry: `[tag: u16 LE][length: u32 LE][value: length bytes]`
pub fn encode_tlv_entries(entries: &[AgiTlvEntry]) -> Vec<u8> {
    let total_size: usize = entries.iter().map(|e| 6 + e.value.len()).sum();
    let mut buf = Vec::with_capacity(total_size);
    for entry in entries {
        buf.extend_from_slice(&entry.tag.to_le_bytes());
        buf.extend_from_slice(&(entry.value.len() as u32).to_le_bytes());
        buf.extend_from_slice(&entry.value);
    }
    buf
}

/// Decode TLV entries from a binary payload.
pub fn decode_tlv_entries(data: &[u8]) -> Result<Vec<AgiTlvEntry>, RvfBridgeError> {
    let mut entries = Vec::new();
    let mut offset = 0;
    while offset + 6 <= data.len() {
        let tag = u16::from_le_bytes([data[offset], data[offset + 1]]);
        let length = u32::from_le_bytes([
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
        ]) as usize;
        offset += 6;
        if offset + length > data.len() {
            return Err(RvfBridgeError::TruncatedTlv);
        }
        entries.push(AgiTlvEntry {
            tag,
            value: data[offset..offset + length].to_vec(),
        });
        offset += length;
    }
    Ok(entries)
}

// ─── Solver bridge ──────────────────────────────────────────────────────────

/// Compact prior exchange format bridging domain expansion's `MetaThompsonEngine`
/// to the rvf-solver-wasm `PolicyKernel`.
///
/// The solver-wasm uses per-bucket `SkipModeStats` with `(alpha_safety, beta_safety)`
/// and `cost_ema`. The domain expansion uses per-bucket `BetaParams` with
/// `(alpha, beta)` and `cost_ema_priors`. This type converts between them.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SolverPriorExchange {
    /// Context bucket key (e.g. "medium:some:clean").
    pub bucket_key: String,
    /// Per-arm alpha/beta pairs mapping arm name to (alpha, beta).
    pub arm_params: Vec<(String, f32, f32)>,
    /// Cost EMA for this bucket.
    pub cost_ema: f32,
    /// Training cycle count for confidence estimation.
    pub training_cycles: u64,
}

/// Extract solver-compatible prior exchange data from the Thompson engine.
///
/// Flattens the domain expansion's hierarchical buckets into the solver's
/// flat "range:distractor:noise" keys for the specified domain.
pub fn extract_solver_priors(
    engine: &MetaThompsonEngine,
    domain_id: &DomainId,
) -> Vec<SolverPriorExchange> {
    let prior = match engine.extract_prior(domain_id) {
        Some(p) => p,
        None => return Vec::new(),
    };

    prior
        .bucket_priors
        .iter()
        .map(|(bucket, arms)| {
            let bucket_key = format!("{}:{}", bucket.difficulty_tier, bucket.category);
            let arm_params: Vec<(String, f32, f32)> = arms
                .iter()
                .map(|(arm, params)| (arm.0.clone(), params.alpha, params.beta))
                .collect();
            let cost_ema = prior.cost_ema_priors.get(bucket).copied().unwrap_or(1.0);

            SolverPriorExchange {
                bucket_key,
                arm_params,
                cost_ema,
                training_cycles: prior.training_cycles,
            }
        })
        .collect()
}

/// Import solver prior exchange data back into the Thompson engine.
///
/// Seeds the specified domain with the exchanged priors, enabling
/// cross-system transfer.
pub fn import_solver_priors(
    engine: &mut MetaThompsonEngine,
    domain_id: &DomainId,
    exchanges: &[SolverPriorExchange],
) {
    // Build a synthetic TransferPrior from the exchange data.
    let mut prior = TransferPrior::uniform(domain_id.clone());

    for exchange in exchanges {
        let parts: Vec<&str> = exchange.bucket_key.splitn(2, ':').collect();
        let bucket = ContextBucket {
            difficulty_tier: parts.first().unwrap_or(&"medium").to_string(),
            category: parts.get(1).unwrap_or(&"general").to_string(),
        };

        let mut arm_map = std::collections::HashMap::new();
        for (arm_name, alpha, beta) in &exchange.arm_params {
            arm_map.insert(
                crate::transfer::ArmId(arm_name.clone()),
                BetaParams {
                    alpha: *alpha,
                    beta: *beta,
                },
            );
        }
        prior.bucket_priors.insert(bucket.clone(), arm_map);
        prior.cost_ema_priors.insert(bucket, exchange.cost_ema);
        prior.training_cycles = exchange.training_cycles;
    }

    engine.init_domain_with_transfer(domain_id.clone(), &prior);
}

// ─── Multi-segment file assembly ────────────────────────────────────────────

/// Assemble a complete RVF byte stream containing all domain expansion segments.
///
/// Outputs concatenated segments: transfer priors, then policy kernels, then
/// cost curves. Each gets a unique segment ID starting from `base_segment_id`.
///
/// The returned bytes can be appended to an existing RVF file or written as
/// a standalone domain expansion archive.
pub fn assemble_domain_expansion_segments(
    priors: &[TransferPrior],
    kernels: &[PolicyKernel],
    curves: &[CostCurve],
    base_segment_id: u64,
) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut seg_id = base_segment_id;

    for prior in priors {
        buf.extend_from_slice(&transfer_prior_to_segment(prior, seg_id));
        seg_id += 1;
    }
    for kernel in kernels {
        buf.extend_from_slice(&policy_kernel_to_segment(kernel, seg_id));
        seg_id += 1;
    }
    for curve in curves {
        buf.extend_from_slice(&cost_curve_to_segment(curve, seg_id));
        seg_id += 1;
    }

    buf
}

// ─── Errors ─────────────────────────────────────────────────────────────────

/// Errors specific to the RVF bridge operations.
#[derive(Debug)]
pub enum RvfBridgeError {
    /// Underlying RVF format error.
    Rvf(rvf_types::RvfError),
    /// JSON serialization/deserialization error.
    Json(serde_json::Error),
    /// Segment type mismatch.
    WrongSegmentType {
        /// Expected segment type discriminant.
        expected: u8,
        /// Actual segment type discriminant.
        got: u8,
    },
    /// TLV payload truncated.
    TruncatedTlv,
}

impl std::fmt::Display for RvfBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rvf(e) => write!(f, "RVF error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::WrongSegmentType { expected, got } => {
                write!(
                    f,
                    "wrong segment type: expected 0x{expected:02X}, got 0x{got:02X}"
                )
            }
            Self::TruncatedTlv => write!(f, "TLV payload truncated"),
        }
    }
}

impl std::error::Error for RvfBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost_curve::{ConvergenceThresholds, CostCurvePoint};

    #[test]
    fn transfer_prior_round_trip() {
        let mut prior = TransferPrior::uniform(DomainId("test".into()));
        let bucket = ContextBucket {
            difficulty_tier: "medium".into(),
            category: "algo".into(),
        };
        prior.update_posterior(bucket, crate::transfer::ArmId("greedy".into()), 0.85);

        let segment = transfer_prior_to_segment(&prior, 1);
        let decoded = transfer_prior_from_segment(&segment).unwrap();

        assert_eq!(decoded.source_domain, prior.source_domain);
        assert_eq!(decoded.training_cycles, prior.training_cycles);
    }

    #[test]
    fn policy_kernel_round_trip() {
        let kernel = PolicyKernel::new("test_kernel".into());
        let segment = policy_kernel_to_segment(&kernel, 2);
        let decoded = policy_kernel_from_segment(&segment).unwrap();

        assert_eq!(decoded.id, "test_kernel");
        assert_eq!(decoded.generation, 0);
    }

    #[test]
    fn cost_curve_round_trip() {
        let mut curve = CostCurve::new(DomainId("test".into()), ConvergenceThresholds::default());
        curve.record(CostCurvePoint {
            cycle: 0,
            accuracy: 0.3,
            cost_per_solve: 0.1,
            robustness: 0.3,
            policy_violations: 0,
            timestamp: 0.0,
        });

        let segment = cost_curve_to_segment(&curve, 3);
        let decoded = cost_curve_from_segment(&segment).unwrap();

        assert_eq!(decoded.domain_id, DomainId("test".into()));
        assert_eq!(decoded.points.len(), 1);
    }

    #[test]
    fn wrong_segment_type_detected() {
        let kernel = PolicyKernel::new("k".into());
        let segment = policy_kernel_to_segment(&kernel, 1);
        let result = transfer_prior_from_segment(&segment);
        assert!(matches!(
            result,
            Err(RvfBridgeError::WrongSegmentType { .. })
        ));
    }

    #[test]
    fn witness_hash_is_deterministic() {
        let prior = TransferPrior::uniform(DomainId("test".into()));
        let h1 = compute_transfer_witness_hash(&prior);
        let h2 = compute_transfer_witness_hash(&prior);
        assert_eq!(h1, h2);
        assert_ne!(h1, [0u8; 32]);
    }

    #[test]
    fn witness_entries_chain() {
        let prior = TransferPrior::uniform(DomainId("d1".into()));
        let entries = build_transfer_witness_entries(
            &prior,
            &DomainId("d1".into()),
            &DomainId("d2".into()),
            2.5,
            1_000_000_000,
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].witness_type, WITNESS_TRANSFER);
        assert_eq!(entries[1].witness_type, WITNESS_CONVERGENCE);

        // Verify the chain is valid after linking
        let chain_bytes = rvf_crypto::create_witness_chain(&entries);
        let verified = rvf_crypto::verify_witness_chain(&chain_bytes).unwrap();
        assert_eq!(verified.len(), 2);
    }

    #[test]
    fn tlv_round_trip() {
        let entries = vec![
            AgiTlvEntry {
                tag: rvf_types::AGI_TAG_TRANSFER_PRIOR,
                value: b"hello".to_vec(),
            },
            AgiTlvEntry {
                tag: rvf_types::AGI_TAG_POLICY_KERNEL,
                value: b"world".to_vec(),
            },
        ];

        let encoded = encode_tlv_entries(&entries);
        let decoded = decode_tlv_entries(&encoded).unwrap();

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].tag, rvf_types::AGI_TAG_TRANSFER_PRIOR);
        assert_eq!(decoded[0].value, b"hello");
        assert_eq!(decoded[1].tag, rvf_types::AGI_TAG_POLICY_KERNEL);
        assert_eq!(decoded[1].value, b"world");
    }

    #[test]
    fn agi_container_packaging() {
        let prior = TransferPrior::uniform(DomainId("test".into()));
        let kernel = PolicyKernel::new("k0".into());
        let scoreboard = crate::cost_curve::AccelerationScoreboard::new();

        let entries = package_for_agi_container(&[prior], &[kernel], &scoreboard);
        assert_eq!(entries.len(), 2); // 1 prior + 1 kernel, 0 curves

        let encoded = encode_tlv_entries(&entries);
        let decoded = decode_tlv_entries(&encoded).unwrap();
        assert_eq!(decoded.len(), 2);
    }

    #[test]
    fn solver_prior_exchange_round_trip() {
        let arms = vec!["greedy".into(), "exploratory".into()];
        let mut engine = MetaThompsonEngine::new(arms);
        let domain = DomainId("test".into());
        engine.init_domain_uniform(domain.clone());

        let bucket = ContextBucket {
            difficulty_tier: "medium".into(),
            category: "algorithm".into(),
        };
        for _ in 0..20 {
            engine.record_outcome(
                &domain,
                bucket.clone(),
                crate::transfer::ArmId("greedy".into()),
                0.9,
                1.0,
            );
        }

        let exchanges = extract_solver_priors(&engine, &domain);
        assert!(!exchanges.is_empty());

        // Import into a fresh engine
        let new_arms = vec!["greedy".into(), "exploratory".into()];
        let mut new_engine = MetaThompsonEngine::new(new_arms);
        let target = DomainId("target".into());
        new_engine.init_domain_uniform(target.clone());
        import_solver_priors(&mut new_engine, &target, &exchanges);

        // Should have transferred priors
        let extracted = new_engine.extract_prior(&target);
        assert!(extracted.is_some());
    }

    #[test]
    fn multi_segment_assembly() {
        let prior = TransferPrior::uniform(DomainId("d1".into()));
        let kernel = PolicyKernel::new("k0".into());
        let mut curve = CostCurve::new(DomainId("d1".into()), ConvergenceThresholds::default());
        curve.record(CostCurvePoint {
            cycle: 0,
            accuracy: 0.5,
            cost_per_solve: 0.05,
            robustness: 0.5,
            policy_violations: 0,
            timestamp: 0.0,
        });

        let assembled = assemble_domain_expansion_segments(&[prior], &[kernel], &[curve], 100);

        // Should contain 3 segments, each 64-byte aligned
        assert!(assembled.len() >= 3 * 64);
        assert_eq!(assembled.len() % 64, 0);

        // Verify first segment header magic
        let magic = u32::from_le_bytes([assembled[0], assembled[1], assembled[2], assembled[3]]);
        assert_eq!(magic, rvf_types::SEGMENT_MAGIC);
    }
}
