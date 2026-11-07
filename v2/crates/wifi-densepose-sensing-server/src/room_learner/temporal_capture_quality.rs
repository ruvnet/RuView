//! Private Runtime capture facade. Whole-period receiver quality is engineering
//! admission only: never source-clock precision, teacher truth or qualification.
//! Seals alone grant no fit authority; the private owner must separately consume
//! the complete durable producer prefix into its opaque fit-only admission.
use super::*;
use crate::room_learner::temporal_acquisition::WindowInputPolicy;
use serde_json::{json, Value};

pub(super) const BRIDGE_POLICY: &str = "native-receiver-whole-train-period-v1;all-selected-packets;existing-cadence-gap-arrival-skew;confirmed-durable-seals;no-fit-authority";
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Disposition {
    Ready,
    Observing,
    Paused,
    NeedsConfirmation,
    Complete,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceQuality {
    pub(super) source_id: u8,
    pub(super) first_ns: u64,
    pub(super) last_ns: u64,
    pub(super) first_sequence: u64,
    pub(super) last_sequence: u64,
    pub(super) packets: u64,
    pub(super) maximum_gap_ns: u64,
    pub(super) minimum_interval_ns: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct QualityPart {
    pub(super) mapping: ObservedSegmentMap,
    pub(super) observed_end_ns: u64,
    pub(super) sources: Vec<SourceQuality>,
}
impl QualityPart {
    fn observe(&mut self, p: &CanonicalPacket) -> Result<(), String> {
        let t = p.native_frame.timestamp_ns;
        if let Some(s) = self.sources.iter_mut().find(|s| s.source_id == p.source_id) {
            let gap = t.checked_sub(s.last_ns).ok_or("temporal_quality_clock")?;
            s.maximum_gap_ns = s.maximum_gap_ns.max(gap);
            s.minimum_interval_ns = s.minimum_interval_ns.min(gap);
            s.last_ns = t;
            s.last_sequence = u64::from(p.sequence);
            s.packets = s.packets.checked_add(1).ok_or("temporal_quality_count")?;
        } else {
            self.sources.push(SourceQuality {
                source_id: p.source_id,
                first_ns: t,
                last_ns: t,
                first_sequence: u64::from(p.sequence),
                last_sequence: u64::from(p.sequence),
                packets: 1,
                maximum_gap_ns: 0,
                minimum_interval_ns: u64::MAX,
            });
            self.sources.sort_by_key(|s| s.source_id);
        }
        Ok(())
    }
    fn validate(
        &self,
        native: &WindowInputPolicy,
        bound: &BoundEpisode,
        now: u64,
    ) -> Result<(), String> {
        let start = self.mapping.observed_start_ns;
        if self.observed_end_ns < start
            || self.observed_end_ns > bound.episode().target_active_recording_ms * 1_000_000
            || self.mapping.receiver_start_ns > now
            || self.sources.len() > native.source_policies().len()
        {
            return Err("temporal_quality_part_bounds".into());
        }
        if self.observed_end_ns == start && !self.sources.is_empty() {
            return Err("temporal_quality_uncommitted_rows".into());
        }
        let mut prior = None;
        for s in &self.sources {
            let policy = native
                .source_policies()
                .iter()
                .find(|p| p.source_id == s.source_id)
                .ok_or("temporal_quality_source")?;
            if prior.is_some_and(|p| p >= s.source_id)
                || s.packets == 0
                || s.packets > 1_000_000
                || s.first_ns < self.mapping.receiver_start_ns
                || s.last_ns < s.first_ns
                || s.last_ns > now
                || s.first_ns - self.mapping.receiver_start_ns
                    > native.max_first_packet_delay_ms() * 1_000_000
                || s.last_sequence < s.first_sequence
                || s.last_sequence - s.first_sequence + 1 < s.packets
                || (s.packets == 1
                    && (s.first_ns != s.last_ns
                        || s.maximum_gap_ns != 0
                        || s.minimum_interval_ns != u64::MAX))
                || (s.packets > 1
                    && (s.last_ns == s.first_ns
                        || s.minimum_interval_ns < policy.min_packet_interval_ns
                        || s.maximum_gap_ns > policy.max_gap_ns
                        || s.minimum_interval_ns > s.maximum_gap_ns))
            {
                return Err("temporal_quality_source_bounds".into());
            }
            if self.observed_end_ns > start {
                let end = self
                    .mapping
                    .receiver_start_ns
                    .checked_add(self.observed_end_ns - start)
                    .ok_or("temporal_quality_time")?;
                let (lo, hi) = policy.receiver_cadence.rate_bounds_millihz();
                let intervals = s.packets.saturating_sub(1) as u128;
                let span = (s.last_ns - s.first_ns) as u128;
                // The exact integer packet/rate bounds already declared by the
                // native policy, including its integer interval representation.
                if s.packets < 2
                    || s.last_ns < end
                    || intervals < span * u128::from(lo) / 1_000_000_000_000u128
                    || span < intervals * u128::from(1_000_000_000_000u64 / hi)
                {
                    return Err("temporal_whole_period_cadence".into());
                }
            }
            prior = Some(s.source_id);
        }
        if self.observed_end_ns > start
            && self
                .sources
                .iter()
                .map(|s| s.source_id)
                .ne(native.source_policies().iter().map(|s| s.source_id))
        {
            return Err("temporal_quality_required_sources".into());
        }
        if let (Some(lo), Some(hi)) = (
            self.sources.iter().map(|s| s.first_ns).min(),
            self.sources.iter().map(|s| s.first_ns).max(),
        ) {
            if hi - lo > native.max_pairwise_receiver_skew_ms() * 1_000_000 {
                return Err("temporal_quality_source_skew".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TrainSeal {
    pub(super) episode: usize,
    pub(super) trial_id: String,
    pub(super) context_sha256: String,
    pub(super) schedule_sha256: String,
    pub(super) row_prefix_sha256: String,
    pub(super) quality_sha256: String,
    pub(super) parts: Vec<QualityPart>,
    pub(super) confirmed_at_ns: u64,
    pub(super) previous_sha256: Option<String>,
    pub(super) sha256: String,
}
impl TrainSeal {
    fn digest(&self) -> Result<String, String> {
        hash(&(
            BRIDGE_POLICY,
            self.episode,
            &self.trial_id,
            &self.context_sha256,
            &self.schedule_sha256,
            &self.row_prefix_sha256,
            &self.quality_sha256,
            &self.parts,
            self.confirmed_at_ns,
            &self.previous_sha256,
        ))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct BridgeState {
    pub(super) policy: String,
    pub(super) disposition: Disposition,
    pub(super) seals: Vec<TrainSeal>,
    pub(super) parts: Vec<QualityPart>,
}
impl BridgeState {
    pub(super) fn new() -> Self {
        Self {
            policy: BRIDGE_POLICY.into(),
            disposition: Disposition::Ready,
            seals: vec![],
            parts: vec![],
        }
    }
    pub(super) fn validate(
        &self,
        schedule: &ManagedScheduleContract,
        run: &str,
        rows: &[CaptureCohort],
        evidence: &[WindowEvidence],
        ledger: &Ledger,
        episode: usize,
        mapping: Option<&ObservedSegmentMap>,
        now: u64,
    ) -> Result<(), String> {
        let train = schedule
            .episodes()
            .iter()
            .take_while(|e| e.role == DataRole::Train)
            .count();
        if self.policy != BRIDGE_POLICY
            || self.seals.len() > train
            || episode != self.seals.len()
            || episode > train
            || self.parts.len() > 128
            || self.seals.iter().map(|s| s.parts.len()).sum::<usize>() + self.parts.len()
                > 128 + train
        {
            return Err("temporal_bridge_phase_order".into());
        }
        let mut count = 0;
        let mut previous = None;
        let mut last_confirm = 0;
        for (i, s) in self.seals.iter().enumerate() {
            count += schedule.episodes()[i].slots.len();
            let bound = schedule.bind_episode_start(run, i, 0, 0, None)?;
            if s.episode != i
                || s.trial_id != bound.trial_id()
                || s.context_sha256 != schedule.context().digest()
                || s.schedule_sha256 != schedule.sha256()
                || count > rows.len()
                || s.confirmed_at_ns < last_confirm
                || s.confirmed_at_ns > now
                || s.previous_sha256 != previous
                || s.sha256 != s.digest()?
                || s.quality_sha256 != hash(&s.parts)?
                || s.row_prefix_sha256
                    != hash(&(&rows[..count], &evidence[..count], &ledger.spans[..count]))?
            {
                return Err("temporal_train_seal_binding".into());
            }
            validate_parts(
                &s.parts,
                schedule,
                run,
                i,
                Some(schedule.episodes()[i].target_active_recording_ms * 1_000_000),
                s.confirmed_at_ns,
            )?;
            validate_part_lineage(&s.parts, i, ledger, rows, None)?;
            previous = Some(s.sha256.clone());
            last_confirm = s.confirmed_at_ns;
        }
        if episode == train {
            if self.disposition != Disposition::Complete
                || !self.parts.is_empty()
                || mapping.is_some()
                || rows.len() != schedule.counts().train_windows
            {
                return Err("temporal_bridge_complete_state".into());
            }
        } else {
            if rows.len() < count || rows.len() > count + schedule.episodes()[episode].slots.len() {
                return Err("temporal_bridge_row_phase".into());
            }
            validate_parts(&self.parts, schedule, run, episode, None, now)?;
            validate_part_lineage(&self.parts, episode, ledger, rows, mapping)?;
            if let Some(m) = mapping {
                if self.parts.last().map(|p| &p.mapping) != Some(m) {
                    return Err("temporal_quality_current_mapping".into());
                }
            } else if !self.parts.is_empty() || rows.len() != count {
                return Err("temporal_bridge_mapping_missing".into());
            }
            let accepted = ledger
                .spans
                .last()
                .filter(|s| s.episode == episode)
                .map_or(0, |s| s.observed_end_ns);
            if self.parts.last().map_or(0, |p| p.observed_end_ns) != accepted {
                return Err("temporal_quality_durable_frontier".into());
            }
            if self.disposition == Disposition::Ready
                && (mapping.is_some() || !self.parts.is_empty() || rows.len() != count)
            {
                return Err("temporal_bridge_ready_with_active_data".into());
            }
            if matches!(
                self.disposition,
                Disposition::Observing | Disposition::NeedsConfirmation | Disposition::Paused
            ) && mapping.is_none()
            {
                return Err("temporal_bridge_phase_without_mapping".into());
            }
            if self.disposition == Disposition::Complete {
                return Err("temporal_bridge_early_complete".into());
            }
            if self.disposition == Disposition::NeedsConfirmation
                && accepted != schedule.episodes()[episode].target_active_recording_ms * 1_000_000
            {
                return Err("temporal_bridge_confirmation_before_end".into());
            }
        }
        Ok(())
    }
}
fn validate_part_lineage(
    parts: &[QualityPart],
    episode: usize,
    ledger: &Ledger,
    rows: &[CaptureCohort],
    current: Option<&ObservedSegmentMap>,
) -> Result<(), String> {
    for part in parts {
        let spans = ledger
            .spans
            .iter()
            .enumerate()
            .filter(|(_, s)| s.episode == episode && s.mapping == part.mapping)
            .collect::<Vec<_>>();
        let frontier = spans
            .last()
            .map_or(part.mapping.observed_start_ns, |(_, s)| s.observed_end_ns);
        let known = !spans.is_empty()
            || current == Some(&part.mapping)
            || ledger.gaps.iter().any(|g| {
                g.episode == episode
                    && (g.new_mapping == part.mapping
                        || g.genesis
                            .as_ref()
                            .is_some_and(|v| v.previous_mapping == part.mapping))
            });
        if !known || part.observed_end_ns != frontier {
            return Err("temporal_quality_lineage_changed".into());
        }
        for source in &part.sources {
            let mut retained = 0u64;
            for (index, _) in &spans {
                let row = rows.get(*index).ok_or("temporal_quality_row_missing")?;
                let view = row
                    .views
                    .iter()
                    .find(|v| v.source_id == source.source_id)
                    .ok_or("temporal_quality_row_source")?;
                retained = retained
                    .checked_add(view.samples.len() as u64)
                    .ok_or("temporal_quality_count")?;
                if source.first_ns > view.first_time_ns
                    || source.last_ns < view.last_time_ns
                    || view
                        .sequence_ids
                        .first()
                        .is_none_or(|v| *v < source.first_sequence)
                    || view
                        .sequence_ids
                        .last()
                        .is_none_or(|v| *v > source.last_sequence)
                {
                    return Err("temporal_quality_retained_identity".into());
                }
            }
            if source.packets < retained {
                return Err("temporal_quality_retained_count".into());
            }
        }
    }
    Ok(())
}
fn validate_parts(
    parts: &[QualityPart],
    schedule: &ManagedScheduleContract,
    run: &str,
    episode: usize,
    complete: Option<u64>,
    now: u64,
) -> Result<(), String> {
    let native = schedule.native_window_policy();
    let mut frontier = 0;
    let mut ordinal = None;
    let mut receiver_end = 0;
    for p in parts {
        let bound =
            schedule.bind_episode_start(run, episode, 0, p.mapping.segment_ordinal, None)?;
        if p.mapping.session_id != bound.session_id()
            || (ordinal.is_none() && p.mapping.segment_ordinal != 0)
            || p.mapping.observed_start_ns != frontier
            || ordinal.is_some_and(|n| p.mapping.segment_ordinal != n + 1)
            || p.mapping.receiver_start_ns < receiver_end
        {
            return Err("temporal_quality_segment_order".into());
        }
        p.validate(&native, &bound, now)?;
        frontier = p.observed_end_ns;
        ordinal = Some(p.mapping.segment_ordinal);
        receiver_end = p
            .mapping
            .receiver_start_ns
            .checked_add(p.observed_end_ns - p.mapping.observed_start_ns)
            .ok_or("temporal_quality_time")?;
    }
    if complete.is_some_and(|v| v != frontier) || complete.is_some() && parts.is_empty() {
        return Err("temporal_quality_period_incomplete".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TemporalCaptureStage {
    Preparing,
    Ready,
    Observing,
    NeedsConfirmation,
    Complete,
    Frozen,
    Paused,
    Blocked,
    Failed,
}
impl TemporalCaptureStage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Ready => "ready",
            Self::Observing => "observing",
            Self::NeedsConfirmation => "needs_confirmation",
            Self::Complete => "complete",
            Self::Frozen => "frozen",
            Self::Paused => "paused",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
        }
    }
}
pub(crate) struct TemporalTrainCapture {
    workflow: TemporalWorkflow,
    // The exact immutable contract decoded from this workflow's validated
    // binding. Packet handling must not reparse its full JSON on every tick.
    schedule: ManagedScheduleContract,
    failure: Option<String>,
    durable_seals: usize,
}
impl TemporalTrainCapture {
    #[cfg(test)]
    pub(crate) fn test_full_run_floors(&self)->(Vec<(u8,u64,u64)>,Option<u64>){
        (self.workflow.source_high.clone(),self.workflow.full_run.as_ref().and_then(|m|m.local_start_not_before_ns))
    }
    pub(crate) fn is_full_run(&self)->bool{self.workflow.full_run.is_some()}
    pub(crate) fn verified_frozen_model(&self)->Option<std::sync::Arc<TemporalModel>>{if self.failure.is_some(){return None;}self.workflow.full_run.as_ref().and_then(|m|m.verified_model())}
    pub(crate) fn request_frozen(&mut self,accepted:crate::room_learner::lifecycle::AcceptedTemporalFit,witness:crate::room_learner::lifecycle::FrozenOwnerWitness,store:std::sync::Arc<crate::room_learner::artifact::ArtifactStore>,now:u64)->Result<(),String>{
        self.ensure_live()?;
        if self.stage()!=TemporalCaptureStage::Complete{return Err("temporal_full_run_train_complete_required".into());}
        self.workflow.request_frozen(accepted,witness,store,now)
    }
    pub(crate) fn prepare_full_run_async(
        schedule:ManagedScheduleContract,run:String,config:FitConfig,now:u64,
        store:std::sync::Arc<RecoveryStore>,witness:crate::room_learner::lifecycle::FullRunGenesisWitness,
    )->Result<(Self,Option<String>),String>{
        // Fail unsupported full-run profiles before creating a writer/genesis.
        full_run::ContinuationPolicy::build(&ManagedBinding::build(&schedule,config.clone())?)?;
        let owned_store=store.clone();
        Self::prepare_with(schedule,run,config,now,&store,move|w,store|{
            let binding=&w.session.managed.as_ref().ok_or("managed_session_required")?.binding;
            w.full_run=Some(full_run::FullRunState::fresh(binding,&w.session.run_id,witness)?);
            let writer=std::sync::Arc::new(durability::Writer::new(owned_store,now)?);
            w.async_state=Some(durability::AsyncState::new(writer));
            w.persist_initial(store)
        })
    }
    pub(crate) fn prepare_fit_input(&self, now: u64) -> Result<PendingTemporalFitInput, String> {
        self.ensure_live()?;
        if self.stage() != TemporalCaptureStage::Complete {
            return Err("temporal_fit_confirmed_durable_train_required".into());
        }
        fit_admission::PendingTemporalFitInput::capture(&self.workflow, &self.schedule, now)
    }
    pub(crate) fn fit_capture_identity(&self, now: u64) -> Result<TemporalFitCaptureIdentity, String> {
        self.ensure_live()?;
        if self.stage() != TemporalCaptureStage::Complete {
            return Err("temporal_fit_confirmed_durable_train_required".into());
        }
        fit_admission::current_identity(&self.workflow, &self.schedule, now)
    }
    pub(crate) fn prepare(
        schedule: ManagedScheduleContract,
        run: String,
        config: FitConfig,
        now: u64,
        store: &RecoveryStore,
    ) -> Result<Self, String> {
        let (capture, error) = Self::prepare_retained(schedule, run, config, now, store)?;
        if let Some(error) = error {
            Err(error)
        } else {
            Ok(capture)
        }
    }
    pub(crate) fn prepare_async(
        schedule: ManagedScheduleContract, run:String, config:FitConfig, now:u64,
        store:std::sync::Arc<RecoveryStore>,
    ) -> Result<(Self,Option<String>),String> {
        let owned_store=store.clone();
        Self::prepare_with(schedule,run,config,now,&store,move |w,store|{
            let writer=std::sync::Arc::new(durability::Writer::new(owned_store,now)?);
            w.async_state=Some(durability::AsyncState::new(writer));
            w.persist_initial(store)
        })
    }
    pub(crate) fn async_start(&mut self,now:u64)->Result<bool,String>{
        self.ensure_live()?;
        if self.workflow.async_state.is_none(){return Ok(false);}
        self.workflow.request_async_start(now)?;Ok(true)
    }
    pub(crate) fn poll_async(&mut self,now:u64,retired:&mut DeferredReclamation)->Result<Option<(bool,u64)>,String>{
        if self.failure.is_some(){return Err("temporal_capture_failed".into());}
        let start=match self.workflow.poll_async(now,retired){
            Ok(start)=>start,
            Err(error)=>{
                if matches!(error.as_str(),"temporal_writer_ack_clock_or_expiry"|"temporal_writer_ack_identity_changed"|"temporal_writer_ack_row_lineage_changed"|"temporal_writer_ack_binding_changed") {self.invalidate(&error);}
                return Err(error);
            }
        };
        if let Some(state)=&self.workflow.async_state{self.durable_seals=state.accepted_seals;}
        Ok(start)
    }
    pub(crate) fn enable_async_restore(&mut self,store:std::sync::Arc<RecoveryStore>,now:u64)->Result<(),String>{
        if self.workflow.async_state.is_some(){return Err("temporal_writer_already_present".into());}
        let mut state=durability::AsyncState::new(std::sync::Arc::new(durability::Writer::new(store,now)?));
        state.seed_restored(&self.workflow)?;
        self.workflow.async_state=Some(state);Ok(())
    }
    pub(crate) fn writer(&self)->Option<std::sync::Arc<durability::Writer>>{
        self.workflow.async_state.as_ref().map(|a|a.writer.clone())
    }
    pub(crate) fn prepare_retained(
        schedule: ManagedScheduleContract,
        run: String,
        config: FitConfig,
        now: u64,
        store: &RecoveryStore,
    ) -> Result<(Self, Option<String>), String> {
        Self::prepare_with(schedule, run, config, now, store, |w, store| {
            w.persist_initial(store)
        })
    }
    fn prepare_with(
        schedule: ManagedScheduleContract,
        run: String,
        config: FitConfig,
        now: u64,
        store: &RecoveryStore,
        initial: impl FnOnce(&mut TemporalWorkflow, &RecoveryStore) -> Result<(), String>,
    ) -> Result<(Self, Option<String>), String> {
        let mut workflow = TemporalWorkflow::prepare(schedule, run, config, now)?;
        let schedule = workflow.schedule()?;
        workflow.bridge = Some(BridgeState::new());
        let error = initial(&mut workflow, store).err();
        if error.is_some() {
            workflow.stage = Stage::BlockedDurability;
        }
        Ok((
            Self {
                workflow,
                schedule,
                failure: None,
                durable_seals: 0,
            },
            error,
        ))
    }
    #[cfg(test)]
    pub(crate) fn synthetic_prepare_lost_initial_ack(
        schedule: ManagedScheduleContract,
        run: String,
        config: FitConfig,
        now: u64,
        store: &RecoveryStore,
    ) -> Result<(Self, Option<String>), String> {
        Self::prepare_with(schedule, run, config, now, store, |w, store| {
            w.persist_with(store, false, |s, i, p, n| {
                s.publish_temporal(i, Some(p), n)?;
                Err("synthetic-lost-initial-ack-after-real-publish".into())
            })
        })
    }
    pub(crate) fn context(&self) -> &CaptureContext {
        self.workflow.context()
    }
    pub(crate) fn check_time(&mut self, now: u64) -> Result<(), String> {
        if self.workflow.async_state.is_none(){self.ensure_live()?;}else if self.failure.is_some(){return Err("temporal_capture_failed".into());}
        if now < self.workflow.last_now_ns
            || now / 1_000_000 >= self.workflow.descriptor.expires_at_ms
        {
            self.invalidate("temporal_receiver_clock_or_expiry");
            return Err("temporal_receiver_clock_or_expiry".into());
        }
        Ok(())
    }
    pub(crate) fn prime_fence(&mut self, p: &CanonicalPacket, now: u64) -> Result<(), String> {
        if self.workflow.async_state.is_none(){self.ensure_live()?;}else if self.failure.is_some(){return Err("temporal_capture_failed".into());}
        // A full-run owner keeps listening between Train completion and the
        // future HeldOut stage. This admits only the existing replay/clock
        // fence update below: never another row, seal, model or role credit.
        let full_run_continuation_fence = self.is_full_run()
            && matches!(self.stage(), TemporalCaptureStage::Complete | TemporalCaptureStage::Frozen);
        if !matches!(
            self.stage(),
            TemporalCaptureStage::Ready | TemporalCaptureStage::Paused | TemporalCaptureStage::Preparing | TemporalCaptureStage::Blocked
        ) && !(self.workflow.async_state.is_some() && self.logical_phase_complete())
            && !full_run_continuation_fence {
            return Err("temporal_priming_state".into());
        }
        let result: Result<(), String> = (|| {
            let w = &mut self.workflow;
            w.check_clock(now)?;
            w.session
                .managed
                .as_ref()
                .ok_or("managed_session_required")?
                .binding
                .native
                .validate_packet_source(p)?;
            let t = p.native_frame.timestamp_ns;
            if p.provenance != CaptureProvenance::LiveReceiverObserved
                || p.native_frame.provenance.class
                    != ruview_unified::frame::ProvenanceClass::Measured
                || t > now
                || w.source_high.iter().any(|(id, seq, last)| {
                    *id == p.source_id && (u64::from(p.sequence) <= *seq || t <= *last)
                })
            {
                return Err("temporal_priming_source_or_fence".into());
            }
            w.source_high.retain(|(id, _, _)| *id != p.source_id);
            w.source_high.push((p.source_id, u64::from(p.sequence), t));
            w.source_high.sort();
            Ok(())
        })();
        if let Err(e) = &result {
            self.invalidate(e);
        }
        result
    }

    pub(crate) fn stage(&self) -> TemporalCaptureStage {
        if self.failure.is_some() {
            return TemporalCaptureStage::Failed;
        }
        let w = &self.workflow;
        if w.async_state.as_ref().is_some_and(|a| a.pending.as_ref().is_some_and(|p|p.start_ticket || !matches!(w.stage,Stage::Observing|Stage::BlockedDurability)) || w.durable.is_none()) {
            return TemporalCaptureStage::Preparing;
        }
        if w.stage == Stage::BlockedDurability {
            return TemporalCaptureStage::Blocked;
        }
        if w.full_run.as_ref().is_some_and(|m|m.is_frozen()){return TemporalCaptureStage::Frozen;}
        if w.bridge
            .as_ref()
            .is_some_and(|b| b.disposition == Disposition::Complete)
        {
            return TemporalCaptureStage::Complete;
        }
        if w.stage == Stage::PausedNeedsReconfirmation {
            return TemporalCaptureStage::Paused;
        }
        if self.phase_ready() {
            return TemporalCaptureStage::NeedsConfirmation;
        }
        if w.stage == Stage::Observing {
            TemporalCaptureStage::Observing
        } else {
            TemporalCaptureStage::Ready
        }
    }
    pub(crate) fn phase_identity(&self) -> Option<(String, String)> {
        let w = &self.workflow;
        let schedule = &self.schedule;
        let e = schedule.episodes().get(w.episode)?;
        if e.role != DataRole::Train {
            return None;
        }
        let bound = schedule
            .bind_episode_start(
                &w.session.run_id,
                w.episode,
                0,
                w.mapping.as_ref().map_or(0, |m| m.segment_ordinal),
                None,
            )
            .ok()?;
        Some((bound.trial_id().into(), bound.session_id().into()))
    }
    fn logical_phase_complete(&self)->bool {
        let w=&self.workflow;
        self.schedule.episodes().get(w.episode).is_some_and(|e|e.role==DataRole::Train
            && w.pending_window.is_none()
            && w.session.rows.len()==self.schedule.episodes().iter().take(w.episode+1).map(|e|e.slots.len()).sum::<usize>())
    }
    fn phase_ready(&self) -> bool {
        let w = &self.workflow;
        let s = &self.schedule;
        let Some(e) = s.episodes().get(w.episode) else {
            return false;
        };
        e.role == DataRole::Train
            && w.accepted() == e.target_active_recording_ms * 1_000_000
            && w.pending_window.is_none()
            && w.session.rows.len()
                == s.episodes()
                    .iter()
                    .take(w.episode + 1)
                    .map(|e| e.slots.len())
                    .sum::<usize>()
    }
    pub(crate) fn start_segment(
        &mut self,
        epoch: &str,
        now: u64,
        store: &RecoveryStore,
    ) -> Result<(), String> {
        self.ensure_live()?;
        if self.phase_ready() {
            return Err("temporal_confirmation_required".into());
        }
        let w = &mut self.workflow;
        let schedule = w.schedule()?;
        if schedule
            .episodes()
            .get(w.episode)
            .is_none_or(|e| e.role != DataRole::Train)
        {
            return Err("temporal_train_complete".into());
        }
        let permit = ConditionsPermit {
            context: w.context().digest(),
            run: w.session.run_id.clone(),
            durable_sha256: w.durable.as_ref().map(|b| bytes_hash(b)),
        };
        w.start_segment(&permit, epoch, now, now)?;
        let mapping = w.mapping.clone().ok_or("temporal_segment_missing")?;
        let b = w.bridge.as_mut().ok_or("temporal_bridge_required")?;
        b.disposition = Disposition::Observing;
        b.parts.push(QualityPart {
            observed_end_ns: mapping.observed_start_ns,
            mapping,
            sources: vec![],
        });
        w.persist_segment(store)?;
        w.begin_slot()?;
        Ok(())
    }
    pub(crate) fn ingest(
        &mut self,
        p: &CanonicalPacket,
        now: u64,
        store: &RecoveryStore,
    ) -> Result<(), String> {
        self.ensure_live()?;
        if self.phase_ready() || (self.workflow.async_state.is_some() && self.logical_phase_complete()) {
            if self.workflow.async_state.is_some(){return self.prime_fence(p,now);}
            return Ok(());
        }
        let result = (|| {
            let w = &mut self.workflow;
            let native = &w
                .session
                .managed
                .as_ref()
                .ok_or("managed_session_required")?
                .binding
                .native;
            native.validate_packet_source(p)?;
            if p.provenance != CaptureProvenance::LiveReceiverObserved
                || p.native_frame.provenance.class
                    != ruview_unified::frame::ProvenanceClass::Measured
            {
                return Err("temporal_measured_receiver_required".into());
            }
            w.observe(p, now)?;
            w.bridge
                .as_mut()
                .ok_or("temporal_bridge_required")?
                .parts
                .last_mut()
                .ok_or("temporal_quality_segment_missing")?
                .observe(p)?;
            self.commit_if_ready(store)
        })();
        if let Err(e) = &result {
            if self.workflow.stage != Stage::BlockedDurability {
                self.invalidate(e);
            }
        }
        result
    }
    pub(crate) fn tick(&mut self, now: u64, store: &RecoveryStore) -> Result<(), String> {
        self.ensure_live()?;
        if self.phase_ready() {
            return self.workflow.check_clock(now);
        }
        let result = self
            .workflow
            .observe_tick(now)
            .and_then(|_| self.commit_if_ready(store));
        if let Err(e) = &result {
            if self.workflow.stage != Stage::BlockedDurability {
                self.invalidate(e);
            }
        }
        result
    }
    fn commit_if_ready(&mut self, store: &RecoveryStore) -> Result<(), String> {
        let w = &mut self.workflow;
        let Some(target) = w.pending_window.as_ref() else {
            return Ok(());
        };
        if w.session.active
            || w.source_high.len() != w.context().sources.len()
            || w.source_high.iter().any(|(_, _, t)| *t < target.end_ns())
        {
            return Ok(());
        }
        let observed = target.identity().slot_index;
        let schedule = w.schedule()?;
        let deadline =
            schedule.episodes()[w.episode].slots[observed].deadline_offset_ms * 1_000_000;
        if w.async_state.as_ref().is_some_and(|a|a.pending.is_some()) {
            let next=schedule.episodes()[w.episode].slots.get(observed+1);
            if let Some(next)=next {
                let mapping=w.mapping.as_ref().ok_or("temporal_segment_missing")?;
                let deadline=mapping.receiver_start_ns.checked_add(next.target_offset_ms*1_000_000-mapping.observed_start_ns).ok_or("temporal_time_overflow")?;
                if w.last_now_ns>=deadline {
                    let state=w.async_state.as_mut().unwrap();state.busy_pause=true;state.error=Some("temporal_storage_busy_pause".into());
                    w.stage=Stage::BlockedDurability;return Err("temporal_storage_busy_pause".into());
                }
            }
            return Ok(());
        }
        let b = w.bridge.as_mut().ok_or("temporal_bridge_required")?;
        b.parts
            .last_mut()
            .ok_or("temporal_quality_segment_missing")?
            .observed_end_ns = deadline;
        if deadline == schedule.episodes()[w.episode].target_active_recording_ms * 1_000_000 {
            b.disposition = Disposition::NeedsConfirmation;
        }
        w.commit_slot(store, w.last_now_ns)?;
        if !self.logical_phase_complete() {
            self.workflow.begin_slot()?;
        }
        Ok(())
    }
    pub(crate) fn confirmation_challenge(&self) -> Result<String, String> {
        self.ensure_live()?;
        if !self.phase_ready() || self.workflow.async_state.as_ref().is_some_and(|a|a.pending.is_some()) {
            return Err("temporal_phase_not_complete".into());
        }
        let w = &self.workflow;
        let b = w.bridge.as_ref().ok_or("temporal_bridge_required")?;
        hash(&(
            BRIDGE_POLICY,
            &w.descriptor.id,
            &w.session.run_id,
            w.episode,
            w.durable_digest(),
            &b.parts,
            &b.seals,
        ))
    }
    pub(crate) fn confirm(
        &mut self,
        challenge: &str,
        store: &RecoveryStore,
        now: u64,
    ) -> Result<(), String> {
        self.ensure_live()?;
        if challenge != self.confirmation_challenge()? {
            return Err("temporal_stale_conditions_challenge".into());
        }
        let w = &mut self.workflow;
        w.check_clock(now)?;
        let schedule = w.schedule()?;
        let context_sha256 = w.context().digest();
        let row_prefix_sha256 = match &w.async_state {
            Some(state)=>state.row_prefix_sha256.clone().ok_or("temporal_row_prefix_unconfirmed")?,
            None=>hash(&(&w.session.rows, &w.session.evidence, &w.ledger.spans))?,
        };
        let b = w.bridge.as_mut().ok_or("temporal_bridge_required")?;
        validate_parts(
            &b.parts,
            &schedule,
            &w.session.run_id,
            w.episode,
            Some(schedule.episodes()[w.episode].target_active_recording_ms * 1_000_000),
            now,
        )?;
        let bound = schedule.bind_episode_start(&w.session.run_id, w.episode, 0, 0, None)?;
        let mut seal = TrainSeal {
            episode: w.episode,
            trial_id: bound.trial_id().into(),
            context_sha256,
            schedule_sha256: schedule.sha256().into(),
            row_prefix_sha256,
            quality_sha256: hash(&b.parts)?,
            parts: b.parts.clone(),
            confirmed_at_ns: now,
            previous_sha256: b.seals.last().map(|s| s.sha256.clone()),
            sha256: String::new(),
        };
        seal.sha256 = seal.digest()?;
        b.seals.push(seal);
        b.parts.clear();
        w.episode += 1;
        w.mapping = None;
        w.custody = SegmentCustody::default();
        w.live_observed_ns = 0;
        b.disposition = if w.episode == schedule.counts().train_episodes {
            Disposition::Complete
        } else {
            Disposition::Ready
        };
        w.stage = Stage::DurableReady;
        w.persist_with(store, false, |s, i, p, n| s.publish_temporal(i, Some(p), n))?;
        if w.async_state.is_none(){self.durable_seals = w.bridge.as_ref().unwrap().seals.len();}
        Ok(())
    }
    pub(crate) fn pause(&mut self, now: u64, store: &RecoveryStore) -> Result<(), String> {
        self.ensure_live()?;
        if self.workflow.async_state.is_some(){return self.workflow.request_async_pause(now);}
        self.workflow.check_clock(now)?;
        self.workflow.pause()?;
        self.workflow
            .bridge
            .as_mut()
            .ok_or("temporal_bridge_required")?
            .disposition = Disposition::Paused;
        self.workflow
            .persist_with(store, false, |s, i, p, n| s.publish_temporal(i, Some(p), n))
    }
    pub(crate) fn cold_restore(
        store: &RecoveryStore,
        context: &CaptureContext,
        now: u64,
        epoch: &str,
        anchor: u64,
    ) -> Result<Self, String> {
        let w = TemporalWorkflow::cold_restore(store, context, now, epoch, anchor)?;
        if w.full_run.is_some(){return Err("temporal_full_run_restore_requires_exact_model_consumer".into());}
        let b = w
            .bridge
            .as_ref()
            .ok_or("temporal_bridge_checkpoint_required")?;
        let n = b.seals.len();
        let schedule = w.schedule()?;
        Ok(Self {
            workflow: w,
            schedule,
            failure: None,
            durable_seals: n,
        })
    }
    pub(crate) fn cold_restore_full_run(
        store:&RecoveryStore,artifact:std::sync::Arc<crate::room_learner::artifact::ArtifactStore>,context:&CaptureContext,
        now:u64,epoch:&str,anchor:u64,witness:crate::room_learner::lifecycle::FullRunRecoveryWitness,
    )->Result<Self,String>{
        let w=full_run::recover_current(store,artifact,context,now,epoch,anchor,witness)?;
        let n=w.bridge.as_ref().ok_or("temporal_bridge_checkpoint_required")?.seals.len();let schedule=w.schedule()?;
        Ok(Self{workflow:w,schedule,failure:None,durable_seals:n})
    }
    pub(crate) fn repair_pending(&mut self, store: &RecoveryStore, now: u64) -> Result<(), String> {
        if self.workflow.async_state.is_some(){return self.workflow.request_async_repair(now);}
        self.workflow.repair_pending(store, now)?;
        if self.workflow.schedule()?.sha256() != self.schedule.sha256() {
            self.invalidate("temporal_repaired_schedule_changed");
            return Err("temporal_repaired_schedule_changed".into());
        }
        self.durable_seals = self
            .workflow
            .bridge
            .as_ref()
            .ok_or("temporal_bridge_required")?
            .seals
            .len();
        Ok(())
    }
    pub(crate) fn invalidate(&mut self, reason: &str) {
        if let Some(state)=&self.workflow.async_state{state.writer.cancel();}
        self.failure = Some(
            reason
                .chars()
                .filter(|c| !c.is_control())
                .take(256)
                .collect(),
        );
    }
    fn ensure_live(&self) -> Result<(), String> {
        if self.failure.is_some() {
            Err("temporal_capture_failed".into())
        } else if self.workflow.stage == Stage::BlockedDurability {
            Err("temporal_capture_durability_blocked".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn snapshot(&self) -> Value {
        let w = &self.workflow;
        let e = self.schedule.episodes().get(w.episode);
        json!({"schema":"ruview.room-learner.private-temporal-capture.v1","stage":self.stage().as_str(),"episode_index":w.episode,"label":e.filter(|e|e.role==DataRole::Train).map(|e|e.condition),"role":"Train","sealed_train_episodes":self.durable_seals,"retained_rows":w.session.rows.len(),"durable_windows":w.durable_windows,"rows_sha256":w.async_state.as_ref().map(|a|if w.session.rows.len()==w.durable_windows{a.rows_sha256.clone()}else{None}).unwrap_or_else(||hash(&w.session.rows).ok()),"durability_pending":w.async_state.as_ref().is_some_and(|a|a.pending.is_some()),"durability_error":w.async_state.as_ref().and_then(|a|a.error.clone()),"observed_ns":w.live_observed_ns.to_string(),"durable_observed_ns":w.accepted().to_string(),"target_ms":e.filter(|e|e.role==DataRole::Train).map(|e|e.target_active_recording_ms),"descriptor":w.descriptor,"failure":self.failure,"fit_admission_available":false,"quality_is_engineering_only":true,"independence_verified":false,"source_clock_measured":false})
    }
}

impl Drop for TemporalTrainCapture {
    fn drop(&mut self) {
        // Runtime drains installed captures before dropping its inner state.
        // Constructor failures/obsolete unpublished cores must also retain the
        // owner store until their one writer has actually stopped.
        if let Some(state)=&self.workflow.async_state {let _=state.writer.drain();}
    }
}
