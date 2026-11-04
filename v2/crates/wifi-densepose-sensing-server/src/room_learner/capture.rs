//! Bounded receiver-observed capture. Operator phase labels are experimental
//! claims, never independently verified empty-room or source-clock evidence.
use super::observation::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};

pub const WINDOW_PACKETS: usize = 8;
pub const COHORTS_PER_PHASE: usize = 4;
pub const MIN_PHASE_MS: u64 = 12_000;
pub const MAX_PHASE_MS: u64 = 600_000;
pub const MAX_CAPTURE_BINS: usize = 128;
const MAX_PHASE_HISTORY: usize = 128;
// Protected diagnostics are consumed by JavaScript clients as exact integers.
const MAX_SEQUENCE_DIAGNOSTIC_COUNT: u64 = (1u64 << 53) - 1;

/// Borrowed actual packet metadata. The caller must reject unknown bandwidth
/// and retain the known raw grid identity. Amplitudes may be length-only
/// interpolated to the declared feature_count; no gain/mean normalization.
/// This adapter never guesses PHY or original bin count from feature length.
pub struct RawSourceFrame<'a> {
    pub source_id: u8,
    pub grid_id: &'a str,
    pub frequency_hz: u64,
    pub bandwidth_hz: u64,
    pub sequence: u32,
    pub amplitudes: &'a [f64],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseRequest {
    pub trial_id: String,
    pub session_id: String,
    pub role: DataRole,
    pub label: RoomLabel,
    pub duration_ms: u64,
    #[serde(default)]
    pub lead_in_ms: u64,
    pub conditions_confirmed: bool,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PhaseStatus { Sealed, Cancelled, Failed }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseReceipt {
    pub trial_id: String,
    pub session_id: String,
    pub role: DataRole,
    pub label: RoomLabel,
    pub status: PhaseStatus,
    pub cohort_count: usize,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
struct Packet { values: Vec<f32>, frame_id: u128, ordinal: u64, received_ns: u64 }
#[derive(Debug, Default)]
struct SourceBuffer {
    sequence: Option<u32>,
    ordinal: u64,
    last_time_ns: u64,
    admitted_packets: u64,
    // Transient diagnostics only. Never restore observations/rejections as
    // fresh input, and never let them advance the admission watermark.
    last_observed_sequence: Option<u32>,
    sequence_rejection_count: u64,
    sequence_rejection_reason: Option<SequenceRejectionReason>,
    live: VecDeque<Packet>,
}
#[derive(Debug)]
struct Phase {
    request: PhaseRequest,
    started_ns: u64,
    bucket: usize,
    pending: HashMap<u8, VecDeque<Packet>>,
    representatives: Vec<CaptureCohort>,
    admitted: HashMap<u8, u64>,
}

/// A bounded diagnostic of the most recent sequence admission failure.
/// It does not establish a source reboot or authenticated source epoch.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SequenceRejectionReason { Duplicate, Reversed }

/// Counts and receiver freshness only. No learned/qualified state is implied.
#[derive(Debug, Clone, Serialize)]
pub struct CaptureProgress {
    pub phase: Option<PhaseProgress>,
    pub sources: Vec<SourceProgress>,
    pub sealed_cohorts: usize,
    pub last_failure: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PhaseProgress {
    pub trial_id: String,
    pub session_id: String,
    pub role: DataRole,
    pub label: RoomLabel,
    pub stage: &'static str,
    pub lead_in_remaining_ms: u64,
    pub capture_elapsed_ms: u64,
    pub elapsed_ms: u64,
    pub target_ms: u64,
    pub representative_cohorts: usize,
    pub required_cohorts: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct SourceProgress {
    pub source_id: u8,
    pub grid_id: String,
    pub admitted_packets: u64,
    /// Last admitted wire sequence, including an original recovery watermark.
    pub last_accepted_sequence: Option<u32>,
    /// Last sequence tested by this receiver after PHY/shape/time validation.
    pub last_observed_sequence: Option<u32>,
    /// Transient since this capture was created/restored; never persisted.
    pub sequence_rejection_count: u64,
    pub sequence_rejection_reason: Option<SequenceRejectionReason>,
    pub phase_admitted_packets: u64,
    pub live_buffer_frames: usize,
    pub phase_buffer_frames: usize,
    pub required_frames: usize,
    pub latest_age_ms: Option<u64>,
    pub window_eligible: bool,
    pub reason: Option<String>,
}

#[derive(Debug)]
pub struct CaptureSession {
    context: CaptureContext,
    run_id: String,
    prefix: u64,
    last_seen_ns: u64,
    sources: HashMap<u8, SourceBuffer>,
    phase: Option<Phase>,
    cohorts: Vec<CaptureCohort>,
    history: Vec<PhaseReceipt>,
    trials: HashMap<String, DataRole>,
    sessions: HashMap<String, DataRole>,
}

impl CaptureSession {
    pub fn new(context: CaptureContext, run_id: String) -> Result<Self, String> {
        context.validate()?;
        identifier(&run_id)?;
        if context.sources.iter().any(|s| s.feature_count > MAX_CAPTURE_BINS) {
            return Err("capture supports at most 128 declared feature bins per source".into());
        }
        let hash = Sha256::digest(format!("{}:{run_id}", context.digest()).as_bytes());
        let prefix = u64::from_be_bytes(hash[..8].try_into().map_err(|_| "run digest")?);
        let sources = context.sources.iter().map(|s| (s.source_id, SourceBuffer::default())).collect();
        Ok(Self { context, run_id, prefix, last_seen_ns: 0, sources, phase: None, cohorts: vec![], history: vec![],
            trials: HashMap::new(), sessions: HashMap::new() })
    }

    /// A rejected selected input withdraws old live windows, not sealed training rows.
    pub(crate) fn clear_live_windows(&mut self) { for source in self.sources.values_mut() { source.live.clear(); } }
    pub fn context(&self) -> &CaptureContext { &self.context }
    pub fn cohorts(&self) -> &[CaptureCohort] { &self.cohorts }
    pub fn history(&self) -> &[PhaseReceipt] { &self.history }
    pub fn phase_active(&self) -> bool { self.phase.is_some() }
    pub fn validated_dataset(&self) -> Result<ValidatedDataset, String> {
        ValidatedDataset::new(self.context.clone(), self.cohorts.clone())
    }

    pub fn progress(&self, now_ns: u64) -> CaptureProgress {
        let phase = self.phase.as_ref().map(|p| PhaseProgress {
            trial_id: p.request.trial_id.clone(), session_id: p.request.session_id.clone(),
            role: p.request.role, label: p.request.label,
            stage: if now_ns < p.started_ns + p.request.lead_in_ms * 1_000_000 { "lead_in" } else { "collecting" },
            lead_in_remaining_ms: (p.started_ns + p.request.lead_in_ms * 1_000_000).saturating_sub(now_ns) / 1_000_000,
            capture_elapsed_ms: now_ns.saturating_sub(p.started_ns + p.request.lead_in_ms * 1_000_000) / 1_000_000,
            elapsed_ms: now_ns.saturating_sub(p.started_ns + p.request.lead_in_ms * 1_000_000) / 1_000_000,
            target_ms: p.request.duration_ms, representative_cohorts: p.representatives.len(),
            required_cohorts: COHORTS_PER_PHASE,
        });
        let sources = self.context.sources.iter().map(|spec| {
            let source = &self.sources[&spec.source_id];
            let latest_age = source.live.back().and_then(|p| now_ns.checked_sub(p.received_ns));
            let oldest_age = source.live.front().and_then(|p| now_ns.checked_sub(p.received_ns));
            let reason = if source.live.is_empty() { Some("missing_source") }
                else if latest_age.is_none() || oldest_age.is_none() { Some("receiver_clock_regressed") }
                else if oldest_age.is_some_and(|age| age > MAX_AGE_NS) { Some("stale_source") }
                else if source.live.len() != WINDOW_PACKETS { Some("incomplete_window") } else { None };
            SourceProgress { source_id: spec.source_id, grid_id: spec.grid_id.clone(),
                admitted_packets: source.admitted_packets,
                last_accepted_sequence: source.sequence,
                last_observed_sequence: source.last_observed_sequence,
                sequence_rejection_count: source.sequence_rejection_count,
                sequence_rejection_reason: source.sequence_rejection_reason,
                phase_admitted_packets: self.phase.as_ref().and_then(|p| p.admitted.get(&spec.source_id)).copied().unwrap_or(0),
                live_buffer_frames: source.live.len(),
                phase_buffer_frames: self.phase.as_ref().and_then(|p| p.pending.get(&spec.source_id)).map_or(0, VecDeque::len),
                required_frames: WINDOW_PACKETS, latest_age_ms: latest_age.map(|age| age / 1_000_000),
                window_eligible: reason.is_none(), reason: reason.map(str::to_string) }
        }).collect();
        CaptureProgress { phase, sources, sealed_cohorts: self.cohorts.len(),
            last_failure: self.history.iter().rev().find(|r| r.status == PhaseStatus::Failed).and_then(|r| r.reason.clone()) }
    }

    pub fn begin_phase(&mut self, request: PhaseRequest, now_ns: u64) -> Result<(), String> {
        identifier(&request.trial_id)?;
        identifier(&request.session_id)?;
        if self.phase.is_some() { return Err("a phase is already active".into()); }
        if !request.conditions_confirmed || request.role == DataRole::Inference || request.label == RoomLabel::Unknown
            || !(MIN_PHASE_MS..=MAX_PHASE_MS).contains(&request.duration_ms) || request.lead_in_ms > 120_000 || now_ns == 0
            || now_ns < self.last_seen_ns || now_ns.checked_add((request.duration_ms + request.lead_in_ms) * 1_000_000).is_none()
            || self.history.len() >= MAX_PHASE_HISTORY || self.cohorts.len() + COHORTS_PER_PHASE > MAX_COHORTS {
            return Err("phase requires confirmed labelled conditions, bounded duration and capacity".into());
        }
        for (roles, key) in [(&self.trials, &request.trial_id), (&self.sessions, &request.session_id)] {
            if roles.get(key).is_some_and(|role| *role != request.role) {
                return Err("trial/session role is frozen".into());
            }
        }
        self.trials.insert(request.trial_id.clone(), request.role);
        self.sessions.insert(request.session_id.clone(), request.role);
        self.phase = Some(Phase { request, started_ns: now_ns, bucket: 0, pending: HashMap::new(), representatives: vec![], admitted: HashMap::new() });
        Ok(())
    }

    pub fn ingest(&mut self, frame: RawSourceFrame<'_>, now_ns: u64) -> Result<(), String> {
        let spec = match self.context.sources.iter().find(|s| s.source_id == frame.source_id) {
            Some(s) => s,
            None => return self.invalidate("unknown source changed capture context"),
        };
        if frame.grid_id != spec.grid_id || frame.frequency_hz != spec.center_frequency_hz
            || frame.bandwidth_hz != spec.bandwidth_hz || frame.amplitudes.len() != spec.feature_count
            || frame.amplitudes.iter().any(|x| !x.is_finite() || *x < 0.0 || *x > f32::MAX as f64) {
            return self.invalidate("source PHY/grid/shape or finite magnitude contract changed");
        }
        let source = self.sources.get_mut(&frame.source_id).ok_or("source buffer unavailable")?;
        if now_ns == 0 || now_ns < self.last_seen_ns || now_ns <= source.last_time_ns { return Err("nonmonotonic receiver time".into()); }
        source.last_observed_sequence = Some(frame.sequence);
        let delta = match source.sequence {
            Some(last) => {
                let delta = frame.sequence.wrapping_sub(last);
                if delta == 0 || delta >= (1u32 << 31) {
                    source.sequence_rejection_count = source.sequence_rejection_count.saturating_add(1).min(MAX_SEQUENCE_DIAGNOSTIC_COUNT);
                    source.sequence_rejection_reason = Some(if delta == 0 {
                        SequenceRejectionReason::Duplicate
                    } else { SequenceRejectionReason::Reversed });
                    return Err("replayed or reversed source sequence".into());
                }
                u64::from(delta)
            }
            None => 1,
        };
        let ordinal = source.ordinal.checked_add(delta).filter(|n| *n < (1u64 << 56)).ok_or("sequence ordinal exhausted")?;
        let packet = Packet { values: frame.amplitudes.iter().map(|x| *x as f32).collect(),
            frame_id: (u128::from(self.prefix) << 64) | (u128::from(frame.source_id) << 56) | u128::from(ordinal),
            ordinal, received_ns: now_ns };
        source.sequence_rejection_reason = None;
        source.sequence = Some(frame.sequence);
        source.ordinal = ordinal;
        source.last_time_ns = now_ns;
        source.admitted_packets = source.admitted_packets.saturating_add(1);
        self.last_seen_ns = now_ns;
        push_bounded(&mut source.live, packet.clone());
        let Some(phase) = self.phase.as_mut() else { return Ok(()); };
        if now_ns < phase.started_ns { return self.invalidate("receiver time precedes phase start"); }
        let capture_start = phase.started_ns + phase.request.lead_in_ms * 1_000_000;
        if now_ns < capture_start { return Ok(()); }
        let elapsed = now_ns - capture_start;
        let duration_ns = phase.request.duration_ms * 1_000_000;
        if elapsed >= duration_ns { return Ok(()); }
        *phase.admitted.entry(frame.source_id).or_default() += 1;
        let bucket = ((elapsed * COHORTS_PER_PHASE as u64) / duration_ns) as usize;
        if bucket != phase.bucket { phase.pending.clear(); phase.bucket = bucket; }
        // Exactly one complete nonoverlapping representative per quarter.
        if phase.representatives.iter().any(|c| c.id.ends_with(&format!("-bucket-{bucket}"))) { return Ok(()); }
        push_bounded(phase.pending.entry(frame.source_id).or_default(), packet);
        if phase.pending.len() != self.context.sources.len()
            || phase.pending.values().any(|v| v.len() != WINDOW_PACKETS) { return Ok(()); }
        let id = format!("{}-phase-{}-bucket-{bucket}", self.run_id, self.history.len());
        // Disjoint streams remain incomplete until a complete aligned window
        // arrives. No pooling, padding, or fabricated source-clock alignment.
        if let Ok(cohort) = assemble(&self.context, &phase.pending, now_ns, id, &phase.request) {
            phase.representatives.push(cohort); phase.pending.clear();
        }
        Ok(())
    }

    pub fn finish_phase(&mut self, now_ns: u64) -> Result<PhaseReceipt, String> {
        let phase = self.phase.as_ref().ok_or("no active capture phase")?;
        if now_ns < self.last_seen_ns || now_ns < phase.started_ns || now_ns - phase.started_ns < (phase.request.duration_ms + phase.request.lead_in_ms) * 1_000_000 {
            return Err("phase duration has not elapsed".into());
        }
        if phase.representatives.len() != COHORTS_PER_PHASE {
            return self.invalidate("phase lacks four complete aligned representative windows");
        }
        let phase = self.phase.take().ok_or("no active capture phase")?;
        let receipt = receipt(&phase, PhaseStatus::Sealed, None);
        let mut combined = self.cohorts.clone();
        combined.extend(phase.representatives);
        if let Err(error) = ValidatedDataset::new(self.context.clone(), combined.clone()) {
            let failed = PhaseReceipt { status: PhaseStatus::Failed, cohort_count: 0, reason: Some(error.clone()), ..receipt };
            self.history.push(failed);
            return Err(error);
        }
        self.cohorts = combined;
        self.history.push(receipt.clone());
        Ok(receipt)
    }

    pub fn cancel_phase(&mut self) {
        if let Some(phase) = self.phase.take() {
            self.history.push(receipt(&phase, PhaseStatus::Cancelled, Some("operator cancelled phase".into())));
        }
    }

    pub fn live_cohort(&self, now_ns: u64) -> Result<CaptureCohort, String> {
        let buffers = self.sources.iter().map(|(id,s)| (*id,s.live.clone())).collect();
        let request = PhaseRequest { trial_id: "live-unlabelled".into(), session_id: self.run_id.clone(),
            role: DataRole::Inference, label: RoomLabel::Unknown, duration_ms: MIN_PHASE_MS, lead_in_ms: 0, conditions_confirmed: false };
        assemble(&self.context, &buffers, now_ns, format!("{}-live-{now_ns}", self.run_id), &request)
    }

    fn invalidate<T>(&mut self, reason: &str) -> Result<T, String> {
        if let Some(phase) = self.phase.take() {
            self.history.push(receipt(&phase, PhaseStatus::Failed, Some(reason.into())));
        }
        for source in self.sources.values_mut() { source.live.clear(); }
        Err(reason.into())
    }
}

fn assemble(context: &CaptureContext, buffers: &HashMap<u8, VecDeque<Packet>>, now_ns: u64,
    id: String, request: &PhaseRequest) -> Result<CaptureCohort, String> {
    let mut views = Vec::with_capacity(context.sources.len());
    for spec in &context.sources {
        let rows = buffers.get(&spec.source_id).filter(|v| v.len() == WINDOW_PACKETS).ok_or("incomplete source window")?;
        let first = rows.front().ok_or("empty source window")?.received_ns;
        let last = rows.back().ok_or("empty source window")?.received_ns;
        let age = now_ns.checked_sub(first).filter(|age| *age <= MAX_AGE_NS).ok_or("stale or future source window")?;
        views.push(SourceView { source_id: spec.source_id, device_id: spec.device_id.clone(), grid_id: spec.grid_id.clone(),
            signal_kind: spec.signal_kind, center_frequency_hz: spec.center_frequency_hz, bandwidth_hz: spec.bandwidth_hz,
            link_id: spec.link_id.clone(), link_index: spec.link_index, geometry: spec.geometry,
            samples: rows.iter().map(|p| p.values.clone()).collect(), frame_ids: rows.iter().map(|p| p.frame_id).collect(),
            sequence_ids: rows.iter().map(|p| p.ordinal).collect(), first_time_ns: first, last_time_ns: last,
            clock: ClockInfo { kind: ClockKind::ReceiverOnly, uncertainty_ns: u64::MAX }, sample_age_ns: age,
            phase: PhaseAvailability::MagnitudeOnly, provenance: CaptureProvenance::LiveReceiverObserved });
    }
    let cohort = CaptureCohort { id, context_digest: context.digest(), trial_id: request.trial_id.clone(),
        session_id: request.session_id.clone(), role: request.role, label: request.label, views, teacher: None,
        provenance: CaptureProvenance::LiveReceiverObserved };
    validate_cohort(context, &cohort)?;
    Ok(cohort)
}
fn push_bounded(buffer: &mut VecDeque<Packet>, packet: Packet) {
    if buffer.len() == WINDOW_PACKETS { buffer.pop_front(); }
    buffer.push_back(packet);
}
fn receipt(phase: &Phase, status: PhaseStatus, reason: Option<String>) -> PhaseReceipt {
    PhaseReceipt { trial_id: phase.request.trial_id.clone(), session_id: phase.request.session_id.clone(),
        role: phase.request.role, label: phase.request.label, status,
        cohort_count: if status == PhaseStatus::Sealed { phase.representatives.len() } else { 0 }, reason }
}
fn identifier(s: &str) -> Result<(), String> {
    if s.is_empty() || s.len() > 128 || !s.is_ascii() || s.trim() != s || s.chars().any(char::is_control) {
        Err("invalid bounded capture identifier".into())
    } else { Ok(()) }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;

#[path = "capture_recovery.rs"]
mod recovery;
pub use recovery::{CaptureCheckpoint, CaptureCheckpointSummary, CaptureRestore, RecoveryPhaseDisposition, RecoveryPhaseSummary};
