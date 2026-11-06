//! Default-inactive, bounded continuous INPUT diagnostic, not a runtime/selected
//! detector. Every window uses the same TemporalModel opaque builder/infer path.
use super::{
    model::RoomPrediction,
    native::CanonicalPacket,
    observation::MAX_AGE_NS,
    temporal_acquisition::{AcquisitionPlan, WindowAdmission},
    temporal_model_input::{TemporalLiveInput, TemporalModel},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LiveState {
    Inactive,
    Warming,
    Current,
    Withdrawn,
}
#[derive(Debug, Clone, Serialize)]
pub struct WindowTiming {
    pub ordinal: u64,
    pub target_ns: String,
    pub completed_ns: String,
    pub receiver_latency_ns: String,
    pub min_observed_span_ns: String,
    pub max_observed_span_ns: String,
    pub interval_since_previous_ns: Option<String>,
    pub min_source_rows: usize,
    pub max_source_rows: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct LiveDiagnostic {
    pub state: LiveState,
    pub reason: Option<&'static str>,
    pub model_sha256: String,
    pub policy_sha256: String,
    pub target_horizon_ms: u64,
    pub cadence: &'static str,
    pub ready_sources: usize,
    pub required_sources: usize,
    pub current_prediction: Option<RoomPrediction>,
    pub last_completed: Option<WindowTiming>,
    pub diagnostic_only: bool,
    pub physically_validated: bool,
    pub adoption_authorized: bool,
}
struct Fence {
    sequence: u32,
    time_ns: u64,
    frame_id: u128,
}
#[derive(Default)]
struct WindowCount {
    first_ns: u64,
    last_ns: u64,
    rows: usize,
}
/// No data-set retention, training schedule cursor, new persistence or clocks.
/// At most one <=4s opaque buffer, <=8 source counters/fences and one aggregate
/// completion are retained. Error withdrawal preserves accepted replay fences.
pub struct ContinuousTemporalInput {
    model: TemporalModel,
    policy: AcquisitionPlan,
    status: LiveDiagnostic,
    active: bool,
    input: Option<TemporalLiveInput>,
    target_ns: Option<u64>,
    end_ns: Option<u64>,
    ordinal: u64,
    fences: HashMap<u8, Fence>,
    continuity: HashMap<u8, u64>,
    counts: HashMap<u8, WindowCount>,
    last_now: Option<u64>,
}
impl ContinuousTemporalInput {
    pub fn new(model: TemporalModel) -> Result<Self, String> {
        let policy = model.input_policy()?;
        let bytes = model.export()?;
        let status = LiveDiagnostic {
            state: LiveState::Inactive,
            reason: Some("inactive"),
            model_sha256: format!("{:x}", Sha256::digest(bytes)),
            policy_sha256: model.plan_sha256().into(),
            target_horizon_ms: policy.target_span_ms(),
            cadence: "nonoverlap_full_native_horizon",
            ready_sources: 0,
            required_sources: policy.context().sources.len(),
            current_prediction: None,
            last_completed: None,
            diagnostic_only: true,
            physically_validated: false,
            adoption_authorized: false,
        };
        Ok(Self {
            model,
            policy,
            status,
            active: false,
            input: None,
            target_ns: None,
            end_ns: None,
            ordinal: 0,
            fences: HashMap::new(),
            continuity: HashMap::new(),
            counts: HashMap::new(),
            last_now: None,
        })
    }
    pub fn activate(&mut self) {
        self.active = true;
        self.withdraw("collecting_full_horizon");
        self.status.state = LiveState::Warming;
    }
    pub fn deactivate(&mut self) {
        self.active = false;
        self.withdraw("inactive");
        self.status.state = LiveState::Inactive;
    }
    fn withdraw(&mut self, reason: &'static str) {
        self.input = None;
        self.target_ns = None;
        self.end_ns = None;
        self.counts.clear();
        self.continuity.clear();
        self.status.current_prediction = None;
        self.status.ready_sources = 0;
        self.status.state = if self.active {
            LiveState::Withdrawn
        } else {
            LiveState::Inactive
        };
        self.status.reason = Some(reason);
    }
    /// Explicit receiver-domain tick; a silent source cannot leave a last result
    /// looking current. No missing input or failure is converted into absence.
    pub fn poll(&mut self, now_ns: u64) -> LiveDiagnostic {
        if !self.active {
            return self.status.clone();
        }
        if self.last_now.is_some_and(|t| now_ns < t) {
            self.withdraw("receiver_clock_reversed");
            return self.status.clone();
        }
        self.last_now = Some(now_ns);
        if self.continuity.iter().any(|(id, last)| {
            self.policy
                .source_policies()
                .iter()
                .find(|p| p.source_id == *id)
                .is_none_or(|p| now_ns.saturating_sub(*last) > p.max_gap_ns)
        }) {
            self.withdraw("source_gap");
        } else if self.target_ns.is_some_and(|target| {
            self.counts.len() < self.policy.context().sources.len()
                && now_ns.saturating_sub(target)
                    > self.policy.max_first_packet_delay_ms() * 1_000_000
        }) {
            self.withdraw("missing_source_warmup");
        } else if self.end_ns.is_some_and(|end| now_ns >= end) {
            self.withdraw("incomplete_horizon");
        }
        self.status.clone()
    }
    pub fn ingest(&mut self, p: &CanonicalPacket, now_ns: u64) -> LiveDiagnostic {
        if !self.active {
            return self.status.clone();
        }
        if self.last_now.is_some_and(|t| now_ns < t) {
            self.withdraw("receiver_clock_reversed");
            return self.status.clone();
        }
        self.last_now = Some(now_ns);
        let time = p.native_frame.timestamp_ns;
        if time > now_ns || now_ns - time > MAX_AGE_NS {
            self.withdraw("stale_or_future_packet");
            return self.status.clone();
        }
        let Some(rule) = self
            .policy
            .source_policies()
            .iter()
            .find(|s| s.source_id == p.source_id)
        else {
            self.withdraw("unbound_source");
            return self.status.clone();
        };
        let max_gap = rule.max_gap_ns;
        if self.fences.get(&p.source_id).is_some_and(|f| {
            p.sequence <= f.sequence || time <= f.time_ns || p.native_frame.frame_id == f.frame_id
        }) {
            self.withdraw("replayed_or_reversed_packet");
            return self.status.clone();
        }
        if self
            .continuity
            .get(&p.source_id)
            .is_some_and(|last| time.saturating_sub(*last) > max_gap)
        {
            self.withdraw("source_gap");
            return self.status.clone();
        }
        // Also check every OTHER source against the receiver tick. A busy source
        // cannot hide a dropout while continuing to fill its own window.
        if self.continuity.iter().any(|(id, last)| {
            self.policy
                .source_policies()
                .iter()
                .find(|s| s.source_id == *id)
                .is_none_or(|s| now_ns.saturating_sub(*last) > s.max_gap_ns)
        }) {
            self.withdraw("source_gap");
            return self.status.clone();
        }
        if self.input.is_none() {
            let end = match time.checked_add(
                self.policy.max_observed_span_ms() * 1_000_000
                    + self.policy.max_first_packet_delay_ms() * 1_000_000
                    + 1,
            ) {
                Some(end) => end,
                None => {
                    self.withdraw("receiver_time_overflow");
                    return self.status.clone();
                }
            };
            let mut input = match self
                .model
                .live_input(format!("continuous-live-{}", self.ordinal))
            {
                Ok(input) => input,
                Err(_) => {
                    self.withdraw("model_input_unavailable");
                    return self.status.clone();
                }
            };
            if input.begin_window(time, end).is_err() {
                self.withdraw("window_start_refused");
                return self.status.clone();
            }
            self.input = Some(input);
            self.target_ns = Some(time);
            self.end_ns = Some(end);
            self.counts.clear();
        }
        self.status.state = LiveState::Warming;
        self.status.reason = Some("collecting_full_horizon");
        self.status.current_prediction = None;
        let admission = match self.input.as_mut().unwrap().ingest(p) {
            Ok(v) => v,
            Err(_) => {
                self.withdraw("source_or_window_invalid");
                return self.status.clone();
            }
        };
        self.fences.insert(
            p.source_id,
            Fence {
                sequence: p.sequence,
                time_ns: time,
                frame_id: p.native_frame.frame_id,
            },
        );
        self.continuity.insert(p.source_id, time);
        let count = self.counts.entry(p.source_id).or_default();
        if count.rows == 0 {
            count.first_ns = time;
        }
        count.last_ns = time;
        count.rows += 1;
        self.status.ready_sources = self.counts.len();
        if admission == WindowAdmission::Ready {
            let target = self.target_ns.unwrap();
            let input = self.input.take().unwrap();
            let observation = match input.finish(now_ns) {
                Ok(v) => v,
                Err(_) => {
                    self.withdraw("window_evidence_invalid");
                    return self.status.clone();
                }
            };
            let prediction = match self.model.predict(observation, now_ns) {
                Ok(v) => v,
                Err(_) => {
                    self.withdraw("prediction_input_refused");
                    return self.status.clone();
                }
            };
            if prediction.physically_validated {
                self.withdraw("unexpected_authority");
                return self.status.clone();
            }
            let timing = WindowTiming {
                ordinal: self.ordinal,
                target_ns: target.to_string(),
                completed_ns: now_ns.to_string(),
                receiver_latency_ns: (now_ns - target).to_string(),
                min_observed_span_ns: self
                    .counts
                    .values()
                    .map(|c| c.last_ns - c.first_ns)
                    .min()
                    .unwrap()
                    .to_string(),
                max_observed_span_ns: self
                    .counts
                    .values()
                    .map(|c| c.last_ns - c.first_ns)
                    .max()
                    .unwrap()
                    .to_string(),
                interval_since_previous_ns: self
                    .status
                    .last_completed
                    .as_ref()
                    .and_then(|w| w.completed_ns.parse::<u64>().ok())
                    .map(|last| now_ns.saturating_sub(last).to_string()),
                min_source_rows: self.counts.values().map(|c| c.rows).min().unwrap(),
                max_source_rows: self.counts.values().map(|c| c.rows).max().unwrap(),
            };
            let Some(next) = self.ordinal.checked_add(1) else {
                self.withdraw("window_counter_exhausted");
                return self.status.clone();
            };
            self.ordinal = next;
            self.target_ns = None;
            self.end_ns = None;
            self.counts.clear();
            self.status.state = LiveState::Current;
            self.status.reason = None;
            self.status.current_prediction = Some(prediction);
            self.status.last_completed = Some(timing);
        }
        self.status.clone()
    }
}
