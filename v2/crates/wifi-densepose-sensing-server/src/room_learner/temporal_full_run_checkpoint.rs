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
    HeldOut {
        fit: VerifiedFitRecordData,
        train_seals: Vec<bridge::TrainSeal>,
        held_out: held_out::HeldOutBridgeState,
    },
    Complete {
        fit: VerifiedFitRecordData,
        train_seals: Vec<bridge::TrainSeal>,
        held_out_seals: Vec<held_out::HeldOutSealData>,
        complete: held_out::FullCompleteRecordData,
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
        let old = if mode.held_out.is_some() {
            mode.check_current_train_prefix(&w.session)?;
            if w.session.failed || w.session.active || w.session.rows.len() != w.ledger.spans.len()
            {
                return Err("temporal_full_snapshot_incomplete".into());
            }
            let managed = w
                .session
                .managed
                .as_ref()
                .ok_or("managed_session_required")?;
            Prefix {
                schema: BRIDGE_SCHEMA.into(),
                binding: managed.binding.clone(),
                run: w.session.run_id.clone(),
                descriptor: w.descriptor.clone(),
                last_now_ns: w.last_now_ns,
                rows: w.session.rows.clone(),
                evidence: w.session.evidence.clone(),
                identities: managed.windows.clone(),
                fences: w.session.assembler.fence_checkpoint()?,
                source_high: w.source_high.clone(),
                ledger: w.ledger.clone(),
                mapping: w.mapping.clone(),
                episode: w.episode,
                frozen_model: None,
                quality_issued: false,
                confirmation_issued: false,
                bridge: w.bridge.clone(),
            }
        } else {
            super::snapshot_old(w)?
        };
        let stage = if let Some(held) = &mode.held_out {
            let fit = mode
                .frozen
                .as_ref()
                .ok_or("temporal_heldout_frozen_record_missing")?
                .clone();
            let train_seals = old
                .bridge
                .as_ref()
                .ok_or("temporal_full_run_bridge_required")?
                .seals
                .clone();
            if let Some(complete) = &mode.complete {
                FullStage::Complete {
                    fit,
                    train_seals,
                    held_out_seals: held.seals.clone(),
                    complete: complete.clone(),
                }
            } else {
                FullStage::HeldOut {
                    fit,
                    train_seals,
                    held_out: held.clone(),
                }
            }
        } else {
            match &mode.frozen {
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
        FullStage::Train { bridge } => {
            // Full-v3 genesis and model identity retain their ORIGINAL Train
            // seal profile. Recognizing fresh343 for Kind1 must not widen this
            // separately versioned Kind2 continuation contract.
            if bridge.policy != bridge::BRIDGE_POLICY {
                return Err("temporal_full_run_train_profile_changed".into());
            }
            (bridge, None)
        }
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
        _ => return Err("temporal_post_frozen_requires_full_decoder".into()),
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
    if matches!(
        e.payload.stage,
        FullStage::HeldOut { .. } | FullStage::Complete { .. }
    ) {
        return decode_continued(e.payload, descriptor, now);
    }
    let (prefix, policy, frozen) = old_projection(e.payload)?;
    // This slice reuses the exact existing Train/quality/native validator. No
    // HeldOut/Complete variant is accepted before the actual C4B producer exists.
    let replay_budget = policy.replay_fence_budget(&prefix.binding)?;
    let mut w =
        super::decode_full_train_projection(&old_bytes(prefix)?, descriptor, now, &replay_budget)?;
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
        full_fit_record_sha256: mode.frozen.as_ref().map(|fit| fit.sha256.clone()),
        full_stage: Some(if let Some(complete) = &mode.complete {
            ValidatedFullStage::Complete(complete.body.accepted_held_out_commitment.clone())
        } else if mode.held_out.is_some() {
            ValidatedFullStage::HeldOut
        } else if let Some(fit) = &mode.frozen {
            ValidatedFullStage::Frozen(fit.body.accepted_train_commitment.clone())
        } else {
            ValidatedFullStage::Train
        }),
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
    if matches!(
        a.payload.stage,
        FullStage::HeldOut { .. } | FullStage::Complete { .. }
    ) || matches!(
        b.payload.stage,
        FullStage::HeldOut { .. } | FullStage::Complete { .. }
    ) {
        return validate_continued_extension(&a.payload, &b.payload);
    }
    match (&a.payload.stage, &b.payload.stage) {
        (FullStage::Train { .. }, FullStage::Train { .. }) => {
            let (ap, apolicy, _) = old_projection(a.payload)?;
            let (bp, bpolicy, _) = old_projection(b.payload)?;
            let a_budget = apolicy.replay_fence_budget(&ap.binding)?;
            let b_budget = bpolicy.replay_fence_budget(&bp.binding)?;
            let ab = old_bytes(ap)?;
            let bb = old_bytes(bp)?;
            let av = super::validated_full_train_projection(
                &ab,
                old.descriptor(),
                old.saved_receiver_ns(),
                &a_budget,
            )?;
            let bv = super::validated_full_train_projection(
                &bb,
                new.descriptor(),
                new.saved_receiver_ns(),
                &b_budget,
            )?;
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

fn decode_continued(
    data: FullData,
    descriptor: &RecoveryDescriptor,
    now: u64,
) -> Result<TemporalWorkflow, String> {
    let (fit, train_seals, held, complete) = match data.stage {
        FullStage::HeldOut {
            fit,
            train_seals,
            held_out,
        } => (fit, train_seals, held_out, None),
        FullStage::Complete {
            fit,
            train_seals,
            held_out_seals,
            complete,
        } => {
            let mut held = held_out::HeldOutBridgeState::new();
            held.disposition = bridge::Disposition::Complete;
            held.seals = held_out_seals;
            (fit, train_seals, held, Some(complete))
        }
        _ => return Err("temporal_post_frozen_stage_required".into()),
    };
    let common = data.common;
    let accepted = data.accepted;
    fit.validate_data(&common.binding, &data.policy)?;
    let schedule = common.binding.contract()?;
    if accepted.rows.len() < 64
        || accepted.rows.len() > 128
        || accepted.evidence.len() != accepted.rows.len()
        || accepted.identities.len() != accepted.rows.len()
        || common.ledger.spans.len() != accepted.rows.len()
        || common.run != fit.body.run
        || common.last_now_ns < fit.body.freeze_prepare_receiver_ns
        || fit.body.source_floors.iter().any(|(id, seq, time)| {
            !common
                .source_high
                .iter()
                .any(|(s, q, t)| s == id && q >= seq && t >= time)
        })
    {
        return Err("temporal_heldout_prefix_or_freeze_floor".into());
    }
    let train = BridgeState {
        policy: bridge::BRIDGE_POLICY.into(),
        disposition: bridge::Disposition::Complete,
        seals: train_seals,
        parts: vec![],
    };
    let train_ledger = Ledger {
        spans: common.ledger.spans[..64].to_vec(),
        gaps: common
            .ledger
            .gaps
            .iter()
            .filter(|g| g.episode < 7)
            .cloned()
            .collect(),
    };
    if train.seals.len() != 7 || held.seals.len() > 8 {
        return Err("temporal_bridge_phase_order".into());
    }
    let mut count = 0;
    let mut cuts = train
        .seals
        .iter()
        .enumerate()
        .map(|(i, _)| {
            count += schedule.episodes()[i].slots.len();
            count
        })
        .collect::<Vec<_>>();
    cuts.extend((0..held.seals.len()).map(|i| 64 + (i + 1) * 8));
    let digests = prefix_digests::canonical_tuple_prefixes(
        &accepted.rows,
        &accepted.evidence,
        &common.ledger.spans,
        &cuts,
    )?;
    train.validate_full_run_with_prefixes(
        &schedule,
        &common.run,
        &accepted.rows[..64],
        &accepted.evidence[..64],
        &train_ledger,
        7,
        None,
        common.last_now_ns,
        &digests,
    )?;
    let train_parts = train.seals.iter().map(|s| s.parts.len()).sum();
    held.validate_shape_data(
        &schedule,
        &common.run,
        &fit,
        common.episode,
        accepted.rows.len(),
        train_parts,
        common.ledger.gaps.len(),
        common.last_now_ns,
    )?;
    let frozen=crate::room_learner::managed_schedule_contract::FrozenModelIdentity::from_runtime_verified_artifact(&schedule,fit.body.artifact_sha256.clone())?;
    for (index, seal) in held.seals.iter().enumerate() {
        let count = 64 + (index + 1) * 8;
        seal.validate_actual_prefix_digest(digests.at(count)?)?;
        bridge::validate_parts_with_frozen(
            &seal.body.parts,
            &schedule,
            &common.run,
            7 + index,
            Some(30_000_000_000),
            seal.body.confirmed_at_ns,
            Some(&frozen),
        )?;
        bridge::validate_part_lineage(
            &seal.body.parts,
            7 + index,
            &common.ledger,
            &accepted.rows,
            None,
        )?;
    }
    if held.seals.len() < 8 {
        let accepted_ns = common
            .ledger
            .spans
            .last()
            .filter(|s| s.episode == common.episode)
            .map_or(0, |s| s.observed_end_ns);
        bridge::validate_parts_with_frozen(
            &held.parts,
            &schedule,
            &common.run,
            common.episode,
            None,
            common.last_now_ns,
            Some(&frozen),
        )?;
        bridge::validate_part_lineage(
            &held.parts,
            common.episode,
            &common.ledger,
            &accepted.rows,
            common.mapping.as_ref(),
        )?;
        if let Some(mapping) = &common.mapping {
            if held.parts.last().map(|p| &p.mapping) != Some(mapping)
                || held.parts.last().map_or(0, |p| p.observed_end_ns) != accepted_ns
            {
                return Err("temporal_heldout_current_mapping_or_frontier".into());
            }
        } else if !held.parts.is_empty() || accepted.rows.len() != 64 + held.seals.len() * 8 {
            return Err("temporal_heldout_mapping_missing".into());
        }
        if matches!(
            held.disposition,
            bridge::Disposition::Observing
                | bridge::Disposition::Paused
                | bridge::Disposition::NeedsConfirmation
        ) && common.mapping.is_none()
        {
            return Err("temporal_heldout_phase_without_mapping".into());
        }
        if held.disposition == bridge::Disposition::NeedsConfirmation
            && accepted_ns != 30_000_000_000
        {
            return Err("temporal_heldout_confirmation_before_end".into());
        }
    } else if common.mapping.is_some() {
        return Err("temporal_heldout_done_mapping".into());
    }
    for part in held
        .seals
        .iter()
        .flat_map(|s| &s.body.parts)
        .chain(&held.parts)
    {
        for observed in &part.sources {
            if !common.source_high.iter().any(|(id, seq, time)| {
                *id == observed.source_id
                    && *seq >= observed.last_sequence
                    && *time >= observed.last_ns
            }) {
                return Err("temporal_heldout_quality_floors_regressed".into());
            }
        }
    }
    if let Some(record) = &complete {
        record.validate_binding_data(&fit, &common.descriptor, now)?;
        if held.seals.len() != 8
            || accepted.rows.len() != 128
            || common.mapping.is_some()
            || record.body.held_out_chain_sha256 != held.seals.last().unwrap().sha256
            || record.body.complete_prepare_receiver_ns != common.last_now_ns
            || record.body.source_floors != common.source_high
            || record.body.raw_sha256 != hash(&accepted.rows)?
            || record.body.evidence_sha256 != hash(&accepted.evidence)?
            || record.body.identity_sha256 != hash(&accepted.identities)?
            || record.body.ledger_sha256 != hash(&common.ledger)?
        {
            return Err("temporal_full_complete_prefix_changed".into());
        }
    }
    let replay_budget = data.policy.replay_fence_budget(&common.binding)?;
    let prefix = Prefix {
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
        bridge: Some(train),
    };
    let mut workflow = super::decode_full_post_frozen(
        prefix,
        descriptor,
        now,
        &fit,
        held.seals.len(),
        &replay_budget,
    )?;
    validate_immutable_train(&workflow, &fit)?;
    let mut mode = FullRunState::structural(data.policy, Some(fit));
    mode.held_out = Some(held);
    mode.complete = complete;
    workflow.full_run = Some(mode);
    Ok(workflow)
}
pub(super) fn validate_immutable_train(
    w: &TemporalWorkflow,
    record: &VerifiedFitRecordData,
) -> Result<(), String> {
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
    if w.session.rows.len() < 64
        || w.session.evidence.len() < 64
        || managed.windows.len() < 64
        || w.session.rows[..64]
            .iter()
            .any(|r| r.role != DataRole::Train)
        || bridge.disposition != bridge::Disposition::Complete
        || bridge.seals.len() != 7
        || body.training_data_sha256 != hash(&(&w.session.rows[..64]))?
        || body.input_evidence_sha256 != hash(&(&w.session.evidence[..64]))?
        || body.identity_evidence_sha256 != hash(&(&managed.windows[..64]))?
        || body.train_seal_chain_sha256 != hash(&bridge.seals)?
        || body.accepted_train_commitment.run != w.session.run_id
        || body.accepted_train_commitment.id != w.descriptor.id
    {
        return Err("temporal_frozen_train_binding".into());
    }
    Ok(())
}

fn validate_continued_extension(a: &FullData, b: &FullData) -> Result<(), String> {
    macro_rules! prefix {
        ($old:expr,$new:expr) => {
            if $new.len() < $old.len()
                || serde_json::to_vec(&$new[..$old.len()]).map_err(|_| "temporal_encoding")?
                    != serde_json::to_vec(&$old).map_err(|_| "temporal_encoding")?
            {
                return Err("temporal_committed_prefix_changed".into());
            }
        };
    }
    prefix!(a.accepted.rows, b.accepted.rows);
    prefix!(a.accepted.evidence, b.accepted.evidence);
    prefix!(a.accepted.identities, b.accepted.identities);
    prefix!(a.common.ledger.spans, b.common.ledger.spans);
    prefix!(a.common.ledger.gaps, b.common.ledger.gaps);
    for (id, seq, time) in &a.common.source_high {
        if !b
            .common
            .source_high
            .iter()
            .any(|(s, q, t)| s == id && q >= seq && t >= time)
        {
            return Err("temporal_commit_floor_regressed".into());
        }
    }
    let fixed = |fit_a: &VerifiedFitRecordData,
                 train_a: &Vec<bridge::TrainSeal>,
                 fit_b: &VerifiedFitRecordData,
                 train_b: &Vec<bridge::TrainSeal>|
     -> Result<(), String> {
        if fit_a != fit_b || train_a != train_b {
            return Err("temporal_heldout_frozen_identity_changed".into());
        }
        Ok(())
    };
    match (&a.stage, &b.stage) {
        (
            FullStage::Frozen { fit, train_seals },
            FullStage::HeldOut {
                fit: next_fit,
                train_seals: next_train,
                held_out,
            },
        ) => {
            fixed(fit, train_seals, next_fit, next_train)?;
            if a.accepted.rows.len() != 64
                || b.accepted.rows.len() != 64
                || a.common.episode != 7
                || b.common.episode != 7
                || a.common.mapping.is_some()
                || b.common.mapping.as_ref().is_none_or(|m| {
                    m.segment_ordinal != 0
                        || m.observed_start_ns != 0
                        || m.receiver_start_ns <= fit.body.freeze_prepare_receiver_ns
                })
                || a.common.ledger != b.common.ledger
                || !held_out.seals.is_empty()
                || held_out.parts.len() != 1
                || held_out.parts[0].mapping != *b.common.mapping.as_ref().unwrap()
                || !held_out.parts[0].sources.is_empty()
                || held_out.parts[0].observed_end_ns != 0
                || held_out.disposition != bridge::Disposition::Observing
            {
                return Err("temporal_first_heldout_segment_changed".into());
            }
            Ok(())
        }
        (
            FullStage::HeldOut {
                fit,
                train_seals,
                held_out: old,
            },
            FullStage::HeldOut {
                fit: next_fit,
                train_seals: next_train,
                held_out: new,
            },
        ) => {
            fixed(fit, train_seals, next_fit, next_train)?;
            prefix!(old.seals, new.seals);
            let window = b.accepted.rows.len() == a.accepted.rows.len() + 1
                && b.common.ledger.gaps == a.common.ledger.gaps
                && b.common.mapping == a.common.mapping;
            let segment = b.accepted.rows.len() == a.accepted.rows.len()
                && ((a.common.mapping.is_none()
                    && b.common.mapping.is_some()
                    && b.common.ledger.gaps == a.common.ledger.gaps)
                    || b.common.ledger.gaps.len() == a.common.ledger.gaps.len() + 1);
            let seal = b.accepted.rows.len() == a.accepted.rows.len()
                && b.common.episode == a.common.episode + 1
                && b.common.ledger.gaps == a.common.ledger.gaps
                && b.common.mapping.is_none()
                && new.seals.len() == old.seals.len() + 1
                && new.parts.is_empty()
                && new.seals.last().is_some_and(|s| {
                    s.body.parts == old.parts && s.body.confirmed_at_ns >= a.common.last_now_ns
                });
            let pause = b.accepted.rows.len() == a.accepted.rows.len()
                && b.common.episode == a.common.episode
                && b.common.mapping == a.common.mapping
                && b.common.ledger.gaps == a.common.ledger.gaps
                && new.seals == old.seals
                && new.parts == old.parts
                && new.disposition == bridge::Disposition::Paused;
            if seal {
                return Ok(());
            }
            if new.seals != old.seals || b.common.episode != a.common.episode {
                return Err("temporal_heldout_seal_transition".into());
            }
            if window {
                if new.parts.len() != old.parts.len() || old.parts.is_empty() {
                    return Err("temporal_quality_part_transition".into());
                }
                let n = old.parts.len() - 1;
                if old.parts[..n] != new.parts[..n]
                    || old.parts[n].mapping != new.parts[n].mapping
                    || new.parts[n].observed_end_ns < old.parts[n].observed_end_ns
                {
                    return Err("temporal_quality_prefix_changed".into());
                }
                for source in &old.parts[n].sources {
                    if !new.parts[n].sources.iter().any(|s| {
                        s.source_id == source.source_id
                            && s.first_ns == source.first_ns
                            && s.first_sequence == source.first_sequence
                            && s.packets >= source.packets
                            && s.last_ns >= source.last_ns
                            && s.last_sequence >= source.last_sequence
                            && s.maximum_gap_ns >= source.maximum_gap_ns
                            && s.minimum_interval_ns <= source.minimum_interval_ns
                    }) {
                        return Err("temporal_quality_counts_regressed".into());
                    }
                }
            } else if segment {
                if new.parts.len() != old.parts.len() + 1
                    || new.parts[..old.parts.len()] != old.parts
                    || new.parts.last().is_none_or(|p| {
                        !p.sources.is_empty() || p.observed_end_ns != p.mapping.observed_start_ns
                    })
                {
                    return Err("temporal_quality_segment_changed".into());
                }
            } else if !pause {
                return Err("temporal_heldout_transition_invalid".into());
            }
            Ok(())
        }
        (
            FullStage::HeldOut {
                fit,
                train_seals,
                held_out,
            },
            FullStage::Complete {
                fit: next_fit,
                train_seals: next_train,
                held_out_seals,
                complete,
            },
        ) => {
            fixed(fit, train_seals, next_fit, next_train)?;
            if held_out.seals.len() != 8
                || held_out.disposition != bridge::Disposition::Complete
                || held_out.seals != *held_out_seals
                || !held_out.parts.is_empty()
                || a.common.mapping.is_some()
                || b.common.mapping.is_some()
                || a.common.episode != 15
                || b.common.episode != 15
                || a.common.ledger != b.common.ledger
                || serde_json::to_vec(&a.accepted).map_err(|_| "temporal_encoding")?
                    != serde_json::to_vec(&b.accepted).map_err(|_| "temporal_encoding")?
                || complete.body.complete_prepare_receiver_ns < b.common.last_now_ns
            {
                return Err("temporal_full_complete_transition_changed".into());
            }
            Ok(())
        }
        _ => Err("temporal_full_run_transition_unsupported".into()),
    }
}

#[cfg(test)]
pub(super) fn c4b_reference_batch_and_stale_seal_probe(
    bytes: &[u8],
    descriptor: &RecoveryDescriptor,
    now: u64,
) -> Result<serde_json::Value, String> {
    let mut envelope = parse(bytes)?;
    let data = &envelope.payload;
    let (train, held) = match &data.stage {
        FullStage::HeldOut {
            train_seals,
            held_out,
            ..
        } => (train_seals, &held_out.seals),
        FullStage::Complete{train_seals,held_out_seals,..}=>(train_seals,held_out_seals),
        _ => return Err("probe_requires_actual_heldout_or_complete_prefix".into()),
    };
    if train.len() != 7 || held.is_empty() || data.accepted.rows.len() < 72 {
        return Err("probe_requires_actual_sealed_suffix".into());
    }
    let mut cuts = Vec::new();
    let schedule = data.common.binding.contract()?;
    let mut count = 0;
    for (index, _) in train.iter().enumerate() {
        count += schedule.episodes()[index].slots.len();
        cuts.push(count);
    }
    cuts.extend((0..held.len()).map(|i| 64 + (i + 1) * 8));
    let batched = prefix_digests::canonical_tuple_prefixes(
        &data.accepted.rows,
        &data.accepted.evidence,
        &data.common.ledger.spans,
        &cuts,
    )?;
    for (index, cut) in cuts.iter().copied().enumerate() {
        let reference = hash(&(
            &data.accepted.rows[..cut],
            &data.accepted.evidence[..cut],
            &data.common.ledger.spans[..cut],
        ))?;
        if reference != batched.at(cut)? {
            return Err("probe_reference_batch_mismatch".into());
        }
        let stored = if index < 7 {
            &train[index].row_prefix_sha256
        } else {
            &held[index - 7].body.row_prefix_sha256
        };
        if stored != &reference {
            return Err("probe_stored_reference_mismatch".into());
        }
    }
    // Valid-shape HeldOut magnitude mutation: immutable Train/model/identities
    // unchanged, update the affected cohort/evidence and COMPLETE span chain.
    // Keep every existing HeldOut seal's expected tuple digest stale.
    let data = &mut envelope.payload;
    let old_span_hashes = data
        .common
        .ledger
        .spans
        .iter()
        .map(hash)
        .collect::<Result<Vec<_>, _>>()?;
    data.accepted.rows[64].views[0].samples[0][0] += 0.125;
    let cohort = hash(&data.accepted.rows[64])?;
    data.accepted.evidence[64].cohort_sha256 = cohort.clone();
    data.common.ledger.spans[64].window_sha256 = cohort;
    let mut changed = std::collections::HashMap::new();
    let mut previous = None;
    for (index, span) in data.common.ledger.spans.iter_mut().enumerate() {
        span.previous_sha256 = previous.clone();
        let new = hash(span)?;
        changed.insert(old_span_hashes[index].clone(), new.clone());
        previous = Some(new);
    }
    for gap in &mut data.common.ledger.gaps {
        if let Some(parent) = &mut gap.after_prefix_sha256 {
            *parent = changed
                .get(parent)
                .cloned()
                .ok_or("probe_missing_span_parent")?;
        }
    }
    envelope.payload_sha256 = hash(&envelope.payload)?;
    let altered = serde_json::to_vec(&envelope).map_err(|_| "probe_encoding")?;
    let error = decode_structural(&altered, descriptor, now)
        .err()
        .ok_or("probe_stale_heldout_seal_accepted")?;
    if error != "temporal_heldout_seal_prefix_changed" {
        return Err(format!("probe_negative_stopped_before_seal:{error}"));
    }
    Ok(
        serde_json::json!({"same_immutable_payload_bytes":bytes.len(),"cuts":cuts,"old_new_stored_all_equal":true,"sameN_mutation_no_stale_table":true,"mutation_recomputed_cohort_evidence_entire_span_chain_and_outer_sha":true,"negative_exact_error":error,"train_model_identity_unchanged":true}),
    )
}

#[cfg(test)]
pub(super) fn c4b_complete_fault_matrix(bytes:&[u8],descriptor:&RecoveryDescriptor,now:u64)->Result<serde_json::Value,String>{
    let mut results=vec![];
    for case in ["heldout_role","heldout_phase","common_phase","complete_kind","complete_model","complete_run","complete_deadline","complete_count","complete_floor","live_floor"]{
        let mut envelope=parse(bytes)?;let data=&mut envelope.payload;
        let FullStage::Complete{held_out_seals,complete,..}=&mut data.stage else{return Err("fault_requires_actual_complete".into())};
        match case{
            "heldout_role"=>{let mut body=held_out_seals[0].body.clone();body.role=DataRole::Train;held_out_seals[0]=held_out::HeldOutSealData::from_body(body)?;},
            "heldout_phase"=>{let mut body=held_out_seals[0].body.clone();body.episode=0;held_out_seals[0]=held_out::HeldOutSealData::from_body(body)?;},
            "common_phase"=>data.common.episode=0,
            "complete_kind"=>complete.body.artifact_kind="ruview.experimental.temporal-model-input.v2".into(),
            "complete_model"=>complete.body.artifact_sha256="0".repeat(64),
            "complete_run"=>complete.body.run.push_str("-forged"),
            "complete_deadline"=>complete.body.expires_at_ms+=1,
            "complete_count"=>complete.body.windows=127,
            "complete_floor"=>complete.body.source_floors[0].1=0,
            "live_floor"=>{data.common.source_high[0].1=0;complete.body.source_floors=data.common.source_high.clone();},
            _=>unreachable!(),
        }
        *complete=held_out::FullCompleteRecordData::from_body(complete.body.clone())?;
        envelope.payload_sha256=hash(&envelope.payload)?;let changed=serde_json::to_vec(&envelope).map_err(|_|"fault_encoding")?;
        let error=decode_structural(&changed,descriptor,now).err().ok_or_else(||format!("forged_complete_accepted:{case}"))?;
        results.push(serde_json::json!({"case":case,"error":error,"inner_and_outer_data_hashes_recomputed":true}));
    }
    Ok(serde_json::json!({"cases":results,"count":10,"raw_json_authority_issued":false}))
}
