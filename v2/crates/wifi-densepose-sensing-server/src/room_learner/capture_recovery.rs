//! Typed reduced-sample recovery. Receiver buffers are deliberately never saved.
use super::*;
use std::collections::HashSet;
const SCHEMA: &str = "ruview.room-learner.capture-checkpoint.v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureCheckpoint {
    schema: String,
    context: CaptureContext,
    run_id: String,
    saved_at_ns: u64,
    last_seen_ns: u64,
    sources: Vec<SourceWatermark>,
    cohorts: Vec<CaptureCohort>,
    history: Vec<PhaseReceipt>,
    phase: Option<CheckpointPhase>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    metadata_lineage: Option<crate::room_learner::managed_training::TrainingProofSeed>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceWatermark {
    source_id: u8,
    sequence: Option<u32>,
    ordinal: u64,
    last_time_ns: u64,
    admitted_packets: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointPhase {
    request: PhaseRequest,
    started_ns: u64,
    bucket: usize,
    representatives: Vec<CaptureCohort>,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPhaseDisposition {
    NoActivePhase,
    CompletedNeedsConfirmation,
    InterruptedNeedsRestart,
}
#[derive(Debug)]
pub struct CaptureRestore {
    pub session: CaptureSession,
    pub disposition: RecoveryPhaseDisposition,
}
#[derive(Debug, Clone, Serialize)]
pub struct CaptureCheckpointSummary {
    pub run_id: String,
    pub room_digest: String,
    pub revision: u64,
    pub source_ids: Vec<u8>,
    pub sealed_cohorts: usize,
    pub phase: Option<RecoveryPhaseSummary>,
}
#[derive(Debug, Clone, Serialize)]
pub struct RecoveryPhaseSummary {
    pub role: DataRole,
    pub label: RoomLabel,
    pub complete: bool,
    pub representative_cohorts: usize,
    pub target_ms: u64,
    pub capture_elapsed_ms: u64,
}
fn phase_end(p: &CheckpointPhase) -> Result<u64, String> {
    if !p.request.conditions_confirmed
        || p.request.role == DataRole::Inference
        || p.request.label == RoomLabel::Unknown
        || !(MIN_PHASE_MS..=MAX_PHASE_MS).contains(&p.request.duration_ms)
        || p.request.lead_in_ms > 120_000
        || p.started_ns == 0
        || p.bucket >= COHORTS_PER_PHASE
    {
        return Err("invalid recovery phase request".into());
    }
    identifier(&p.request.trial_id)?;
    identifier(&p.request.session_id)?;
    p.started_ns
        .checked_add((p.request.duration_ms + p.request.lead_in_ms) * 1_000_000)
        .ok_or_else(|| "recovery phase time overflow".into())
}
impl CaptureCheckpoint {
    pub fn context(&self) -> &CaptureContext {
        &self.context
    }
    pub fn run_id(&self) -> &str {
        &self.run_id
    }
    pub fn saved_at_ns(&self) -> u64 {
        self.saved_at_ns
    }
    pub fn summary(&self) -> CaptureCheckpointSummary {
        CaptureCheckpointSummary {
            run_id: self.run_id.clone(),
            room_digest: self.context.room_digest.clone(),
            revision: self.context.revision,
            source_ids: self.context.sources.iter().map(|s| s.source_id).collect(),
            sealed_cohorts: self.cohorts.len(),
            phase: self.phase.as_ref().map(|p| RecoveryPhaseSummary {
                role: p.request.role,
                label: p.request.label,
                complete: phase_end(p).is_ok_and(|end| self.saved_at_ns >= end)
                    && p.representatives.len() == COHORTS_PER_PHASE,
                representative_cohorts: p.representatives.len(),
                target_ms: p.request.duration_ms,
                capture_elapsed_ms: self.saved_at_ns.saturating_sub(
                    p.started_ns
                        .saturating_add(p.request.lead_in_ms.saturating_mul(1_000_000)),
                ) / 1_000_000,
            }),
        }
    }
}
impl CaptureSession {
    pub fn checkpoint(&self, now_ns: u64) -> Result<CaptureCheckpoint, String> {
        let snapshot = CaptureCheckpoint {
            schema: if self.metadata_lineage.is_some() {"ruview.room-learner.capture-checkpoint.v2"} else {SCHEMA}.into(),
            context: self.context.clone(),
            run_id: self.run_id.clone(),
            saved_at_ns: now_ns,
            last_seen_ns: self.last_seen_ns,
            sources: self
                .context
                .sources
                .iter()
                .map(|s| {
                    let b = &self.sources[&s.source_id];
                    SourceWatermark {
                        source_id: s.source_id,
                        sequence: b.sequence,
                        ordinal: b.ordinal,
                        last_time_ns: b.last_time_ns,
                        admitted_packets: b.admitted_packets,
                    }
                })
                .collect(),
            cohorts: self.cohorts.clone(),
            history: self.history.clone(),
            metadata_lineage: self.metadata_lineage.clone(),
            phase: self.phase.as_ref().map(|p| CheckpointPhase {
                request: p.request.clone(),
                started_ns: p.started_ns,
                bucket: p.bucket,
                representatives: p.representatives.clone(),
            }),
        };
        // Apply the same validation to exports and imports; do not roundtrip
        // through serde_json::Value, which cannot retain arbitrary u128 IDs.
        snapshot.validate(&self.context, now_ns)?;
        Ok(snapshot)
    }
    pub fn restore_checkpoint(
        snapshot: CaptureCheckpoint,
        expected: &CaptureContext,
        now_ns: u64,
    ) -> Result<CaptureRestore, String> {
        snapshot.validate(expected, now_ns)?;
        let mut session = Self::new(snapshot.context.clone(), snapshot.run_id.clone())?;
        session.last_seen_ns = snapshot.last_seen_ns;
        session.cohorts = snapshot.cohorts;
        session.history = snapshot.history;
        session.metadata_lineage = snapshot.metadata_lineage;
        for receipt in &session.history {
            session
                .trials
                .insert(receipt.trial_id.clone(), receipt.role);
            session
                .sessions
                .insert(receipt.session_id.clone(), receipt.role);
        }
        for watermark in snapshot.sources {
            session.sources.insert(
                watermark.source_id,
                SourceBuffer {
                    sequence: watermark.sequence,
                    ordinal: watermark.ordinal,
                    last_time_ns: watermark.last_time_ns,
                    admitted_packets: watermark.admitted_packets,
                    live: VecDeque::new(),
                    ..SourceBuffer::default()
                },
            );
        }
        let disposition = if let Some(p) = snapshot.phase {
            session
                .trials
                .insert(p.request.trial_id.clone(), p.request.role);
            session
                .sessions
                .insert(p.request.session_id.clone(), p.request.role);
            let complete = snapshot.saved_at_ns >= phase_end(&p)?
                && p.representatives.len() == COHORTS_PER_PHASE;
            let phase = Phase {
                request: p.request,
                started_ns: p.started_ns,
                bucket: p.bucket,
                representatives: if complete { p.representatives } else { vec![] },
                pending: HashMap::new(),
                admitted: HashMap::new(),
            };
            if complete {
                session.phase = Some(phase);
                RecoveryPhaseDisposition::CompletedNeedsConfirmation
            } else {
                session.history.push(receipt(
                    &phase,
                    PhaseStatus::Failed,
                    Some(
                        "capture interrupted; restart required; downtime is not observation".into(),
                    ),
                ));
                RecoveryPhaseDisposition::InterruptedNeedsRestart
            }
        } else {
            RecoveryPhaseDisposition::NoActivePhase
        };
        Ok(CaptureRestore {
            session,
            disposition,
        })
    }
    /// Only semantic milestones trigger persistence; packet cadence and receiver
    /// counters cannot turn this into a raw-packet write path.
    pub fn recovery_marker(&self, now_ns: u64) -> String {
        let phase = self.phase.as_ref().map(|p| {
            let end = p
                .started_ns
                .saturating_add((p.request.duration_ms + p.request.lead_in_ms) * 1_000_000);
            (
                &p.request,
                p.started_ns,
                p.representatives.len(),
                now_ns >= end,
            )
        });
        let bytes = serde_json::to_vec(&(
            &self.context,
            &self.run_id,
            &self.history,
            self.cohorts.len(),
            phase,
        ))
        .unwrap_or_default();
        format!("{:x}", Sha256::digest(bytes))
    }
}
impl CaptureCheckpoint {
    fn validate(&self, expected: &CaptureContext, now_ns: u64) -> Result<(), String> {
        self.context.validate()?;
        if let Some(seed)=&self.metadata_lineage {seed.validate()?;}
        let lineage_offset=self.metadata_lineage.as_ref().map_or(0,|s|s.phase_claims.len());
        let expected_schema=if self.metadata_lineage.is_some() {"ruview.room-learner.capture-checkpoint.v2"} else {SCHEMA};
        if self.schema != expected_schema
            || &self.context != expected
            || self.saved_at_ns == 0
            || self.saved_at_ns > now_ns
            || self.last_seen_ns > self.saved_at_ns
            || self.sources.len() != self.context.sources.len()
            || self.history.len()+lineage_offset > MAX_PHASE_HISTORY
            || (self.phase.is_some() && self.history.len()+lineage_offset >= MAX_PHASE_HISTORY)
            || self.cohorts.len()+self.metadata_lineage.as_ref().map_or(0,|s|s.phase_claims.iter().map(|p|p.cohort_count).sum::<usize>()) > MAX_COHORTS
        {
            return Err("invalid recovery context, time or capacity".into());
        }
        let session = CaptureSession::new(self.context.clone(), self.run_id.clone())?;
        if let Some(seed)=&self.metadata_lineage {
            seed.validate()?;
            if seed.context!=self.context || seed.run_id!=self.run_id || seed.last_seen_ns>self.last_seen_ns {
                return Err("recovery managed lineage context/namespace mismatch".into());
            }
        }
        let mut latest = 0;
        for (w, spec) in self.sources.iter().zip(&self.context.sources) {
            if w.source_id != spec.source_id
                || w.ordinal >= (1u64 << 56)
                || w.last_time_ns > self.last_seen_ns
                || w.admitted_packets > w.ordinal
                || match w.sequence {
                    None => w.ordinal != 0 || w.last_time_ns != 0 || w.admitted_packets != 0,
                    Some(_) => w.ordinal == 0 || w.last_time_ns == 0 || w.admitted_packets == 0,
                }
            {
                return Err("invalid source recovery watermark".into());
            }
            if let Some(seed)=&self.metadata_lineage {
                let original=seed.sources.iter().find(|s|s.source_id==w.source_id).ok_or("missing original recovery source")?;
                if w.ordinal<original.ordinal || w.last_time_ns<original.last_time_ns || w.admitted_packets<original.admitted_packets
                    || w.sequence.map(|s|s.wrapping_sub(original.sequence))!=Some((w.ordinal-original.ordinal) as u32) {
                    return Err("recovery source counter regressed from managed training proof".into());
                }
            }
            latest = latest.max(w.last_time_ns);
        }
        if latest != self.last_seen_ns {
            return Err("recovery receiver watermark mismatch".into());
        }
        let mut trials = HashMap::new();
        let mut sessions = HashMap::new();
        let mut bind =
            |trial: &String, capture_session: &String, role: DataRole| -> Result<(), String> {
                identifier(trial)?;
                identifier(capture_session)?;
                if let Some(seed)=&self.metadata_lineage {seed.check_role(trial,capture_session,role)?;}
                if role == DataRole::Inference
                    || trials
                        .insert(trial.clone(), role)
                        .is_some_and(|r| r != role)
                    || sessions
                        .insert(capture_session.clone(), role)
                        .is_some_and(|r| r != role)
                {
                    return Err("recovery phase crosses frozen roles".into());
                }
                Ok(())
            };
        for r in &self.history {
            bind(&r.trial_id, &r.session_id, r.role)?;
            if r.label == RoomLabel::Unknown
                || r.cohort_count
                    != if r.status == PhaseStatus::Sealed {
                        COHORTS_PER_PHASE
                    } else {
                        0
                    }
                || r.reason
                    .as_ref()
                    .is_some_and(|s| s.len() > 512 || s.chars().any(char::is_control))
            {
                return Err("invalid recovery history".into());
            }
        }
        let mut all = self.cohorts.clone();
        if let Some(p) = &self.phase {
            phase_end(p)?;
            bind(&p.request.trial_id, &p.request.session_id, p.request.role)?;
            if p.started_ns > self.saved_at_ns
                || p.representatives.len() > COHORTS_PER_PHASE
                || all.len() + p.representatives.len() > MAX_COHORTS
            {
                return Err("invalid provisional recovery phase".into());
            }
            let mut previous_bucket = None;
            let start = p.started_ns + p.request.lead_in_ms * 1_000_000;
            let width = p.request.duration_ms * 1_000_000 / COHORTS_PER_PHASE as u64;
            for c in &p.representatives {
                let bucket = (0..COHORTS_PER_PHASE)
                    .find(|b| {
                        c.id == format!("{}-phase-{}-bucket-{b}", self.run_id, self.history.len()+lineage_offset)
                    })
                    .ok_or("invalid provisional cohort identity")?;
                if previous_bucket.is_some_and(|b| b >= bucket)
                    || bucket > p.bucket
                    || c.role != p.request.role
                    || c.label != p.request.label
                    || c.trial_id != p.request.trial_id
                    || c.session_id != p.request.session_id
                    || c.views.iter().any(|v| {
                        v.first_time_ns < start + bucket as u64 * width
                            || v.last_time_ns >= start + (bucket as u64 + 1) * width
                    })
                {
                    return Err("invalid provisional phase representative".into());
                }
                previous_bucket = Some(bucket);
            }
            all.extend(p.representatives.clone());
        }
        let mut expected_cohorts = HashSet::new();
        for (index, r) in self
            .history
            .iter()
            .enumerate()
            .filter(|(_, r)| r.status == PhaseStatus::Sealed)
        {
            for bucket in 0..COHORTS_PER_PHASE {
                let index=index+lineage_offset;
                let id = format!("{}-phase-{index}-bucket-{bucket}", self.run_id);
                let c = self
                    .cohorts
                    .iter()
                    .find(|c| c.id == id)
                    .ok_or("missing sealed recovery cohort")?;
                if c.role != r.role
                    || c.label != r.label
                    || c.trial_id != r.trial_id
                    || c.session_id != r.session_id
                {
                    return Err("sealed recovery history mismatch".into());
                }
                expected_cohorts.insert(id);
            }
        }
        if expected_cohorts.len() != self.cohorts.len() {
            return Err("unrecorded sealed recovery cohort".into());
        }
        let mut frames = HashSet::new();
        for c in &all {
            if c.provenance != CaptureProvenance::LiveReceiverObserved || c.teacher.is_some()
                || (self.metadata_lineage.is_some() && c.role==DataRole::Train) {
                return Err("recovery requires original managed capture provenance".into());
            }
            for v in &c.views {
                let w = self
                    .sources
                    .iter()
                    .find(|w| w.source_id == v.source_id)
                    .ok_or("unknown recovery source")?;
                if v.samples.len() != WINDOW_PACKETS
                    || v.last_time_ns > w.last_time_ns
                    || v.clock.kind != ClockKind::ReceiverOnly
                    || v.clock.uncertainty_ns != u64::MAX
                    || v.phase != PhaseAvailability::MagnitudeOnly
                {
                    return Err("invalid reduced recovery source window".into());
                }
                if let Some(seed)=&self.metadata_lineage {
                    let old=seed.sources.iter().find(|s|s.source_id==v.source_id).ok_or("unknown original source")?;
                    if v.first_time_ns<=old.last_time_ns || v.sequence_ids.iter().any(|n|*n<=old.ordinal) {
                        return Err("recovered cohort overlaps original managed Train capture".into());
                    }
                }
                for (frame, ordinal) in v.frame_ids.iter().zip(&v.sequence_ids) {
                    let expected = (u128::from(session.prefix) << 64)
                        | (u128::from(v.source_id) << 56)
                        | u128::from(*ordinal);
                    if *ordinal == 0
                        || *ordinal > w.ordinal
                        || *frame != expected
                        || !frames.insert(*frame)
                    {
                        return Err("reused or invalid recovery frame/sequence watermark".into());
                    }
                }
            }
        }
        if !all.is_empty() {
            ValidatedDataset::new(self.context.clone(), all)?;
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "capture_recovery_tests.rs"]
mod tests;
