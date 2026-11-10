//! Private Runtime capture facade. Whole-period receiver quality is engineering
//! admission only: never source-clock precision, teacher truth or qualification.
//! Seals alone grant no fit authority; the private owner must separately consume
//! the complete durable producer prefix into its opaque fit-only admission.
use super::*;
use crate::room_learner::temporal_acquisition::WindowInputPolicy;
use serde_json::{json, Value};

pub(super) const BRIDGE_POLICY: &str = "native-receiver-whole-train-period-v1;all-selected-packets;existing-cadence-gap-arrival-skew;confirmed-durable-seals;no-fit-authority";
// Explicit NEW Train genesis only. The generic constructor and old literal
// remain historical329; restore never upgrades a stored generation.
pub(super) const FRESH_TRAIN_343_POLICY:&str="native-receiver-whole-train-period-v2;all-selected-packets;existing-cadence-gap-arrival-skew;confirmed-durable-seals;train-budget343;no-fit-authority";
pub(super) const OPERATOR_EIGHT_POLICY:&str="native-receiver-whole-operator-period-v1;reserved-validation-not-training;confirmed-durable-seals;no-fit-authority";
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum TrainBudgetProfile { Historical329, FreshTrain343, OperatorEightStep }
impl TrainBudgetProfile {
    pub(super) fn from_stored(policy:&str)->Result<Self,String>{match policy{
        BRIDGE_POLICY=>Ok(Self::Historical329),FRESH_TRAIN_343_POLICY=>Ok(Self::FreshTrain343),OPERATOR_EIGHT_POLICY=>Ok(Self::OperatorEightStep),
        _=>Err("temporal_train_profile_unknown".into()),
    }}
    pub(super) fn policy(self)->&'static str{match self{Self::Historical329=>BRIDGE_POLICY,Self::FreshTrain343=>FRESH_TRAIN_343_POLICY,Self::OperatorEightStep=>OPERATOR_EIGHT_POLICY}}
    pub(super) fn transition_limit(self,schedule:&ManagedScheduleContract)->Result<usize,String>{
        let episodes=schedule.counts().train_episodes;
        let old=schedule.counts().train_windows.checked_add(2*128).and_then(|n|n.checked_add(episodes)).and_then(|n|n.checked_add(2)).ok_or("temporal_transition_capacity")?;
        match self{Self::Historical329=>Ok(old),Self::FreshTrain343=>old.checked_add(2*episodes).ok_or("temporal_transition_capacity".into()),Self::OperatorEightStep=>Ok(schedule.counts().total_windows+2*(128+schedule.episodes().len())+schedule.episodes().len()+4)}
    }
}

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
    fn digest(&self,profile:TrainBudgetProfile) -> Result<String, String> {
        hash(&(
            profile.policy(),
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
    pub(super) fn operator_eight_step()->Self{Self{policy:OPERATOR_EIGHT_POLICY.into(),..Self::new()}}
    pub(super) fn fresh_train_343()->Self{
        Self{policy:FRESH_TRAIN_343_POLICY.into(),..Self::new()}
    }
    pub(super) fn profile(&self)->Result<TrainBudgetProfile,String>{TrainBudgetProfile::from_stored(&self.policy)}
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
        self.validate_with_prefixes(schedule,run,rows,evidence,ledger,episode,mapping,now,None)
    }
    pub(super) fn validate_full_run_with_prefixes(
        &self,schedule:&ManagedScheduleContract,run:&str,rows:&[CaptureCohort],evidence:&[WindowEvidence],ledger:&Ledger,episode:usize,mapping:Option<&ObservedSegmentMap>,now:u64,digests:&prefix_digests::CanonicalPrefixDigests,
    )->Result<(),String>{
        if self.policy!=if schedule.is_operator_eight_step(){OPERATOR_EIGHT_POLICY}else{BRIDGE_POLICY}{return Err("temporal_full_run_train_profile_changed".into());}
        self.validate_with_prefixes(schedule,run,rows,evidence,ledger,episode,mapping,now,Some(digests))
    }
    fn validate_with_prefixes(
        &self,schedule:&ManagedScheduleContract,run:&str,rows:&[CaptureCohort],evidence:&[WindowEvidence],ledger:&Ledger,episode:usize,mapping:Option<&ObservedSegmentMap>,now:u64,digests:Option<&prefix_digests::CanonicalPrefixDigests>,
    )->Result<(),String>{
        let train = if schedule.is_operator_eight_step() { schedule.episodes().len() } else {
            schedule.episodes().iter().take_while(|e| e.role == DataRole::Train).count()
        };
        let profile=self.profile()?;
        if self.seals.len() > train
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
                || s.sha256 != s.digest(profile)?
                || s.quality_sha256 != hash(&s.parts)?
                || s.row_prefix_sha256 != match digests{
                    Some(digests)=>digests.at(count)?.to_owned(),
                    None=>hash(&(&rows[..count], &evidence[..count], &ledger.spans[..count]))?,
                }
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
                || rows.len() != if schedule.is_operator_eight_step(){schedule.counts().total_windows}else{schedule.counts().train_windows}
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
pub(super) fn validate_part_lineage(
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
    validate_parts_with_frozen(parts,schedule,run,episode,complete,now,None)
}
pub(super) fn validate_parts_with_frozen(
    parts:&[QualityPart],schedule:&ManagedScheduleContract,run:&str,episode:usize,
    complete:Option<u64>,now:u64,
    frozen:Option<&crate::room_learner::managed_schedule_contract::FrozenModelIdentity>,
)->Result<(),String>{
    let native = schedule.native_window_policy();
    let mut frontier = 0;
    let mut ordinal = None;
    let mut receiver_end = 0;
    for p in parts {
        let bound =
            schedule.bind_episode_start(run, episode, 0, p.mapping.segment_ordinal, frozen)?;
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
    FullComplete,
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
            Self::FullComplete => "full_complete",
            Self::Paused => "paused",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
        }
    }
}
pub(crate) struct TemporalTrainCapture {
    workflow: TemporalWorkflow,
    public_generation_id: String,
    // The exact immutable contract decoded from this workflow's validated
    // binding. Packet handling must not reparse its full JSON on every tick.
    schedule: ManagedScheduleContract,
    failure: Option<String>,
    durable_seals: usize,
}
impl TemporalTrainCapture {
    #[cfg(test)]
    pub(crate) fn c4b_test_abort_contract(&self)->Result<(),String>{self.workflow.require_abortable_contract()}
    #[cfg(test)]
    pub(crate) fn c4b_test_custody(&self)->(usize,usize,bool,(usize,usize,usize,bool,bool)){(self.workflow.session.rows.len(),self.workflow.ledger.spans.len(),self.workflow.session.active,self.workflow.session.assembler.c4b_test_fence_counts())}
    #[cfg(test)]
    pub(crate) fn c4b_test_replay_budget(&self)->Result<full_run::ReplayFenceBudget,String>{self.workflow.full_run.as_ref().ok_or("temporal_full_run_required")?.policy.replay_fence_budget(&self.workflow.session.managed.as_ref().unwrap().binding)}
    #[cfg(test)]
    pub(crate) fn c4b_test_target(&self)->Option<(u64,u64)>{self.workflow.pending_window.as_ref().map(|w|(w.target_ns(),w.end_ns()))}
    #[cfg(test)]
    pub(crate) fn test_full_run_floors(&self)->(Vec<(u8,u64,u64)>,Option<u64>){
        (self.workflow.source_high.clone(),self.workflow.full_run.as_ref().and_then(|m|m.local_start_not_before_ns))
    }
    pub(crate) fn is_full_run(&self)->bool{self.workflow.full_run.is_some()}
    pub(crate) fn current_context_matches(&self,context:&CaptureContext)->bool{self.workflow.context()==context}
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
        // Reuse authenticated retired-head chaining. An active predecessor is
        // refused; this never upgrades/replaces an existing capture in place.
        let predecessor=store.fresh_train_genesis_predecessor(now)?;
        Self::prepare_with(schedule,run,config,now,&store,move|w,store|{
            let binding=&w.session.managed.as_ref().ok_or("managed_session_required")?.binding;
            w.full_run=Some(full_run::FullRunState::fresh(binding,&w.session.run_id,witness)?);
            w.commit_token=predecessor;
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
    /// Explicit NEW Train genesis only. Existing generic/old constructors and
    /// cold restore retain their stored profile; C4 never inherits this choice.
    pub(crate) fn prepare_async_fresh_train_343(
        schedule:ManagedScheduleContract,run:String,config:FitConfig,now:u64,
        store:std::sync::Arc<RecoveryStore>,
    )->Result<(Self,Option<String>),String>{
        let predecessor=store.fresh_train_genesis_predecessor(now)?;
        let mut workflow=TemporalWorkflow::prepare(schedule,run,config,now)?;
        let schedule=workflow.schedule()?;
        workflow.bridge=Some(BridgeState::fresh_train_343());
        workflow.commit_token=predecessor;
        let writer=std::sync::Arc::new(durability::Writer::new(store.clone(),now)?);
        workflow.async_state=Some(durability::AsyncState::new(writer));
        let error=workflow.persist_initial(&store).err();
        if error.is_some(){workflow.stage=Stage::BlockedDurability;}
        Ok((Self{public_generation_id:public_generation_id(&workflow)?,workflow,schedule,failure:None,durable_seals:0},error))
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
        if start.is_some(){self.workflow.prepare_complete_from_actual_seal_ack(now)?;}
        Ok(start)
    }
    pub(crate) fn enable_async_restore(&mut self,store:std::sync::Arc<RecoveryStore>,now:u64)->Result<(),String>{
        if self.workflow.async_state.is_some(){return Err("temporal_writer_already_present".into());}
        let mut state=durability::AsyncState::new(std::sync::Arc::new(durability::Writer::new(store,now)?));
        state.seed_restored(&self.workflow)?;
        self.workflow.async_state=Some(state);
        self.workflow.prepare_complete_from_actual_seal_ack(now)?;Ok(())
    }
    pub(crate) fn current_lead_in_ns(&self)->Result<u64,String>{self.schedule.episodes().get(self.workflow.episode).ok_or("temporal_capture_complete")?.lead_in_ms.checked_mul(1_000_000).ok_or("temporal_time_overflow".into())}
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
        workflow.bridge = Some(if schedule.is_operator_eight_step(){BridgeState::operator_eight_step()}else{BridgeState::new()});
        let error = initial(&mut workflow, store).err();
        if error.is_some() {
            workflow.stage = Stage::BlockedDurability;
        }
        Ok((
            Self {
                public_generation_id:public_generation_id(&workflow)?,
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
            && matches!(self.stage(), TemporalCaptureStage::Complete | TemporalCaptureStage::Frozen | TemporalCaptureStage::FullComplete);
        if !matches!(
            self.stage(),
            TemporalCaptureStage::Ready | TemporalCaptureStage::Paused | TemporalCaptureStage::Preparing | TemporalCaptureStage::Blocked
        ) && !(self.workflow.async_state.is_some() && self.logical_phase_complete())
            && !full_run_continuation_fence {
            return Err("temporal_priming_state".into());
        }
        // No recorded observation exists in these closed Full2 stages. A
        // changing host-arrival estimate may prime identity/replay fences,
        // while the frozen recording band remains mandatory in ingest().
        let full_run_idle = self.is_full_run() && matches!(self.stage(),
            TemporalCaptureStage::Ready | TemporalCaptureStage::Paused |
            TemporalCaptureStage::Preparing | TemporalCaptureStage::Blocked |
            TemporalCaptureStage::Complete | TemporalCaptureStage::Frozen |
            TemporalCaptureStage::FullComplete);
        let result: Result<(), String> = (|| {
            let w = &mut self.workflow;
            w.check_clock(now)?;
            let native = &w.session.managed.as_ref()
                .ok_or("managed_session_required")?.binding.native;
            if full_run_idle {
                native.validate_priming_packet_source_identity(p)?;
            } else {
                native.validate_packet_source(p)?;
            }
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
        if w.full_run.as_ref().is_some_and(|m|m.is_complete()){return TemporalCaptureStage::FullComplete;}
        if w.full_run.as_ref().is_some_and(|m|m.is_frozen()&&m.held_out.is_none()){return TemporalCaptureStage::Frozen;}
        if w.full_run.as_ref().is_none_or(|m|m.held_out.is_none()) && w.bridge
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
        if e.role != DataRole::Train && !(schedule.is_operator_eight_step()&&e.role==DataRole::Validation) && !w.full_run.as_ref().is_some_and(|m|m.is_frozen()) {return None;}
        let frozen=w.frozen_identity().ok()?;
        let bound = schedule
            .bind_episode_start(
                &w.session.run_id,w.episode,0,w.mapping.as_ref().map_or(0, |m| m.segment_ordinal),frozen.as_ref(),
            )
            .ok()?;
        Some((bound.trial_id().into(), bound.session_id().into()))
    }
    fn logical_phase_complete(&self)->bool {
        let w=&self.workflow;
        self.schedule.episodes().get(w.episode).is_some_and(|e|(e.role==DataRole::Train || self.schedule.is_operator_eight_step()&&e.role==DataRole::Validation || w.full_run.as_ref().is_some_and(|m|m.held_out.is_some()))
            && w.pending_window.is_none()
            && w.session.rows.len()==self.schedule.episodes().iter().take(w.episode+1).map(|e|e.slots.len()).sum::<usize>())
    }
    fn phase_ready(&self) -> bool {
        let w = &self.workflow;
        let s = &self.schedule;
        let Some(e) = s.episodes().get(w.episode) else {
            return false;
        };
        (e.role == DataRole::Train || s.is_operator_eight_step()&&e.role==DataRole::Validation || w.full_run.as_ref().is_some_and(|m|m.held_out.is_some()))
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
        let role=schedule.episodes().get(w.episode).ok_or("temporal_capture_complete")?.role;
        if role==DataRole::HeldOut{w.begin_held_out_role(now)?;}
        else if role!=DataRole::Train&&!(schedule.is_operator_eight_step()&&role==DataRole::Validation){return Err("temporal_train_complete".into());}
        else if w.full_run.is_some()&&w.mapping.is_none(){
            let b=w.bridge.as_ref().ok_or("temporal_bridge_required")?;
            let expected=schedule.episodes().iter().take(w.episode).map(|e|e.slots.len()).sum::<usize>();
            if b.seals.len()!=w.episode||!b.parts.is_empty()||w.session.rows.len()!=expected||w.async_state.as_ref().is_none_or(|a|a.pending.is_some()||a.accepted_seals!=b.seals.len()){
                return Err("temporal_initial_episode_prior_seal_ack_required".into());
            }
        }
        let permit = ConditionsPermit {
            context: w.context().digest(),
            run: w.session.run_id.clone(),
            durable_sha256: w.durable.as_ref().map(|b| bytes_hash(b)),
        };
        w.start_segment(&permit, epoch, now, now)?;
        let mapping = w.mapping.clone().ok_or("temporal_segment_missing")?;
        w.set_current_disposition(Disposition::Observing)?;
        w.current_parts_mut()?.push(QualityPart {
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
            w.current_parts_mut()?.last_mut()
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
        w.current_parts_mut()?.last_mut()
            .ok_or("temporal_quality_segment_missing")?
            .observed_end_ns = deadline;
        if deadline == schedule.episodes()[w.episode].target_active_recording_ms * 1_000_000 {
            w.set_current_disposition(Disposition::NeedsConfirmation)?;
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
        if let Some(mode)=w.full_run.as_ref().filter(|m|m.held_out.is_some()){
            let held=mode.held_out.as_ref().unwrap();let fit=mode.frozen.as_ref().ok_or("temporal_verified_frozen_required")?;
            return hash(&(held_out::HELD_OUT_CONDITIONS_DOMAIN,&w.descriptor.id,&w.session.run_id,w.episode,w.durable_digest(),&fit.sha256,&held.parts,&held.seals));
        }
        let b = w.bridge.as_ref().ok_or("temporal_bridge_required")?;
        hash(&(
            b.profile()?.policy(),
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
        if schedule.episodes().get(w.episode).is_some_and(|e|e.role==DataRole::HeldOut){return self.confirm_held_out(store,now);}
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
        seal.sha256 = seal.digest(b.profile()?)?;
        b.seals.push(seal);
        b.parts.clear();
        w.episode += 1;
        w.mapping = None;
        w.custody = SegmentCustody::default();
        w.live_observed_ns = 0;
        b.disposition = if w.episode == if schedule.is_operator_eight_step(){schedule.episodes().len()}else{schedule.counts().train_episodes} {
            Disposition::Complete
        } else {
            Disposition::Ready
        };
        w.stage = Stage::DurableReady;
        w.persist_with(store, false, |s, i, p, n| s.publish_temporal(i, Some(p), n))?;
        if w.async_state.is_none(){self.durable_seals = w.bridge.as_ref().unwrap().seals.len();}
        Ok(())
    }
    fn confirm_held_out(&mut self,store:&RecoveryStore,now:u64)->Result<(),String>{
        let w=&mut self.workflow;let schedule=w.schedule()?;
        let frozen=w.frozen_identity()?.ok_or("temporal_verified_frozen_required")?;
        let mode=w.full_run.as_ref().unwrap();let fit=mode.frozen.as_ref().unwrap();let held=mode.held_out.as_ref().ok_or("temporal_heldout_bridge_required")?;
        if w.episode!=7+held.seals.len()||held.seals.len()>=8{return Err("temporal_heldout_seal_order".into());}
        validate_parts_with_frozen(&held.parts,&schedule,&w.session.run_id,w.episode,Some(30_000_000_000),now,Some(&frozen))?;
        let state=w.async_state.as_ref().ok_or("temporal_async_required")?;
        if state.pending.is_some()||state.accepted_held_out_seals!=held.seals.len(){return Err("temporal_heldout_prior_seal_ack_required".into());}
        let row_prefix_sha256=state.row_prefix_sha256.clone().ok_or("temporal_row_prefix_unconfirmed")?;
        let bound=schedule.bind_episode_start(&w.session.run_id,w.episode,0,0,Some(&frozen))?;
        let seal=held_out::HeldOutSealData::from_body(held_out::HeldOutSealBody{role:DataRole::HeldOut,episode:w.episode,trial_id:bound.trial_id().into(),context_sha256:w.context().digest(),schedule_sha256:schedule.sha256().into(),continuation_policy_sha256:mode.policy.sha256().into(),verified_fit_sha256:fit.sha256.clone(),artifact_kind:fit.body.artifact_kind.clone(),artifact_sha256:fit.body.artifact_sha256.clone(),row_prefix_sha256,quality_sha256:hash(&held.parts)?,parts:held.parts.clone(),confirmed_at_ns:now,previous_sha256:held.seals.last().map(|s|s.sha256.clone())})?;
        seal.validate_binding_data(&schedule,&w.session.run_id,fit,now)?;
        let held=w.full_run.as_mut().unwrap().held_out.as_mut().unwrap();held.seals.push(seal);held.parts.clear();
        w.episode+=1;w.mapping=None;w.custody=SegmentCustody::default();w.live_observed_ns=0;
        held.disposition=if w.episode==15{Disposition::Complete}else{Disposition::Ready};w.stage=Stage::DurableReady;w.descriptor.saved_at_ms=now/1_000_000;
        w.persist_with(store,false,|s,i,p,n|s.publish_temporal(i,Some(p),n))
    }
    pub(crate) fn pause(&mut self, now: u64, store: &RecoveryStore) -> Result<(), String> {
        self.ensure_live()?;
        if self.workflow.async_state.is_some(){return self.workflow.request_async_pause(now);}
        self.workflow.check_clock(now)?;
        self.workflow.pause()?;
        self.workflow.set_current_disposition(Disposition::Paused)?;
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
            public_generation_id:public_generation_id(&w)?,
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
        Ok(Self{public_generation_id:public_generation_id(&w)?,workflow:w,schedule,failure:None,durable_seals:n})
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
    /// Metadata-only update after the exact packet has already failed admission.
    pub(crate) fn annotate_receiver_failure(&mut self, expected: &str, detail: String) -> bool {
        if self.stage() != TemporalCaptureStage::Failed || self.failure.as_deref() != Some(expected)
            || detail.len() > 256 || !detail.starts_with(expected) { return false; }
        self.failure = Some(detail);
        true
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
        let full=w.full_run.is_some();
        let mut value=json!({"schema":"ruview.room-learner.private-temporal-capture.v1","stage":self.stage().as_str(),"episode_index":w.episode,"label":e.filter(|e|e.role==DataRole::Train||full).map(|e|e.condition),"role":if self.schedule.is_operator_eight_step(){e.map(|e|match e.role{DataRole::Train=>"Train",DataRole::Validation=>"Validation",DataRole::HeldOut=>"HeldOut",DataRole::Inference=>"Inference"}).unwrap_or("Train")}else if full&&e.is_some_and(|e|e.role==DataRole::HeldOut){"HeldOut"}else{"Train"},"sealed_train_episodes":if self.schedule.is_operator_eight_step(){self.schedule.episodes().iter().take(self.durable_seals).filter(|e|e.role==DataRole::Train).count()}else{self.durable_seals},"retained_rows":w.session.rows.len(),"durable_windows":w.durable_windows,"rows_sha256":w.async_state.as_ref().map(|a|if w.session.rows.len()==w.durable_windows{a.rows_sha256.clone()}else{None}).unwrap_or_else(||hash(&w.session.rows).ok()),"durability_pending":w.async_state.as_ref().is_some_and(|a|a.pending.is_some()),"durability_error":w.async_state.as_ref().and_then(|a|a.error.clone()),"observed_ns":w.live_observed_ns.to_string(),"durable_observed_ns":w.accepted().to_string(),"target_ms":e.filter(|e|e.role==DataRole::Train||full).map(|e|e.target_active_recording_ms),"descriptor":w.descriptor,"failure":self.failure,"fit_admission_available":false,"quality_is_engineering_only":true,"independence_verified":false,"source_clock_measured":false});
        if full{value["sealed_held_out_episodes"]=json!(if self.schedule.is_operator_eight_step(){self.schedule.episodes().iter().take(self.durable_seals).filter(|e|e.role==DataRole::Validation).count()}else{w.async_state.as_ref().map_or(0,|s|s.accepted_held_out_seals)});}
        value
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

#[cfg(test)]
#[path="temporal_budget_profile_tests.rs"]
mod budget_profile_tests;

impl TemporalTrainCapture {
    pub(crate) fn prepare_evaluation_source(&self,now:u64,held_out:bool)->Result<full_run::PendingTemporalEvidenceSource,String>{
        self.ensure_live()?;self.workflow.prepare_temporal_evidence_source(now,held_out,&self.schedule)
    }
    pub(crate) fn evaluation_binding_matches(&self,binding:&full_run::TemporalEvaluationBinding)->bool{
        self.failure.is_none()&&self.workflow.current_temporal_evidence_matches(binding)
    }
}

// Informational public generation binding only, never an owner/source permit.
// Computed once by off-lock construction/recovery, not on packet ingress.
fn public_generation_id(w:&TemporalWorkflow)->Result<String,String>{hash(&("ruview.room-learner.capture-generation.v2",&w.descriptor.id,&w.session.run_id,w.context().digest()))}
impl TemporalTrainCapture {
    pub(crate) fn public_generation_id(&self)->&str{&self.public_generation_id}
    pub(crate) fn public_phase_matches(&self,generation:&str,schedule:&str,episode:usize)->bool{self.public_generation_id==generation&&self.schedule.sha256()==schedule&&self.workflow.episode==episode}
    pub(crate) fn public_profile(&self)->crate::room_learner::temporal_wire::TemporalProfileWire{
        use crate::room_learner::temporal_wire::*;
        TemporalProfileWire{profile_id:self.schedule.profile_id().into(),window_unit:"native_receiver_2s_window".into(),native_window_ms:2000,train_windows:self.schedule.counts().train_windows as u16,held_out_windows:self.schedule.counts().held_out_windows as u16,train_episodes:self.schedule.counts().train_episodes as u8,held_out_episodes:self.schedule.counts().held_out_episodes as u8,windows_per_condition:[16;4],train_episodes_per_condition:if self.schedule.is_operator_eight_step(){[1;4]}else{[1,2,2,2]},held_out_episodes_per_condition:if self.schedule.is_operator_eight_step(){[1;4]}else{[2;4]},episodes:self.schedule.episodes().iter().map(|e|TemporalEpisodeWire{index:e.index as u8,role:e.role,label:e.condition,target_ms:e.target_active_recording_ms,lead_in_ms:e.lead_in_ms,native_windows:e.slots.len()as u16}).collect()}
    }
    pub(crate) fn public_capture(&self,epoch:&str,lead_until:Option<u64>,now:u64)->crate::room_learner::temporal_wire::TemporalCaptureWire{
        use crate::room_learner::temporal_wire::*;
        let w=&self.workflow;let episode=self.schedule.episodes().get(w.episode);let lead=lead_until.map_or(0,|end|end.saturating_sub(now)/1_000_000);
        let challenge=if matches!(self.stage(),TemporalCaptureStage::NeedsConfirmation|TemporalCaptureStage::Paused){self.confirmation_challenge().ok()}else{None};
        TemporalCaptureWire{capture_generation_id:self.public_generation_id.clone(),schedule_sha256:self.schedule.sha256().into(),continuation_policy_sha256:w.full_run.as_ref().map(|m|m.policy.sha256().into()).unwrap_or_default(),durable_commitment_sha256:w.commit_token.as_ref().map(|t|t.data().commitment),original_deadline:TemporalOriginalDeadlineWire{created_at_unix_ms:w.descriptor.created_at_ms,expires_at_unix_ms:w.descriptor.expires_at_ms},receiver_epoch:epoch.into(),stage:if lead>0{"preparing".into()}else{self.stage().as_str().into()},current_episode:episode.map(|e|e.index as u8),role:episode.map(|e|e.role),label:episode.map(|e|e.condition),target_ms:episode.map(|e|e.target_active_recording_ms),lead_in_remaining_ms:lead,observed_ms:w.live_observed_ns/1_000_000,retained_windows:w.session.rows.len()as u16,durable_windows:w.durable_windows as u16,sealed_train_episodes:if self.schedule.is_operator_eight_step(){self.schedule.episodes().iter().take(self.durable_seals).filter(|e|e.role==DataRole::Train).count()as u8}else{self.durable_seals as u8},sealed_held_out_episodes:if self.schedule.is_operator_eight_step(){self.schedule.episodes().iter().take(self.durable_seals).filter(|e|e.role==DataRole::Validation).count()as u8}else{w.async_state.as_ref().map_or(0,|a|a.accepted_held_out_seals as u8)},durability_pending:w.async_state.as_ref().is_some_and(|a|a.pending.is_some()),awaiting_ground_truth:challenge.is_some(),confirmation_challenge:challenge,failure:self.failure.clone()}
    }
    pub(crate) fn public_candidate(&self)->Option<crate::room_learner::temporal_wire::TemporalCandidateIdentityWire>{
        use crate::room_learner::temporal_wire::*;
        let mode=self.workflow.full_run.as_ref()?;let record=mode.public_verified_fit()?;
        Some(TemporalCandidateIdentityWire{model_kind:WholeTemporalKind::ManagedTemporalV3,model_sha256:record.body.artifact_sha256.clone(),verified_fit_record_sha256:record.sha256.clone(),context_digest:record.body.context_digest.clone(),room_digest:self.context().room_digest.clone(),persisted:true})
    }
    pub(crate) fn public_complete_refs(&self,proof_sha:&str)->Option<crate::room_learner::temporal_wire::TemporalCompleteRefsWire>{
        use crate::room_learner::temporal_wire::*;
        let mode=self.workflow.full_run.as_ref()?;let record=mode.public_complete(self.workflow.commit_token.as_ref())?;let fit=mode.public_verified_fit()?;
        Some(TemporalCompleteRefsWire{model:TemporalModelIdentityWire{model_kind:WholeTemporalKind::ManagedTemporalV3,model_sha256:fit.body.artifact_sha256.clone(),verified_fit_record_sha256:fit.sha256.clone()},training_proof_sha256:proof_sha.into(),complete_record_sha256:record.sha256.clone(),complete_commitment_sha256:self.workflow.commit_token.as_ref()?.data().commitment})
    }
}

impl TemporalTrainCapture {
    /// Authenticated current recovery is decoded only to discover its context.
    /// This DATA does not mint a continued/Frozen witness; restore does that.
    pub(crate) fn saved_full_context(store:&RecoveryStore,now:u64)->Result<(CaptureContext,String),String>{
        let loaded=store.load_temporal(now)?.ok_or("recovery_missing")?;
        let prefix=loaded.prefix.ok_or("temporal_retired")?;
        if prefix.metadata_kind()!=full_run::META_KIND{return Err("temporal_full_run_no_upconversion".into());}
        let w=checkpoint::decode(prefix.bytes(),prefix.descriptor(),now)?;
        Ok((w.context().clone(),public_generation_id(&w)?))
    }
    pub(crate) fn retirement_generation(store:&RecoveryStore,now:u64)->Result<String,String>{
        let inspection=store.inspect_temporal_retirement(now)?.ok_or("recovery_missing")?;
        if inspection.metadata_kind()!=full_run::META_KIND{return Err("temporal_full_run_no_upconversion".into());}
        hash(&("ruview.room-learner.capture-generation.v2",&inspection.descriptor().id,inspection.run(),&inspection.descriptor().context_digest))
    }
}

impl TemporalTrainCapture {pub(crate) fn current_source_floors(&self)->(Vec<(u8,u64,u64)>,u64){(self.workflow.source_high.clone(),self.workflow.last_now_ns)}}

impl TemporalTrainCapture {
    pub(crate) fn recovery_discovery(store:&RecoveryStore,now:u64)->Option<crate::room_learner::temporal_wire::TemporalRecoveryStatusWire>{
        use crate::room_learner::temporal_wire::*;
        let result=(||->Result<Option<TemporalRecoveryStatusWire>,String>{
            let Some(inspection)=store.inspect_temporal_read_only(now)?else{return Ok(None)};
            if inspection.metadata_kind()!=full_run::META_KIND||inspection.retired(){return Ok(None)}
            let descriptor=inspection.descriptor();let generation=hash(&("ruview.room-learner.capture-generation.v2",&descriptor.id,inspection.run(),&descriptor.context_digest))?;
            Ok(Some(TemporalRecoveryStatusWire{schema:RECOVERY_STATUS_SCHEMA.into(),state:if inspection.expired(){"expired"}else{"available"}.into(),capture_generation_id:Some(generation),context_digest:Some(descriptor.context_digest.clone()),original_deadline:Some(TemporalOriginalDeadlineWire{created_at_unix_ms:descriptor.created_at_ms,expires_at_unix_ms:descriptor.expires_at_ms})}))
        })();
        result.unwrap_or_else(|_|Some(TemporalRecoveryStatusWire{schema:RECOVERY_STATUS_SCHEMA.into(),state:"unavailable".into(),capture_generation_id:None,context_digest:None,original_deadline:None}))
    }
}

/// Opaque same-process Retry fence snapshot. It cannot be deserialized and
/// never grants recording or conditions authority.
pub(crate) struct FailedRetryState {
    generation: String,
    schedule: String,
    created_at_ms: u64,
    expires_at_ms: u64,
    durable_windows: usize,
    seals: usize,
    accepted_prefix_sha256: Option<String>,
    accepted_seal_digests: Vec<String>,
    fences: AssemblerCheckpoint,
    source_high: Vec<(u8, u64, u64)>,
}
impl TemporalTrainCapture {
    pub(crate) fn failed_retry_state(&mut self) -> Result<FailedRetryState, String> {
        if !matches!(self.stage(), TemporalCaptureStage::Failed | TemporalCaptureStage::Blocked)
            || !self.is_full_run()
            || self.workflow.full_run.as_ref().is_some_and(|m| m.is_frozen() || m.is_complete())
        { return Err("temporal_retry_requires_stopped_capture".into()); }
        let state = FailedRetryState {
            generation:self.public_generation_id.clone(), schedule:self.schedule.sha256().into(),
            created_at_ms:self.workflow.descriptor.created_at_ms, expires_at_ms:self.workflow.descriptor.expires_at_ms,
            durable_windows:self.workflow.durable_windows, seals:self.durable_seals,
            accepted_prefix_sha256:self.workflow.async_state.as_ref().and_then(|state|state.row_prefix_sha256.clone()),
            accepted_seal_digests:self.workflow.bridge.as_ref().map_or_else(Vec::new,|b|b.seals.iter().take(self.durable_seals).map(|seal|seal.sha256.clone()).collect()),
            fences:self.workflow.session.assembler.fence_checkpoint()?, source_high:self.workflow.source_high.clone(),
        };
        if self.failure.is_none() { self.invalidate("temporal_retry_stopped_capture"); }
        Ok(state)
    }
    pub(crate) fn apply_failed_retry_state(&mut self, old: FailedRetryState, now:u64) -> Result<(), String> {
        if self.public_generation_id != old.generation || self.schedule.sha256()!=old.schedule
            || self.workflow.descriptor.created_at_ms!=old.created_at_ms || self.workflow.descriptor.expires_at_ms!=old.expires_at_ms
            || now/1_000_000>=old.expires_at_ms || self.workflow.durable_windows<old.durable_windows || self.durable_seals<old.seals
        { return Err("temporal_retry_committed_identity_changed".into()); }
        if let Some(expected)=old.accepted_prefix_sha256.as_ref(){
            let n=old.durable_windows;
            if hash(&(&self.workflow.session.rows[..n],&self.workflow.session.evidence[..n],&self.workflow.ledger.spans[..n]))?!=*expected {
                return Err("temporal_retry_committed_prefix_changed".into());
            }
        }
        if self.workflow.bridge.as_ref().is_none_or(|b|b.seals.iter().take(old.seals).map(|s|s.sha256.clone()).collect::<Vec<_>>()!=old.accepted_seal_digests){
            return Err("temporal_retry_confirmed_seals_changed".into());
        }
        let n=self.workflow.session.rows.len();
        let budget=self.workflow.full_run.as_ref().ok_or("temporal_full_run_required")?.policy.replay_fence_budget(&self.workflow.session.managed.as_ref().unwrap().binding)?;
        self.workflow.session.assembler.restore_full_run_prefix_fences(old.fences.for_prefix(n)?,n,&budget)?;
        for (id,seq,time) in old.source_high {
            if time>now {return Err("temporal_retry_receiver_clock_changed".into());}
            if let Some((_,old_seq,old_time))=self.workflow.source_high.iter_mut().find(|(source,_,_)|*source==id) {
                *old_seq=(*old_seq).max(seq); *old_time=(*old_time).max(time);
            } else {self.workflow.source_high.push((id,seq,time));}
        }
        self.workflow.source_high.sort();
        self.workflow.stage=Stage::PausedNeedsReconfirmation;
        self.workflow.last_now_ns=now;
        self.failure=None;
        Ok(())
    }
}

#[cfg(test)]
impl TemporalTrainCapture {
    pub(crate) fn test_corrupt_retry_prefix(&mut self){self.workflow.async_state.as_mut().unwrap().row_prefix_sha256=Some("f".repeat(64));}
}

#[cfg(test)]
#[path = "receiver_failure_annotation_tests.rs"]
mod receiver_failure_annotation_tests;
