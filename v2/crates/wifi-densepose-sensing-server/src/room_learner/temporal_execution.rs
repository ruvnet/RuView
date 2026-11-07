//! Private future ONE Runtime Temporal workflow; no public constructor/API issuer.
//! Durability is not scientific qualification or an assertion of whole-trial quality.
use super::*;
use crate::room_learner::recovery_store::temporal_commitment::{
    TemporalAbortTicket, TemporalIntent, TemporalToken,
};
use crate::room_learner::{
    managed_schedule_contract::BoundEpisode,
    recovery_store::{RecoveryDescriptor, RecoveryStore, RECOVERY_TTL_MS},
    temporal_acquisition::{AssemblerCheckpoint, ManagedWindowTarget},
};
use serde::{Deserialize, Serialize};
#[path = "temporal_durability.rs"]
mod durability;
use durability::AsyncState;
pub(crate) use durability::DeferredReclamation;
#[cfg(test)] pub(crate) use durability::ReclamationStats;
#[path = "temporal_capture_quality.rs"]
mod bridge;
use bridge::BridgeState;
pub(crate) use bridge::{TemporalCaptureStage, TemporalTrainCapture};
#[path = "temporal_fit_admission.rs"]
mod fit_admission;
pub(crate) use fit_admission::{PendingTemporalFitInput, TemporalFitCaptureIdentity,VerifiedTemporalFitSource};
#[path = "temporal_full_run.rs"]
pub(crate) mod full_run;

