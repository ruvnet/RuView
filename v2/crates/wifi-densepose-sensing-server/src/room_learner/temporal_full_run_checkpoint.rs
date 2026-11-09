//! Structural v3 codec. Raw JSON never installs owner/fitted/write authority.
use super::*;
use full_run::{ContinuationPolicy, FullRunSnapshotPermit, FullRunState, VerifiedFitRecordData};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Common {
    binding: ManagedBinding,
    run: String,
    descriptor: RecoveryDescriptor,
    last_now_ns: u64,
    source_high: Vec<(u8, u64, u64)>,
    ledger: Ledger,
    mapping: Option<ObservedSegmentMap>,
    episode: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Accepted<R = CaptureCohort, E = WindowEvidence> {
    rows: Vec<R>,
    evidence: Vec<E>,
    identities: Vec<ManagedWindowIdentity>,
    fences: AssemblerCheckpoint,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FullStage {
    Train {
        bridge: BridgeState,
    },
    Frozen {
        fit: VerifiedFitRecordData,
        train_seals: Vec<bridge::TrainSeal>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FullData<R = CaptureCohort, E = WindowEvidence> {
    schema: String,
    policy: ContinuationPolicy,
    common: Common,
    accepted: Accepted<R, E>,
    stage: FullStage,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FullEnvelope<P = FullData> {
    schema: String,
    payload_sha256: String,
    payload: P,
}
pub(super) struct FullSnapshot {
    data: FullData<SharedRecord<CaptureCohort>, SharedRecord<WindowEvidence>>,
    pub permit: Option<FullRunSnapshotPermit>,
}
impl FullSnapshot {
    pub(super) fn from_workflow(w: &TemporalWorkflow) -> Result<Self, String> {
        let mode = w.full_run.as_ref().ok_or("temporal_full_run_required")?;
        mode.policy.validate(
            &w.session
                .managed
                .as_ref()
                .ok_or("managed_session_required")?
                .binding,
        )?;
        let permit = mode.snapshot_permit(w.commit_token.clone())?;
        let old = super::snapshot_old(w)?;
        let stage = match &mode.frozen {
            None => FullStage::Train {
                bridge: old
                    .bridge
                    .clone()
                    .ok_or("temporal_full_run_bridge_required")?,
            },
            Some(fit) => {
                if mode.pending_record() != Some(fit) {
                    return Err("temporal_frozen_current_producer_required".into());
                }
                FullStage::Frozen {
                    fit: fit.clone(),
                    train_seals: old
                        .bridge
                        .as_ref()
                        .ok_or("temporal_full_run_bridge_required")?
                        .seals
                        .clone(),
                }
            }
        };
        Ok(Self {
            data: FullData {
                schema: full_run::PREFIX_SCHEMA.into(),
                policy: mode.policy.clone(),
                common: Common {
                    binding: old.binding,
                    run: old.run,
                    descriptor: old.descriptor,
                    last_now_ns: old.last_now_ns,
                    source_high: old.source_high,
                    ledger: old.ledger,
                    mapping: old.mapping,
                    episode: old.episode,
                },
                accepted: Accepted {
                    rows: old.rows,
                    evidence: old.evidence,
                    identities: old.identities,
                    fences: old.fences,
                },
                stage,
            },
            permit: Some(permit),
        })
    }
    pub(super) fn records(&self) -> SharedRecords {
        SharedRecords {
            rows: self.data.accepted.rows.clone(),
            evidence: self.data.accepted.evidence.clone(),
        }
    }
    pub(super) fn encode(self) -> Result<Vec<u8>, String> {
        let envelope = FullEnvelope {
            schema: full_run::PREFIX_SCHEMA.into(),
            payload_sha256: hash(&self.data)?,
            payload: self.data,
        };
        let bytes = serde_json::to_vec(&envelope).map_err(|_| "temporal_full_run_encoding")?;
        if bytes.len() > crate::room_learner::recovery_store::MAX_RECOVERY_BYTES {
            return Err("temporal_checkpoint_budget".into());
        }
        Ok(bytes)
    }
}
pub(super) fn is_full(bytes: &[u8]) -> Result<bool, String> {
    if bytes.len() > crate::room_learner::recovery_store::MAX_RECOVERY_BYTES {
        return Err("temporal_checkpoint_budget".into());
    }
    #[derive(Deserialize)]
    struct Header {
        schema: String,
    }
    // Header dispatch skips unknown fields without making a serde Value/raw
    // copy. The selected strict decoder still checks every field/canonical byte.
    let header: Header = serde_json::from_slice(bytes).map_err(|_| "temporal_checkpoint_schema")?;
    Ok(header.schema == full_run::PREFIX_SCHEMA)
}
fn parse(bytes: &[u8]) -> Result<FullEnvelope, String> {
    if bytes.len() > crate::room_learner::recovery_store::MAX_RECOVERY_BYTES {
        return Err("temporal_checkpoint_budget".into());
    }
    let e: FullEnvelope = serde_json::from_slice(bytes).map_err(|_| "temporal_full_run_schema")?;
    if e.schema != full_run::PREFIX_SCHEMA
        || e.payload.schema != full_run::PREFIX_SCHEMA
        || e.payload_sha256 != hash(&e.payload)?
        || serde_json::to_vec(&e).map_err(|_| "temporal_full_run_encoding")? != bytes
    {
        return Err("temporal_full_run_integrity".into());
    }
    e.payload.policy.validate(&e.payload.common.binding)?;
    Ok(e)
}
fn old_projection(
    data: FullData,
) -> Result<(Prefix, ContinuationPolicy, Option<VerifiedFitRecordData>), String> {
    let common = data.common;
    let accepted = data.accepted;
    let (bridge, frozen) = match data.stage {
        FullStage::Train { bridge } => (bridge, None),
        FullStage::Frozen { fit, train_seals } => {
            fit.validate_data(&common.binding, &data.policy)?;
            if common.run != fit.body.run
                || common.last_now_ns != fit.body.freeze_prepare_receiver_ns
                || common.source_high != fit.body.source_floors
                || common.mapping.is_some()
            {
                return Err("temporal_frozen_record_cut_or_floors".into());
            }
            let b = BridgeState {
                policy: bridge::BRIDGE_POLICY.into(),
                disposition: bridge::Disposition::Complete,
                seals: train_seals,
                parts: vec![],
            };
            (b, Some(fit))
        }
    };
    Ok((
        Prefix {
            schema: BRIDGE_SCHEMA.into(),
            binding: common.binding,
            run: common.run,
            descriptor: common.descriptor,
            last_now_ns: common.last_now_ns,
            rows: accepted.rows,
            evidence: accepted.evidence,
            identities: accepted.identities,
            fences: accepted.fences,
            source_high: common.source_high,
            ledger: common.ledger,
            mapping: common.mapping,
            episode: common.episode,
            frozen_model: None,
            quality_issued: false,
            confirmation_issued: false,
            bridge: Some(bridge),
        },
        data.policy,
        frozen,
    ))
}
fn old_bytes(prefix: Prefix) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&Envelope {
        schema: BRIDGE_SCHEMA.into(),
        payload_sha256: hash(&prefix)?,
        payload: prefix,
    })
    .map_err(|_| "temporal_full_run_projection".into())
}
pub(super) fn decode_structural(
    bytes: &[u8],
    descriptor: &RecoveryDescriptor,
    now: u64,
) -> Result<TemporalWorkflow, String> {
    let e = parse(bytes)?;
    let (prefix, policy, frozen) = old_projection(e.payload)?;
    // This slice reuses the exact existing Train/quality/native validator. No
    // HeldOut/Complete variant is accepted before the actual C4B producer exists.
    let mut w = super::decode_old(&old_bytes(prefix)?, descriptor, now)?;
    if let Some(record) = &frozen {
        let managed = w
            .session
            .managed
            .as_ref()
            .ok_or("managed_session_required")?;
        let bridge = w
            .bridge
            .as_ref()
            .ok_or("temporal_full_run_bridge_required")?;
        let body = &record.body;
        if w.session.rows.len() != 64
            || w.session.rows.iter().any(|r| r.role != DataRole::Train)
            || bridge.disposition != bridge::Disposition::Complete
            || bridge.seals.len() != 7
            || body.training_data_sha256 != hash(&w.session.rows)?
            || body.input_evidence_sha256 != hash(&w.session.evidence)?
            || body.identity_evidence_sha256 != hash(&managed.windows)?
            || body.train_seal_chain_sha256 != hash(&bridge.seals)?
            || body.accepted_train_commitment.run != w.session.run_id
            || body.accepted_train_commitment.id != descriptor.id
        {
            return Err("temporal_frozen_train_binding".into());
        }
    }
    w.full_run = Some(FullRunState::structural(policy, frozen));
    // Deliberately remains unfitted. Only authenticated CURRENT recovery plus
    // exact whole-model verification can mint the install witness.
    Ok(w)
}
pub(super) fn validated_structural(
    bytes: &[u8],
    descriptor: &RecoveryDescriptor,
    now: u64,
) -> Result<ValidatedTemporalPrefix, String> {
    let w = decode_structural(bytes, descriptor, now)?;
    let mode = w.full_run.as_ref().ok_or("temporal_full_run_required")?;
    Ok(ValidatedTemporalPrefix {
        bytes: bytes.to_vec(),
        descriptor: descriptor.clone(),
        run: w.session.run_id.clone(),
        policy_sha256: mode.policy.sha256().into(),
        window_count: w.session.rows.len(),
        transition_limit: mode.policy.transition_limit(),
        saved_receiver_ns: w.last_now_ns,
        metadata_kind: full_run::META_KIND,
        full_write_grant: None,
        full_frozen_predecessor: mode
            .frozen
            .as_ref()
            .map(|r| r.body.accepted_train_commitment.clone()),
    })
}
pub(super) fn is_new_full_run(p: &ValidatedTemporalPrefix) -> Result<bool, String> {
    let e = parse(p.bytes())?;
    if !matches!(e.payload.stage, FullStage::Train { .. }) {
        return Ok(false);
    }
    let (prefix, _, _) = old_projection(e.payload)?;
    let bytes = old_bytes(prefix)?;
    let old = super::validated_prefix(&bytes, p.descriptor(), p.saved_receiver_ns())?;
    super::is_new_empty_run(&old)
}
pub(super) fn validate_full_extension(
    old: &ValidatedTemporalPrefix,
    new: &ValidatedTemporalPrefix,
) -> Result<(), String> {
    if old.run != new.run
        || old.policy_sha256 != new.policy_sha256
        || old.transition_limit != new.transition_limit
        || old.descriptor.id != new.descriptor.id
        || old.descriptor.owner_scope != new.descriptor.owner_scope
        || old.descriptor.installation_id != new.descriptor.installation_id
        || old.descriptor.context_digest != new.descriptor.context_digest
        || old.descriptor.created_at_ms != new.descriptor.created_at_ms
        || old.descriptor.expires_at_ms != new.descriptor.expires_at_ms
        || new.descriptor.saved_at_ms < old.descriptor.saved_at_ms
        || new.saved_receiver_ns < old.saved_receiver_ns
    {
        return Err("temporal_commit_identity_or_time_changed".into());
    }
    let a = parse(old.bytes())?;
    let b = parse(new.bytes())?;
    if serde_json::to_vec(&a.payload.policy).map_err(|_| "temporal_encoding")?
        != serde_json::to_vec(&b.payload.policy).map_err(|_| "temporal_encoding")?
    {
        return Err("temporal_full_run_policy_changed".into());
    }
    if old.bytes() == new.bytes() {
        return Ok(());
    }
    match (&a.payload.stage, &b.payload.stage) {
        (FullStage::Train { .. }, FullStage::Train { .. }) => {
            let (ap, _, _) = old_projection(a.payload)?;
            let (bp, _, _) = old_projection(b.payload)?;
            let ab = old_bytes(ap)?;
            let bb = old_bytes(bp)?;
            let av = super::validated_prefix(&ab, old.descriptor(), old.saved_receiver_ns())?;
            let bv = super::validated_prefix(&bb, new.descriptor(), new.saved_receiver_ns())?;
            super::validate_extension(&av, &bv)
        }
        (FullStage::Train { bridge }, FullStage::Frozen { fit, train_seals }) => {
            if bridge.disposition != bridge::Disposition::Complete
                || bridge.seals != *train_seals
                || fit.body.accepted_train_prefix_sha256 != bytes_hash(old.bytes())
                || serde_json::to_vec(&a.payload.accepted).map_err(|_| "temporal_encoding")?
                    != serde_json::to_vec(&b.payload.accepted).map_err(|_| "temporal_encoding")?
                || a.payload.common.binding.schedule_sha256
                    != b.payload.common.binding.schedule_sha256
                || a.payload.common.run != b.payload.common.run
                || a.payload.common.ledger != b.payload.common.ledger
                || a.payload.common.mapping.is_some()
                || b.payload.common.mapping.is_some()
                || a.payload.common.episode != b.payload.common.episode
                || b.payload.common.last_now_ns < a.payload.common.last_now_ns
            {
                return Err("temporal_frozen_transition_binding".into());
            }
            for (id, seq, time) in &a.payload.common.source_high {
                if !b
                    .payload
                    .common
                    .source_high
                    .iter()
                    .any(|(s, q, t)| s == id && q >= seq && t >= time)
                {
                    return Err("temporal_commit_floor_regressed".into());
                }
            }
            // General descriptor/policy identity is also enforced by store
            // commitment preparation; the raw structure grants no write.
            Ok(())
        }
        _ => Err("temporal_full_run_transition_unsupported".into()),
    }
}
