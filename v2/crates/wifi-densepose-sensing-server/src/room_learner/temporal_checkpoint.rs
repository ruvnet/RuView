//! Typed complete native-prefix payload INSIDE existing RecoveryStore AEAD.
//! This never issues operator confirmation, scientific quality, or fit authority.
use super::*;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Prefix<R = CaptureCohort, E = WindowEvidence> {
    schema: String,
    binding: ManagedBinding,
    run: String,
    descriptor: RecoveryDescriptor,
    last_now_ns: u64,
    rows: Vec<R>,
    evidence: Vec<E>,
    identities: Vec<ManagedWindowIdentity>,
    fences: AssemblerCheckpoint,
    source_high: Vec<(u8, u64, u64)>,
    ledger: Ledger,
    mapping: Option<ObservedSegmentMap>,
    episode: usize,
    frozen_model: Option<String>,
    quality_issued: bool,
    confirmation_issued: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bridge: Option<BridgeState>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<P = Prefix> {
    schema: String,
    payload_sha256: String,
    payload: P,
}
const SCHEMA: &str = "ruview.room-learner.temporal-runtime-prefix.v1";
const BRIDGE_SCHEMA: &str = "ruview.room-learner.temporal-runtime-prefix.v2";
pub(super) type OldSnapshot = Prefix<SharedRecord<CaptureCohort>, SharedRecord<WindowEvidence>>;
#[path="temporal_full_run_checkpoint.rs"]
mod full_checkpoint;
pub(super) enum Snapshot {
    Old(OldSnapshot),
    Full(full_checkpoint::FullSnapshot),
}
#[derive(Clone)]
pub(super) struct SharedRecords {
    rows: Vec<SharedRecord<CaptureCohort>>,
    evidence: Vec<SharedRecord<WindowEvidence>>,
}
pub(super) fn snapshot_records(snapshot:&Snapshot)->SharedRecords {
    match snapshot {
        Snapshot::Old(snapshot)=>SharedRecords{rows:snapshot.rows.clone(),evidence:snapshot.evidence.clone()},
        Snapshot::Full(snapshot)=>snapshot.records(),
    }
}
pub(super) fn snapshot_full_permit(snapshot:&mut Snapshot)->Option<full_run::FullRunSnapshotPermit>{
    match snapshot{Snapshot::Old(_)=>None,Snapshot::Full(snapshot)=>snapshot.permit.take()}
}
pub(super) fn workflow_records(w:&TemporalWorkflow)->SharedRecords {
    SharedRecords{rows:w.session.rows.clone(),evidence:w.session.evidence.clone()}
}
pub(super) fn workflow_record_prefix(w:&TemporalWorkflow,count:usize)->Result<SharedRecords,String>{
    if count>w.session.rows.len()||count>w.session.evidence.len(){return Err("temporal_record_prefix_missing".into());}
    Ok(SharedRecords{rows:w.session.rows[..count].to_vec(),evidence:w.session.evidence[..count].to_vec()})
}
fn reuse_snapshot_records(w:&mut TemporalWorkflow,records:&SharedRecords)->Result<(),String>{
    if w.session.rows.len()!=records.rows.len() || records.rows.len()!=records.evidence.len()
        || hash(&w.session.evidence)?!=hash(&records.evidence)? {
        return Err("temporal_writer_shared_records_changed".into());
    }
    // decode() already validates every decoded row's exact canonical SHA
    // against its WindowEvidence. Validate the SAME expected immutable rows
    // against those evidence hashes once, then reuse their cached canonical
    // hashes on later prefixes. This is not a new hash formula or authority.
    for (row,evidence) in records.rows.iter().zip(&records.evidence) {
        if row.canonical_sha256()?!=evidence.cohort_sha256 {return Err("temporal_writer_shared_records_changed".into());}
    }
    // This call executes on the writer. Free validation's decoded raw copies
    // here; the receiver completion contains only refs to its SAME immutable
    // authoritative rows/evidence plus bounded reducer metadata.
    w.session.rows=records.rows.clone();w.session.evidence=records.evidence.clone();Ok(())
}
pub(super) fn decode_shared(
    bytes:&[u8], descriptor:&RecoveryDescriptor, now:u64, records:&SharedRecords,
)->Result<TemporalWorkflow,String>{
    // Keep validation and immutable-ref reuse one private admission boundary.
    // No caller-provided/mutated workflow can substitute for this decode.
    let mut validated=decode(bytes,descriptor,now)?;
    reuse_snapshot_records(&mut validated,records)?;
    Ok(validated)
}
pub(super) fn snapshot(v: &TemporalWorkflow) -> Result<Snapshot, String> {
    if v.full_run.is_some(){return Ok(Snapshot::Full(full_checkpoint::FullSnapshot::from_workflow(v)?));}
    Ok(Snapshot::Old(snapshot_old(v)?))
}
fn snapshot_old(v: &TemporalWorkflow) -> Result<OldSnapshot, String> {
    if v.session.fitted
        || v.session.failed
        || v.session.active
        || v.session.rows.iter().any(|r| r.role != DataRole::Train)
        || v.session.rows.len() != v.ledger.spans.len()
    {
        return Err("temporal_checkpoint_complete_train_prefix_only".into());
    }
    let m = v
        .session
        .managed
        .as_ref()
        .ok_or("managed_session_required")?;
    let schema = if v.bridge.is_some() {
        BRIDGE_SCHEMA
    } else {
        SCHEMA
    };
    let payload = Prefix {
        schema: schema.into(),
        binding: m.binding.clone(),
        run: v.session.run_id.clone(),
        descriptor: v.descriptor.clone(),
        last_now_ns: v.last_now_ns,
        rows: v.session.rows.clone(),
        evidence: v.session.evidence.clone(),
        identities: m.windows.clone(),
        fences: v.session.assembler.fence_checkpoint()?,
        source_high: v.source_high.clone(),
        ledger: v.ledger.clone(),
        mapping: v.mapping.clone(),
        episode: v.episode,
        frozen_model: None,
        quality_issued: false,
        confirmation_issued: false,
        bridge: v.bridge.clone(),
    };
    Ok(payload)
}
pub(super) fn encode_snapshot(payload: Snapshot) -> Result<Vec<u8>, String> {
    match payload {
        Snapshot::Old(payload)=>encode_old_snapshot(payload),
        Snapshot::Full(payload)=>payload.encode(),
    }
}
fn encode_old_snapshot(payload: OldSnapshot) -> Result<Vec<u8>, String> {
    let envelope = Envelope {
        schema: payload.schema.clone(),
        payload_sha256: hash(&payload)?,
        payload,
    };
    let bytes = serde_json::to_vec(&envelope).map_err(|_| "temporal_checkpoint_encoding")?;
    if bytes.len() > crate::room_learner::recovery_store::MAX_RECOVERY_BYTES {
        return Err("temporal_checkpoint_budget".into());
    }
    Ok(bytes)
}
pub(super) fn encode(v: &TemporalWorkflow) -> Result<Vec<u8>, String> {
    encode_snapshot(snapshot(v)?)
}
pub(super) fn decode(
    bytes: &[u8],
    descriptor: &RecoveryDescriptor,
    now: u64,
) -> Result<TemporalWorkflow, String> {
    if full_checkpoint::is_full(bytes)? {return full_checkpoint::decode_structural(bytes,descriptor,now);}
    decode_old(bytes,descriptor,now)
}
fn decode_old(bytes:&[u8],descriptor:&RecoveryDescriptor,now:u64)->Result<TemporalWorkflow,String>{
    if bytes.len() > crate::room_learner::recovery_store::MAX_RECOVERY_BYTES {
        return Err("temporal_checkpoint_budget".into());
    }
    let e: Envelope = serde_json::from_slice(bytes).map_err(|_| "temporal_checkpoint_schema")?;
    if !matches!(e.schema.as_str(), SCHEMA | BRIDGE_SCHEMA)
        || e.payload.schema != e.schema
        || (e.schema == SCHEMA) != e.payload.bridge.is_none()
        || e.payload_sha256 != hash(&e.payload)?
        || serde_json::to_vec(&e).map_err(|_| "temporal_checkpoint_encoding")? != bytes
    {
        return Err("temporal_checkpoint_integrity".into());
    }
    let p = e.payload;
    let schedule = p.binding.contract()?;
    let context = schedule.context();
    if &p.descriptor != descriptor
        || p.descriptor.owner_scope != context.owner_scope
        || p.descriptor.installation_id != context.installation_id
        || p.descriptor.context_digest != context.digest()
        || p.last_now_ns / 1_000_000 != descriptor.saved_at_ms
        || p.last_now_ns > now
        || now / 1_000_000 >= descriptor.expires_at_ms
        || descriptor
            .expires_at_ms
            .checked_sub(descriptor.created_at_ms)
            != Some(RECOVERY_TTL_MS)
        || p.frozen_model.is_some()
        || p.quality_issued
        || p.confirmation_issued
        || p.episode >= schedule.episodes().len()
        || p.rows.len() > schedule.counts().train_windows
        || p.rows.len() != p.evidence.len()
        || p.rows.len() != p.identities.len()
        || p.rows.len() != p.ledger.spans.len()
        || p.ledger.gaps.len() > 128
        || p.source_high.len() > context.sources.len()
        || (!p.rows.is_empty() && p.source_high.len() != context.sources.len())
    {
        return Err("temporal_checkpoint_binding_or_unsupported_state".into());
    }
    if let Some(mapping) = &p.mapping {
        if mapping.segment_ordinal >= 128 || mapping.receiver_start_ns > p.last_now_ns {
            return Err("temporal_checkpoint_mapping".into());
        }
        let bound =
            schedule.bind_episode_start(&p.run, p.episode, 0, mapping.segment_ordinal, None)?;
        let slot = schedule.episodes()[p.episode]
            .slots
            .iter()
            .find(|s| s.target_offset_ms * 1_000_000 >= mapping.observed_start_ns)
            .ok_or("temporal_checkpoint_mapping")?;
        mapping.slot_bounds(
            &bound,
            slot.target_offset_ms * 1_000_000,
            slot.deadline_offset_ms * 1_000_000,
        )?;
        let expected_start = p
            .ledger
            .gaps
            .iter()
            .rev()
            .find(|g| g.episode == p.episode && g.new_mapping == *mapping)
            .map_or(0, |g| g.accepted_observed_ns);
        if mapping.observed_start_ns != expected_start {
            return Err("temporal_checkpoint_mapping_start".into());
        }
    } else if !p.rows.is_empty() && p.bridge.as_ref().is_none_or(|b| b.seals.len() != p.episode) {
        return Err("temporal_checkpoint_mapping_missing".into());
    }
    validate_records(
        &schedule,
        &p.run,
        &p.evidence,
        &p.identities,
        &p.binding.validation_binding()?,
        true,
    )?;
    let mut retained = Vec::new();
    let mut prior: Option<String> = None;
    for (i, ((row, evidence), span)) in p
        .rows
        .iter()
        .zip(&p.evidence)
        .zip(&p.ledger.spans)
        .enumerate()
    {
        validate_cohort(context, row)?;
        if row.views.len() != evidence.sources.len() {
            return Err("temporal_checkpoint_row_evidence_shape".into());
        }
        for (view, source) in row.views.iter().zip(&evidence.sources) {
            if view.source_id != source.source_id
                || view.samples.len() != source.times_ns.len()
                || view
                    .frame_ids
                    .iter()
                    .map(u128::to_string)
                    .collect::<Vec<_>>()
                    != source.frame_ids
                || view.sequence_ids != source.sequence_ids
                || source.times_ns.first() != Some(&view.first_time_ns.to_string())
                || source.times_ns.last() != Some(&view.last_time_ns.to_string())
            {
                return Err("temporal_checkpoint_original_row_identity".into());
            }
        }
        let record = &p.identities[i];
        let episode = &schedule.episodes()[record.episode_index];
        let slot = &episode.slots[record.slot_index];
        if hash(row)? != evidence.cohort_sha256
            || row.id != evidence.cohort_id
            || row.trial_id != evidence.trial_id
            || row.session_id != evidence.session_id
            || row.role != DataRole::Train
            || row.label != evidence.label
            || span.previous_sha256 != prior
            || span.window_sha256 != hash(row)?
            || span.mapping != record.mapping
            || span.episode != record.episode_index
            || span.slot != record.slot_index
            || span.observed_end_ns != slot.deadline_offset_ms * 1_000_000
            || span.receiver_end_ns != integer::<u64>(&record.end_ns)?
            || span.receiver_end_ns > p.last_now_ns
        {
            return Err("temporal_checkpoint_row_or_span_changed".into());
        }
        let expected_start = if i > 0 && p.ledger.spans[i - 1].episode == span.episode {
            p.ledger.spans[i - 1].observed_end_ns
        } else {
            0
        };
        if span.observed_start_ns != expected_start
            || span.observed_end_ns <= span.observed_start_ns
        {
            return Err("temporal_committed_span_overlap".into());
        }
        for source in &evidence.sources {
            for ((t, f), seq) in source
                .times_ns
                .iter()
                .zip(&source.frame_ids)
                .zip(&source.sequence_ids)
            {
                retained.push((
                    source.source_id,
                    *seq,
                    integer::<u64>(t)?,
                    integer::<u128>(f)?,
                ));
            }
        }
        prior = Some(hash(span)?);
    }
    for (gap_index, gap) in p.ledger.gaps.iter().enumerate() {
        if gap.episode >= schedule.episodes().len() {
            return Err("temporal_gap_episode".into());
        }
        let bound = schedule.bind_episode_start(
            &p.run,
            gap.episode,
            0,
            gap.new_mapping.segment_ordinal,
            None,
        )?;
        let next_slot = schedule.episodes()[gap.episode]
            .slots
            .iter()
            .find(|s| s.target_offset_ms * 1_000_000 >= gap.accepted_observed_ns)
            .ok_or("temporal_gap_mapping_changed")?;
        gap.new_mapping.slot_bounds(
            &bound,
            next_slot.target_offset_ms * 1_000_000,
            next_slot.deadline_offset_ms * 1_000_000,
        )?;
        if let Some(parent) = &gap.after_prefix_sha256 {
            if gap.genesis.is_some() {
                return Err("temporal_gap_conflicting_parent".into());
            }
            let span = p
                .ledger
                .spans
                .iter()
                .find(|s| hash(s).ok().as_ref() == Some(parent))
                .ok_or("temporal_gap_prefix_missing")?;
            if gap.episode != span.episode
                || gap.accepted_observed_ns != span.observed_end_ns
                || gap.previous_receiver_ns != span.receiver_end_ns
            {
                return Err("temporal_gap_mapping_changed".into());
            }
        } else {
            let genesis = gap.genesis.as_ref().ok_or("temporal_gap_genesis_missing")?;
            let previous = &genesis.previous_mapping;
            if gap.accepted_observed_ns != 0
                || previous.observed_start_ns != 0
                || genesis.sha256 != genesis.digest(descriptor, &p.run, gap.episode)?
                || genesis.paused_receiver_ns < previous.receiver_start_ns
                || gap.previous_receiver_ns != genesis.paused_receiver_ns
                || gap.new_mapping.segment_ordinal
                    != previous
                        .segment_ordinal
                        .checked_add(1)
                        .ok_or("temporal_gap_segment_overflow")?
                || p.ledger.spans.iter().any(|s| {
                    s.episode == gap.episode
                        && s.mapping.segment_ordinal <= previous.segment_ordinal
                })
            {
                return Err("temporal_gap_invalid_genesis".into());
            }
            let prior_bound = schedule.bind_episode_start(
                &p.run,
                gap.episode,
                0,
                previous.segment_ordinal,
                None,
            )?;
            let first = &schedule.episodes()[gap.episode].slots[0];
            previous.slot_bounds(
                &prior_bound,
                first.target_offset_ms * 1_000_000,
                first.deadline_offset_ms * 1_000_000,
            )?;
            if let Some(prior) = p.ledger.gaps[..gap_index]
                .iter()
                .rev()
                .find(|g| g.episode == gap.episode)
            {
                if &prior.new_mapping != previous {
                    return Err("temporal_gap_genesis_history_changed".into());
                }
            } else if previous.segment_ordinal != 0 {
                return Err("temporal_gap_genesis_history_missing".into());
            }
            if genesis.source_floors.len() > context.sources.len() {
                return Err("temporal_gap_genesis_floors".into());
            }
            let mut seen = HashSet::new();
            for (id, seq, t) in &genesis.source_floors {
                if !seen.insert(*id)
                    || !context.sources.iter().any(|s| s.source_id == *id)
                    || *seq == 0
                    || *t > genesis.paused_receiver_ns
                    || !p
                        .source_high
                        .iter()
                        .any(|(s, q, n)| s == id && q >= seq && n >= t)
                {
                    return Err("temporal_gap_genesis_floors".into());
                }
            }
        }
        if gap.resume_receiver_ns < gap.previous_receiver_ns
            || gap.new_mapping.observed_start_ns != gap.accepted_observed_ns
            || gap.new_mapping.receiver_start_ns != gap.resume_receiver_ns
            || gap.new_mapping.clock_anchor_ns < gap.previous_receiver_ns
        {
            return Err("temporal_gap_mapping_changed".into());
        }
    }
    if let Some(bridge) = &p.bridge {
        bridge.validate(
            &schedule,
            &p.run,
            &p.rows,
            &p.evidence,
            &p.ledger,
            p.episode,
            p.mapping.as_ref(),
            p.last_now_ns,
        )?;
        // Whole-period quality also observes the reservation tail and packets
        // between retained strata. Those durably acknowledged observations
        // must not disappear from the replay floors merely because they are
        // not samples in a retained native window.
        for part in bridge
            .seals
            .iter()
            .flat_map(|seal| &seal.parts)
            .chain(&bridge.parts)
        {
            for observed in &part.sources {
                if !p.source_high.iter().any(|(id, sequence, time)| {
                    *id == observed.source_id
                        && *sequence >= observed.last_sequence
                        && *time >= observed.last_ns
                }) {
                    return Err("temporal_quality_floors_regressed".into());
                }
            }
        }
    }
    let mut session =
        TemporalTrainingSession::managed(schedule.clone(), p.run, p.binding.fit.clone())?;
    session.rows = p.rows.into_iter().map(SharedRecord::new).collect();
    session.evidence = p.evidence.into_iter().map(SharedRecord::new).collect();
    session.managed.as_mut().unwrap().windows = p.identities;
    session
        .assembler
        .restore_prefix_fences(p.fences, session.rows.len())?;
    if !session.assembler.retained_fences_cover(&retained) {
        return Err("temporal_checkpoint_fences_regressed".into());
    }
    let mut seen = HashSet::new();
    for (id, seq, t) in &p.source_high {
        if !seen.insert(*id)
            || !context.sources.iter().any(|s| s.source_id == *id)
            || *seq == 0
            || *t > p.last_now_ns
            || retained
                .iter()
                .any(|(s, q, n, _)| s == id && (*q > *seq || *n > *t))
        {
            return Err("temporal_checkpoint_source_floor".into());
        }
    }
    let accepted = p
        .ledger
        .spans
        .last()
        .filter(|s| s.episode == p.episode)
        .map_or(0, |s| s.observed_end_ns);
    let durable_windows = session.rows.len();
    Ok(TemporalWorkflow {
        full_run: None,
        session,
        descriptor: p.descriptor,
        ledger: p.ledger,
        stage: Stage::PausedNeedsReconfirmation,
        mapping: p.mapping,
        episode: p.episode,
        last_now_ns: p.last_now_ns,
        live_observed_ns: accepted,
        source_high: p.source_high,
        pending_window: None,
        durable: None,
        durable_windows,
        custody: SegmentCustody::default(),
        commit_token: None,
        pending_intent: None,
        pending_prefix: None,
        bridge: p.bridge,
        async_state: None,
    })
}
#[cfg(test)]
#[path = "temporal_checkpoint_tests.rs"]
mod tests;

/// Opaque, crate-local storage input; generic JSON cannot claim validity/proof.
#[derive(Clone)]
pub(crate) struct ValidatedTemporalPrefix {
    bytes: Vec<u8>,
    descriptor: RecoveryDescriptor,
    run: String,
    policy_sha256: String,
    window_count: usize,
    transition_limit: usize,
    saved_receiver_ns: u64,
    metadata_kind: &'static str,
    full_write_grant: Option<full_run::FullRunWriteGrant>,
    full_frozen_predecessor: Option<crate::room_learner::recovery_store::temporal_commitment::TemporalCommitmentData>,
}
pub(crate) fn validated_prefix(
    bytes: &[u8],
    descriptor: &RecoveryDescriptor,
    now_ns: u64,
) -> Result<ValidatedTemporalPrefix, String> {
    if full_checkpoint::is_full(bytes)?{return full_checkpoint::validated_structural(bytes,descriptor,now_ns);}
    let v = decode(bytes, descriptor, now_ns)?;
    let schedule = v.schedule()?;
    // Historical profiles retain their ORIGINAL limit/hash; only an explicit
    // fresh-Train genesis can select the recognized343 budget profile.
    let profile=match &v.bridge{Some(bridge)=>bridge.profile()?,None=>bridge::TrainBudgetProfile::Historical329};
    let transition_limit=profile.transition_limit(&schedule)?;
    if transition_limit > 512 {
        return Err("temporal_transition_capacity".into());
    }
    Ok(ValidatedTemporalPrefix {
        bytes: bytes.to_vec(),
        descriptor: descriptor.clone(),
        run: v.session.run_id.clone(),
        policy_sha256: if v.bridge.is_some() {
            hash(&(v.session.plan_sha256(), profile.policy()))?
        } else {
            v.session.plan_sha256().into()
        },
        window_count: v.session.rows.len(),
        transition_limit,
        saved_receiver_ns: v.last_now_ns,
        metadata_kind:"temporal-prefix-commitment-v1",
        full_write_grant:None,
        full_frozen_predecessor:None,
    })
}
pub(crate) fn validate_extension(
    old: &ValidatedTemporalPrefix,
    new: &ValidatedTemporalPrefix,
) -> Result<(), String> {
    if old.metadata_kind!=new.metadata_kind{return Err("temporal_prefix_generation_kind_changed".into());}
    if old.metadata_kind==full_run::META_KIND{return full_checkpoint::validate_full_extension(old,new);}
    let a: Envelope =
        serde_json::from_slice(&old.bytes).map_err(|_| "temporal_checkpoint_schema")?;
    let b: Envelope =
        serde_json::from_slice(&new.bytes).map_err(|_| "temporal_checkpoint_schema")?;
    let a = a.payload;
    let b = b.payload;
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
        || b.last_now_ns < a.last_now_ns
    {
        return Err("temporal_commit_identity_or_time_changed".into());
    }
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
    prefix!(a.rows, b.rows);
    prefix!(a.evidence, b.evidence);
    prefix!(a.identities, b.identities);
    prefix!(a.ledger.spans, b.ledger.spans);
    prefix!(a.ledger.gaps, b.ledger.gaps);
    for (id, seq, t) in &a.source_high {
        if !b
            .source_high
            .iter()
            .any(|(s, q, n)| s == id && q >= seq && n >= t)
        {
            return Err("temporal_commit_floor_regressed".into());
        }
    }
    if old.bytes == new.bytes {
        return Ok(());
    }
    let window = b.rows.len() == a.rows.len() + 1
        && b.ledger.gaps.len() == a.ledger.gaps.len()
        && b.mapping == a.mapping;
    let segment = b.rows.len() == a.rows.len()
        && ((a.mapping.is_none()
            && b.mapping.is_some()
            && b.ledger.gaps.len() == a.ledger.gaps.len())
            || (b.ledger.gaps.len() == a.ledger.gaps.len() + 1));
    match (&a.bridge, &b.bridge) {
        (None, None) => {
            if !window && !segment {
                return Err("temporal_transition_without_window_or_segment".into());
            }
        }
        (Some(old), Some(new)) => {
            prefix!(old.seals, new.seals);
            let seal = b.rows.len() == a.rows.len()
                && b.episode == a.episode + 1
                && b.ledger.gaps == a.ledger.gaps
                && b.mapping.is_none()
                && new.seals.len() == old.seals.len() + 1
                && new.parts.is_empty()
                && new
                    .seals
                    .last()
                    .is_some_and(|s| s.parts == old.parts && s.confirmed_at_ns >= a.last_now_ns);
            let pause = b.rows.len() == a.rows.len()
                && b.episode == a.episode
                && b.mapping == a.mapping
                && b.ledger.gaps == a.ledger.gaps
                && new.seals == old.seals
                && new.parts == old.parts
                && new.disposition == bridge::Disposition::Paused;
            if seal {
                return Ok(());
            }
            if new.seals != old.seals || b.episode != a.episode {
                return Err("temporal_bridge_seal_transition".into());
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
                return Err("temporal_bridge_transition_invalid".into());
            }
        }
        _ => return Err("temporal_bridge_kind_changed".into()),
    }
    Ok(())
}

impl ValidatedTemporalPrefix {
    pub(crate) fn metadata_kind(&self)->&str{self.metadata_kind}
    pub(crate) fn authorizes_write(&self,expected:Option<&TemporalToken>)->bool{
        self.metadata_kind!=full_run::META_KIND||(self.full_write_grant.as_ref().is_some_and(|g|g.permits(&self.bytes,expected))
            &&self.full_frozen_predecessor.as_ref().is_none_or(|p|expected.is_some_and(|e|e.data()==*p)))
    }
    pub(super) fn verify_full_artifact(&self)->Result<(),String>{
        if self.metadata_kind==full_run::META_KIND&&self.full_frozen_predecessor.is_some(){
            return self.full_write_grant.as_ref().ok_or("temporal_frozen_producer_model_required")?.verify_frozen_model();
        }
        Ok(())
    }
    pub(super) fn attach_full_grant(&mut self,grant:full_run::FullRunWriteGrant)->Result<(),String>{
        if self.metadata_kind!=full_run::META_KIND{return Err("temporal_full_run_grant_kind".into());}
        self.full_write_grant=Some(grant);Ok(())
    }
    pub(crate) fn saved_receiver_ns(&self) -> u64 {
        self.saved_receiver_ns
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(crate) fn descriptor(&self) -> &RecoveryDescriptor {
        &self.descriptor
    }
    pub(crate) fn run(&self) -> &str {
        &self.run
    }
    pub(crate) fn policy_sha256(&self) -> &str {
        &self.policy_sha256
    }
    pub(crate) fn window_count(&self) -> usize {
        self.window_count
    }
    pub(crate) fn transition_limit(&self) -> usize {
        self.transition_limit
    }
}

pub(crate) fn is_new_empty_run(p: &ValidatedTemporalPrefix) -> Result<bool, String> {
    if p.metadata_kind==full_run::META_KIND{return full_checkpoint::is_new_full_run(p);}
    let e: Envelope =
        serde_json::from_slice(p.bytes()).map_err(|_| "temporal_checkpoint_schema")?;
    let q = e.payload;
    let schedule = q.binding.contract()?;
    let fresh = TemporalTrainingSession::managed(schedule, q.run.clone(), q.binding.fit.clone())?;
    Ok(q.rows.is_empty()
        && q.evidence.is_empty()
        && q.identities.is_empty()
        && q.ledger.spans.is_empty()
        && q.ledger.gaps.is_empty()
        && q.source_high.is_empty()
        && q.mapping.is_none()
        && q.episode == 0
        && q.bridge.as_ref().is_none_or(|b| {
            b.seals.is_empty() && b.parts.is_empty() && b.disposition == bridge::Disposition::Ready
        })
        && serde_json::to_vec(&q.fences).map_err(|_| "temporal_encoding")?
            == serde_json::to_vec(&fresh.assembler.fence_checkpoint()?)
                .map_err(|_| "temporal_encoding")?)
}

#[cfg(test)]
pub(crate) fn synthetic_storage_prefixes() -> Vec<ValidatedTemporalPrefix> {
    let (_dir, store, mut w) = super::tests::setup();
    let mut out = Vec::new();
    let add = |w: &TemporalWorkflow| {
        let bytes = encode(w).unwrap();
        validated_prefix(&bytes, &w.descriptor, w.last_now_ns).unwrap()
    };
    out.push(add(&w));
    w.persist_initial(&store).unwrap();
    super::tests::start(&mut w, &store, 2_000_000_000, "one");
    out.push(add(&w));
    let mut seq = 0;
    let next = super::tests::commit_next(&mut w, &store, 2_000_000_000, &mut seq);
    out.push(add(&w));
    super::tests::commit_next(&mut w, &store, next, &mut seq);
    out.push(add(&w));
    out
}
#[cfg(test)]
pub(crate) fn synthetic_segment_fork(initial: &ValidatedTemporalPrefix) -> ValidatedTemporalPrefix {
    let mut w = decode(
        initial.bytes(),
        initial.descriptor(),
        initial.descriptor().saved_at_ms * 1_000_000,
    )
    .unwrap();
    w.stage = Stage::DurableReady;
    w.durable = Some(std::sync::Arc::new(initial.bytes().to_vec()));
    let p = w.synthetic_conditions();
    w.start_segment(&p, "different-segment", 2_000_000_000, 2_000_000_000)
        .unwrap();
    w.descriptor.saved_at_ms = w.last_now_ns / 1_000_000;
    let b = encode(&w).unwrap();
    validated_prefix(&b, &w.descriptor, w.last_now_ns).unwrap()
}
#[cfg(test)]
pub(crate) fn synthetic_changed_old_row(
    input: &ValidatedTemporalPrefix,
) -> ValidatedTemporalPrefix {
    let mut e: Envelope = serde_json::from_slice(input.bytes()).unwrap();
    e.payload.rows[0].views[0].samples[0][0] += 1.;
    e.payload.evidence[0].cohort_sha256 = hash(&e.payload.rows[0]).unwrap();
    e.payload.ledger.spans[0].window_sha256 = hash(&e.payload.rows[0]).unwrap();
    for i in 1..e.payload.ledger.spans.len() {
        e.payload.ledger.spans[i].previous_sha256 =
            Some(hash(&e.payload.ledger.spans[i - 1]).unwrap());
    }
    e.payload_sha256 = hash(&e.payload).unwrap();
    let b = serde_json::to_vec(&e).unwrap();
    validated_prefix(&b, &e.payload.descriptor, e.payload.last_now_ns).unwrap()
}
#[cfg(test)]
pub(crate) fn synthetic_new_empty_run(
    input: &ValidatedTemporalPrefix,
    created: u64,
) -> ValidatedTemporalPrefix {
    let mut e: Envelope = serde_json::from_slice(input.bytes()).unwrap();
    assert!(e.payload.rows.is_empty());
    e.payload.run = "synthetic-next-run".into();
    e.payload.descriptor.id = "synthetic-new-recovery-id".into();
    e.payload.descriptor.created_at_ms = created;
    e.payload.descriptor.saved_at_ms = created;
    e.payload.descriptor.expires_at_ms = created + RECOVERY_TTL_MS;
    e.payload.last_now_ns = created * 1_000_000;
    e.payload_sha256 = hash(&e.payload).unwrap();
    let b = serde_json::to_vec(&e).unwrap();
    validated_prefix(&b, &e.payload.descriptor, e.payload.last_now_ns).unwrap()
}
#[cfg(test)]
pub(crate) fn synthetic_floor_regression(
    input: &ValidatedTemporalPrefix,
) -> ValidatedTemporalPrefix {
    let mut w = decode(input.bytes(), input.descriptor(), input.saved_receiver_ns()).unwrap();
    w.durable = Some(std::sync::Arc::new(input.bytes().to_vec()));
    let permit = w.synthetic_conditions();
    let start = input.saved_receiver_ns() + 10_000_000_000;
    w.start_segment(&permit, "floor-regression", start, start)
        .unwrap();
    w.descriptor.saved_at_ms = w.last_now_ns / 1_000_000;
    let b = encode(&w).unwrap();
    let mut e: Envelope = serde_json::from_slice(&b).unwrap();
    let source = &e.payload.evidence.last().unwrap().sources[0];
    let minimum = *source.sequence_ids.last().unwrap();
    let old = e.payload.source_high[0].1;
    assert!(old > minimum);
    e.payload.source_high[0].1 = minimum;
    e.payload_sha256 = hash(&e.payload).unwrap();
    let b = serde_json::to_vec(&e).unwrap();
    validated_prefix(&b, &e.payload.descriptor, e.payload.last_now_ns).unwrap()
}

// Test-only typed fixture: create a NEW empty run and encode its explicit
// profile through the same strict codec; never upgrades an existing prefix.
#[cfg(test)]
pub(crate) fn synthetic_fresh_train343_empty_run(input:&ValidatedTemporalPrefix,created:u64)->ValidatedTemporalPrefix{
    let empty=synthetic_new_empty_run(input,created);
    let mut workflow=decode(empty.bytes(),empty.descriptor(),empty.saved_receiver_ns()).unwrap();
    workflow.bridge=Some(super::bridge::BridgeState::fresh_train_343());
    let bytes=encode(&workflow).unwrap();validated_prefix(&bytes,&workflow.descriptor,workflow.last_now_ns).unwrap()
}