pub(crate) const EXECUTION_POLICY:&str="observed-complete-reservation-prefix-v1;paused-time;immutable-spans;partial-tail-drop;max128segments;persegment-native-firstarrival-skew-gaps;explicit-genesis-gap-anchor;quality-unimplemented;independence-false";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObservedSegmentMap {
    pub(crate) segment_ordinal: u16,
    pub(crate) session_id: String,
    pub(crate) receiver_epoch: String,
    pub(crate) clock_anchor_ns: u64,
    pub(crate) receiver_start_ns: u64,
    pub(crate) observed_start_ns: u64,
}
impl ObservedSegmentMap {
    pub(crate) fn slot_bounds(
        &self,
        b: &BoundEpisode,
        a: u64,
        z: u64,
    ) -> Result<(u64, u64), String> {
        if self.segment_ordinal != b.segment_ordinal()
            || self.session_id != b.session_id()
            || self.receiver_epoch.is_empty()
            || self.receiver_epoch.len() > 128
            || self.receiver_start_ns == 0
            || self.clock_anchor_ns == 0
            || self.receiver_start_ns < self.clock_anchor_ns
            || a < self.observed_start_ns
            || z <= a
            || z > b.episode().target_active_recording_ms * 1_000_000
        {
            return Err("managed_observed_segment_mapping".into());
        }
        Ok((
            self.receiver_start_ns
                .checked_add(a - self.observed_start_ns)
                .ok_or("managed_mapping_overflow")?,
            self.receiver_start_ns
                .checked_add(z - self.observed_start_ns)
                .ok_or("managed_mapping_overflow")?,
        ))
    }
    #[cfg(test)]
    pub(crate) fn synthetic_uninterrupted(b: &BoundEpisode, start: u64) -> Self {
        Self {
            segment_ordinal: b.segment_ordinal(),
            session_id: b.session_id().into(),
            receiver_epoch: "synthetic-uninterrupted".into(),
            clock_anchor_ns: start,
            receiver_start_ns: start,
            observed_start_ns: 0,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CommittedSpan {
    episode: usize,
    slot: usize,
    mapping: ObservedSegmentMap,
    observed_start_ns: u64,
    observed_end_ns: u64,
    receiver_end_ns: u64,
    previous_sha256: Option<String>,
    window_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct GenesisGap {
    previous_mapping: ObservedSegmentMap,
    paused_receiver_ns: u64,
    source_floors: Vec<(u8, u64, u64)>,
    sha256: String,
}
impl GenesisGap {
    fn digest(
        &self,
        descriptor: &RecoveryDescriptor,
        run: &str,
        episode: usize,
    ) -> Result<String, String> {
        hash(&(
            "temporal-zero-prefix-genesis-v1",
            EXECUTION_POLICY,
            &descriptor.id,
            &descriptor.owner_scope,
            &descriptor.installation_id,
            &descriptor.context_digest,
            descriptor.created_at_ms,
            descriptor.expires_at_ms,
            run,
            episode,
            &self.previous_mapping,
            self.paused_receiver_ns,
            &self.source_floors,
        ))
    }
    fn new(
        descriptor: &RecoveryDescriptor,
        run: &str,
        episode: usize,
        previous_mapping: ObservedSegmentMap,
        paused_receiver_ns: u64,
        source_floors: Vec<(u8, u64, u64)>,
    ) -> Result<Self, String> {
        let mut v = Self {
            previous_mapping,
            paused_receiver_ns,
            source_floors,
            sha256: String::new(),
        };
        v.sha256 = v.digest(descriptor, run, episode)?;
        Ok(v)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Gap {
    episode: usize,
    after_prefix_sha256: Option<String>,
    genesis: Option<GenesisGap>,
    accepted_observed_ns: u64,
    previous_receiver_ns: u64,
    resume_receiver_ns: u64,
    discarded_live_ns: u64,
    new_mapping: ObservedSegmentMap,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Ledger {
    spans: Vec<CommittedSpan>,
    gaps: Vec<Gap>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Prepared,
    DurableReady,
    Observing,
    PausedNeedsReconfirmation,
    PreparedSegment,
    BlockedDurability,
}
/// Neither JSON nor a generic saved checkpoint can construct an operator permit.
struct ConditionsPermit {
    context: String,
    run: String,
    durable_sha256: Option<String>,
}
/// Transient per-segment observation custody, distinct from persistent replay
/// high-watermarks. It is NEVER restored as active observation from JSON.
#[derive(Default)]
struct SegmentCustody {
    first_last: Vec<(u8, u64, u64)>,
    failed: bool,
}
/// Future value owned by existing RoomLearningRuntime::Inner's ONE tagged workflow.
/// No production constructor/permit issuer is exposed by this source slice.
struct TemporalWorkflow {
    full_run: Option<full_run::FullRunState>,
    session: TemporalTrainingSession,
    descriptor: RecoveryDescriptor,
    ledger: Ledger,
    stage: Stage,
    mapping: Option<ObservedSegmentMap>,
    episode: usize,
    last_now_ns: u64,
    live_observed_ns: u64,
    source_high: Vec<(u8, u64, u64)>,
    pending_window: Option<ManagedWindowTarget>,
    durable: Option<std::sync::Arc<Vec<u8>>>,
    durable_windows: usize,
    custody: SegmentCustody,
    commit_token: Option<TemporalToken>,
    pending_intent: Option<TemporalIntent>,
    pending_prefix: Option<checkpoint::ValidatedTemporalPrefix>,
    bridge: Option<BridgeState>,
    async_state: Option<AsyncState>,
}
impl TemporalWorkflow {
    fn context(&self) -> &CaptureContext {
        &self.session.binding.context
    }
    fn schedule(&self) -> Result<ManagedScheduleContract, String> {
        self.session
            .managed
            .as_ref()
            .ok_or("managed_session_required")?
            .binding
            .contract()
    }
    fn prefix_hash(&self) -> Result<Option<String>, String> {
        self.ledger.spans.last().map(hash).transpose()
    }
    fn accepted(&self) -> u64 {
        self.ledger
            .spans
            .iter()
            .take(self.durable_windows)
            .last()
            .filter(|s| s.episode == self.episode)
            .map_or(0, |s| s.observed_end_ns)
    }
    fn check_clock(&mut self, now: u64) -> Result<(), String> {
        if now < self.last_now_ns || now / 1_000_000 >= self.descriptor.expires_at_ms {
            return Err("temporal_receiver_clock_or_expiry".into());
        }
        self.last_now_ns = now;
        Ok(())
    }
    fn prepare(
        schedule: ManagedScheduleContract,
        run: String,
        config: FitConfig,
        now: u64,
    ) -> Result<Self, String> {
        if now == 0 {
            return Err("temporal_receiver_clock".into());
        }
        let c = schedule.context().clone();
        let digest = c.digest();
        let session = TemporalTrainingSession::managed(schedule, run, config)?;
        Ok(Self {
            full_run: None,
            session,
            descriptor: RecoveryDescriptor {
                schema: crate::room_learner::recovery_store::RECOVERY_SCHEMA.into(),
                id: crate::companion::secret(),
                owner_scope: c.owner_scope,
                installation_id: c.installation_id,
                context_digest: digest,
                created_at_ms: now / 1_000_000,
                saved_at_ms: now / 1_000_000,
                expires_at_ms: now / 1_000_000 + RECOVERY_TTL_MS,
            },
            ledger: Ledger::default(),
            stage: Stage::Prepared,
            mapping: None,
            episode: 0,
            last_now_ns: now,
            live_observed_ns: 0,
            source_high: vec![],
            pending_window: None,
            durable: None,
            durable_windows: 0,
            custody: SegmentCustody::default(),
            commit_token: None,
            pending_intent: None,
            pending_prefix: None,
            bridge: None,
            async_state: None,
        })
    }
    #[cfg(test)]
    fn synthetic_conditions(&self) -> ConditionsPermit {
        ConditionsPermit {
            context: self.context().digest(),
            run: self.session.run_id.clone(),
            durable_sha256: self.durable_digest(),
        }
    }
    fn start_segment(
        &mut self,
        permit: &ConditionsPermit,
        epoch: &str,
        anchor: u64,
        start: u64,
    ) -> Result<(), String> {
        if !matches!(
            self.stage,
            Stage::DurableReady | Stage::PausedNeedsReconfirmation
        ) || permit.context != self.context().digest()
            || permit.run != self.session.run_id
            || permit.durable_sha256 != self.durable_digest()
            || epoch.is_empty()
            || epoch.len() > 128
            || anchor < self.last_now_ns
            || start < anchor
            || self.ledger.gaps.len() >= 128
        {
            return Err("temporal_reconfirmation_or_clock_required".into());
        }
        let paused_receiver_ns = self.last_now_ns;
        self.check_clock(start)?;
        let schedule = self.schedule()?;
        let segment = self.mapping.as_ref().map_or(0, |m| m.segment_ordinal + 1);
        let bound =
            schedule.bind_episode_start(&self.session.run_id, self.episode, 0, segment, None)?;
        let mapping = ObservedSegmentMap {
            segment_ordinal: segment,
            session_id: bound.session_id().into(),
            receiver_epoch: epoch.into(),
            clock_anchor_ns: anchor,
            receiver_start_ns: start,
            observed_start_ns: self.accepted(),
        };
        if let Some(old) = &self.mapping {
            let parent = self
                .ledger
                .spans
                .last()
                .filter(|s| s.episode == self.episode)
                .map(hash)
                .transpose()?;
            let genesis = if parent.is_none() {
                Some(GenesisGap::new(
                    &self.descriptor,
                    &self.session.run_id,
                    self.episode,
                    old.clone(),
                    paused_receiver_ns,
                    self.source_high.clone(),
                )?)
            } else {
                None
            };
            let prev = self
                .ledger
                .spans
                .last()
                .filter(|s| s.episode == self.episode)
                .map_or(paused_receiver_ns, |s| s.receiver_end_ns);
            self.ledger.gaps.push(Gap {
                episode: self.episode,
                after_prefix_sha256: parent,
                genesis,
                accepted_observed_ns: self.accepted(),
                previous_receiver_ns: prev,
                resume_receiver_ns: start,
                discarded_live_ns: self.live_observed_ns.saturating_sub(self.accepted()),
                new_mapping: mapping.clone(),
            });
        }
        self.custody = SegmentCustody::default();
        self.mapping = Some(mapping);
        self.live_observed_ns = self.accepted();
        self.stage = Stage::PreparedSegment;
        Ok(())
    }

    fn persist_segment(&mut self, store: &RecoveryStore) -> Result<(), String> {
        if self.stage != Stage::PreparedSegment {
            return Err("temporal_segment_not_prepared".into());
        }
        self.persist_with(store, false, |s, intent, prefix, now_ns| {
            s.publish_temporal(intent, Some(prefix), now_ns)
        })?;
        self.stage = Stage::Observing;
        Ok(())
    }
    fn begin_slot(&mut self) -> Result<ManagedWindowTarget, String> {
        if self.stage != Stage::Observing || self.pending_window.is_some() {
            return Err("temporal_slot_state".into());
        }
        let schedule = self.schedule()?;
        let mapping = self.mapping.clone().ok_or("temporal_segment_missing")?;
        let bound = schedule.bind_episode_start(
            &self.session.run_id,
            self.episode,
            0,
            mapping.segment_ordinal,
            None,
        )?;
        let slot = self.session.rows.len()
            - schedule
                .episodes()
                .iter()
                .take(self.episode)
                .map(|e| e.slots.len())
                .sum::<usize>();
        let target = ManagedWindowTarget::from_observed_episode(
            &schedule,
            &bound,
            &self.session.run_id,
            slot,
            mapping,
        )?;
        self.session.begin_managed_window(&target)?;
        self.pending_window = Some(target.clone());
        Ok(target)
    }
    /// Busy packets and trusted receiver ticks both withdraw custody when a
    /// required source is absent/stale; neither can backfill missing intervals.
    fn custody_at(&mut self, now: u64) -> Result<(), String> {
        if self.stage != Stage::Observing || self.custody.failed {
            return Err("temporal_segment_custody_unavailable".into());
        }
        let mapping = self.mapping.as_ref().ok_or("temporal_segment_missing")?;
        let native = &self
            .session
            .managed
            .as_ref()
            .ok_or("managed_session_required")?
            .binding
            .native;
        let first_limit = native
            .max_first_packet_delay_ms()
            .checked_mul(1_000_000)
            .ok_or("temporal_time_overflow")?;
        if now < mapping.receiver_start_ns {
            return Err("temporal_segment_clock".into());
        }
        for source in native.source_policies() {
            let invalid = match self
                .custody
                .first_last
                .iter()
                .find(|(id, _, _)| *id == source.source_id)
            {
                None => now - mapping.receiver_start_ns > first_limit,
                Some((_, _, last)) => now < *last || now - *last > source.max_gap_ns,
            };
            if invalid {
                self.custody.failed = true;
                return Err("temporal_missing_segment_source".into());
            }
        }
        Ok(())
    }
    fn observe_tick(&mut self, now: u64) -> Result<(), String> {
        self.check_clock(now)?;
        self.custody_at(now)
    }
    fn observe(&mut self, p: &CanonicalPacket, now: u64) -> Result<(), String> {
        if self.stage != Stage::Observing {
            return Err("temporal_not_observing".into());
        }
        self.check_clock(now)?;
        self.custody_at(now)?;
        let source_count = self.context().sources.len();
        let c = self.context();
        let spec = c
            .sources
            .iter()
            .find(|s| s.source_id == p.source_id)
            .ok_or("temporal_unbound_source")?;
        let t = p.native_frame.timestamp_ns;
        if t > now
            || p.grid_id != spec.grid_id
            || p.frequency_hz != spec.center_frequency_hz
            || p.bandwidth_hz != spec.bandwidth_hz
            || p.native_frame.provenance.device_id != spec.device_id
            || p.amplitudes.len() != spec.feature_count
            || p.amplitudes.iter().any(|v| !v.is_finite() || *v < 0.)
            || self.source_high.iter().any(|(id, seq, last)| {
                *id == p.source_id && (u64::from(p.sequence) <= *seq || t <= *last)
            })
        {
            return Err("temporal_observation_source_or_fence".into());
        }
        let mapping = self.mapping.as_ref().ok_or("temporal_segment_missing")?;
        if t < mapping.receiver_start_ns {
            return Err("temporal_segment_replay".into());
        }
        let native = &self
            .session
            .managed
            .as_ref()
            .ok_or("managed_session_required")?
            .binding
            .native;
        let policy = native
            .source_policies()
            .iter()
            .find(|s| s.source_id == p.source_id)
            .ok_or("temporal_unbound_source")?;
        if self.source_high.iter().any(|(id, _, last)| {
            *id == p.source_id
                && *last >= mapping.receiver_start_ns
                && (t - *last > policy.max_gap_ns || t - *last < policy.min_packet_interval_ns)
        }) {
            return Err("temporal_observed_gap_or_rate".into());
        }
        let first_limit = native.max_first_packet_delay_ms() * 1_000_000;
        if let Some((_, _, last)) = self
            .custody
            .first_last
            .iter_mut()
            .find(|(id, _, _)| *id == p.source_id)
        {
            *last = t;
        } else {
            if t - mapping.receiver_start_ns > first_limit {
                self.custody.failed = true;
                return Err("temporal_late_segment_source".into());
            }
            self.custody.first_last.push((p.source_id, t, t));
        }
        let first_min = self
            .custody
            .first_last
            .iter()
            .map(|(_, first, _)| *first)
            .min()
            .unwrap();
        let first_max = self
            .custody
            .first_last
            .iter()
            .map(|(_, first, _)| *first)
            .max()
            .unwrap();
        if first_max - first_min > native.max_pairwise_receiver_skew_ms() * 1_000_000 {
            self.custody.failed = true;
            return Err("temporal_segment_first_source_skew".into());
        }
        self.source_high.retain(|(id, _, _)| *id != p.source_id);
        self.source_high
            .push((p.source_id, u64::from(p.sequence), t));
        self.source_high.sort();
        if self.custody.first_last.len() == source_count {
            let frontier = self
                .custody
                .first_last
                .iter()
                .map(|(_, _, t)| *t)
                .min()
                .unwrap();
            if frontier >= mapping.receiver_start_ns {
                self.live_observed_ns =
                    mapping.observed_start_ns + frontier - mapping.receiver_start_ns;
            }
        }
        if self.session.active {
            let target = self
                .pending_window
                .as_ref()
                .ok_or("temporal_target_missing")?;
            if t >= target.target_ns() {
                if self.session.ingest(p)? == WindowAdmission::Ready {
                    self.session.finish_window(t)?;
                }
            }
        }
        Ok(())
    }
    fn pause(&mut self) -> Result<(), String> {
        if self.stage != Stage::Observing {
            return Err("temporal_pause_state".into());
        }
        let bytes = self
            .durable
            .clone()
            .ok_or("temporal_durable_prefix_missing")?;
        let ram = self.session.assembler.fence_checkpoint()?;
        let high = self.source_high.clone();
        let mapping = self.mapping.clone();
        let live = self.live_observed_ns;
        let last = self.last_now_ns;
        let token = self.commit_token.clone();
        let mut restored = checkpoint::decode(&bytes, &self.descriptor, last)?;
        restored.commit_token = token;
        restored.session.assembler.restore_prefix_fences(
            ram.for_prefix(restored.session.rows.len())?,
            restored.session.rows.len(),
        )?;
        restored.mapping = mapping;
        restored.live_observed_ns = live;
        restored.source_high = high;
        restored.last_now_ns = last;
        restored.stage = Stage::PausedNeedsReconfirmation;
        restored.durable = Some(bytes);
        *self = restored;
        Ok(())
    }
    fn persist_initial(&mut self, store: &RecoveryStore) -> Result<(), String> {
        if self.stage != Stage::Prepared {
            return Err("temporal_prepare_state".into());
        }
        self.persist_with(store, false, |store, intent, prefix, now_ns| {
            store.publish_temporal(intent, Some(prefix), now_ns)
        })?;
        self.stage = Stage::DurableReady;
        Ok(())
    }
    fn commit_slot(&mut self, store: &RecoveryStore, now: u64) -> Result<(), String> {
        self.check_clock(now)?;
        self.persist_with(store, true, |store, intent, prefix, now_ns| {
            store.publish_temporal(intent, Some(prefix), now_ns)
        })?;
        self.last_now_ns = now.max(self.last_now_ns);
        Ok(())
    }
    fn persist_with(
        &mut self,
        store: &RecoveryStore,
        slot: bool,
        save: impl FnOnce(
            &RecoveryStore,
            &TemporalIntent,
            &checkpoint::ValidatedTemporalPrefix,
            u64,
        ) -> Result<TemporalToken, String>,
    ) -> Result<(), String> {
        if self.async_state.as_ref().is_some_and(|a| a.pending.is_some()) {
            self.async_state.as_mut().unwrap().busy_pause = true;
            self.stage = Stage::BlockedDurability;
            return Err("temporal_storage_busy_pause".into());
        }
        if slot {
            self.custody_at(self.last_now_ns)?;
            let t = self
                .pending_window
                .as_ref()
                .ok_or("temporal_no_completed_window")?;
            let schedule = self.schedule()?;
            let e = &schedule.episodes()[self.episode];
            let s = &e.slots[t.identity().slot_index];
            if self.stage != Stage::Observing
                || self.session.active
                || self.session.rows.len() != self.ledger.spans.len() + 1
                || self.last_now_ns < t.end_ns()
                || self.source_high.len() != self.context().sources.len()
                || self.source_high.iter().any(|(_, _, n)| *n < t.end_ns())
            {
                return Err("temporal_full_reservation_not_observed".into());
            }
            let mapping = t.identity().mapping.clone();
            let accepted = self.accepted();
            let span = CommittedSpan {
                episode: self.episode,
                slot: s.index,
                mapping,
                observed_start_ns: accepted,
                observed_end_ns: s.deadline_offset_ms * 1_000_000,
                receiver_end_ns: t.end_ns(),
                previous_sha256: self.prefix_hash()?,
                window_sha256: hash(self.session.rows.last().unwrap())?,
            };
            self.ledger.spans.push(span);
        }
        self.descriptor.saved_at_ms = self.last_now_ns / 1_000_000;
        if self.async_state.is_some() {
            let result=self.submit_async_commit(slot);
            if result.is_err(){self.stage=Stage::BlockedDurability;}
            return result;
        }
        let bytes = checkpoint::encode(self)?;
        let prefix = checkpoint::validated_prefix(&bytes, &self.descriptor, self.last_now_ns)?;
        let intent =
            match store.prepare_temporal(self.commit_token.as_ref(), &prefix, self.last_now_ns) {
                Ok(i) => i,
                Err(e) => {
                    self.stage = Stage::BlockedDurability;
                    return Err(e);
                }
            };
        self.pending_intent = Some(intent.clone());
        self.pending_prefix = Some(prefix.clone());
        let token = match save(store, &intent, &prefix, self.last_now_ns).and_then(|token| {
            let loaded = store
                .load_temporal(self.last_now_ns)?
                .ok_or("temporal_readback_missing")?;
            if loaded.token != token
                || loaded.prefix.as_ref().map(|p| p.bytes()) != Some(prefix.bytes())
            {
                return Err("temporal_readback_changed".into());
            }
            Ok(token)
        }) {
            Ok(token) => token,
            Err(e) => {
                self.stage = Stage::BlockedDurability;
                return Err(e);
            }
        };
        self.commit_token = Some(token);
        self.pending_intent = None;
        self.pending_prefix = None;
        self.durable_windows = self.session.rows.len();
        self.durable = Some(std::sync::Arc::new(bytes));
        if slot {
            self.pending_window = None;
        }
        Ok(())
    }
    fn abort_pending_artifact(
        &mut self,
        store: &RecoveryStore,
        ticket: &TemporalAbortTicket,
        now_ns: u64,
    ) -> Result<(), String> {
        if self.stage != Stage::BlockedDurability
            || Some(ticket.expected_token()) != self.commit_token.as_ref()
            || now_ns < self.last_now_ns
        {
            return Err("temporal_abort_workflow_changed".into());
        }
        let ram = self.session.assembler.fence_checkpoint()?;
        let high = self.source_high.clone();
        let mapping = self.mapping.clone();
        let live = self.live_observed_ns;
        let loaded = store.abort_temporal_pending(ticket, now_ns)?;
        let prefix = loaded.prefix.ok_or("temporal_recovery_retired")?;
        let mut restored = checkpoint::decode(prefix.bytes(), prefix.descriptor(), now_ns)?;
        restored.session.assembler.restore_prefix_fences(
            ram.for_prefix(restored.session.rows.len())?,
            restored.session.rows.len(),
        )?;
        restored.source_high = high;
        restored.mapping = mapping;
        restored.live_observed_ns = live;
        restored.commit_token = Some(loaded.token);
        restored.durable = Some(std::sync::Arc::new(prefix.bytes().to_vec()));
        restored.stage = Stage::PausedNeedsReconfirmation;
        restored.last_now_ns = now_ns;
        *self = restored;
        Ok(())
    }
    fn repair_pending(&mut self, store: &RecoveryStore, now_ns: u64) -> Result<(), String> {
        if self.stage != Stage::BlockedDurability || now_ns < self.last_now_ns {
            return Err("temporal_pending_repair_state".into());
        }
        let intent = self
            .pending_intent
            .as_ref()
            .ok_or("temporal_pending_intent_missing")?;
        let prefix = self
            .pending_prefix
            .as_ref()
            .ok_or("temporal_pending_prefix_missing")?;
        let token = store.repair_same_temporal(intent, Some(prefix), now_ns)?;
        let mut restored = checkpoint::decode(prefix.bytes(), prefix.descriptor(), now_ns)?;
        restored.commit_token = Some(token);
        restored.durable = Some(std::sync::Arc::new(prefix.bytes().to_vec()));
        restored.stage = Stage::PausedNeedsReconfirmation;
        restored.last_now_ns = now_ns;
        *self = restored;
        Ok(())
    }
    fn cold_restore(
        store: &RecoveryStore,
        expected: &CaptureContext,
        now: u64,
        epoch: &str,
        anchor: u64,
    ) -> Result<Self, String> {
        Self::cold_restore_with(store, expected, now, epoch, anchor, |s, t, n| {
            s.recertify_temporal(t, n)
        })
    }
    fn cold_restore_with(
        store: &RecoveryStore,
        expected: &CaptureContext,
        now: u64,
        epoch: &str,
        anchor: u64,
        recertify: impl FnOnce(
            &RecoveryStore,
            &TemporalToken,
            u64,
        ) -> Result<
            crate::room_learner::recovery_store::temporal_commitment::TemporalRestored,
            String,
        >,
    ) -> Result<Self, String> {
        let loaded = store
            .load_temporal(now)?
            .ok_or("temporal_recovery_missing")?;
        if loaded.prefix.is_none() {
            return Err("temporal_recovery_retired".into());
        }
        // A read is only an authenticated observation. Cold restoration must
        // re-establish actual barriers before promising a durable saved prefix.
        let certified = recertify(store, &loaded.token, now)?;
        let prefix = certified.prefix;
        let mut v = checkpoint::decode(prefix.bytes(), prefix.descriptor(), now)?;
        v.commit_token = Some(certified.token);
        if v.context() != expected || epoch.is_empty() || anchor < v.last_now_ns || now < anchor {
            return Err("temporal_restore_context_or_clock".into());
        }
        v.stage = Stage::PausedNeedsReconfirmation;
        v.last_now_ns = now;
        v.durable = Some(std::sync::Arc::new(prefix.bytes().to_vec()));
        Ok(v)
    }
}
#[path = "temporal_checkpoint.rs"]
pub(crate) mod checkpoint;
#[cfg(test)]
#[path = "temporal_execution_tests.rs"]
mod tests;
