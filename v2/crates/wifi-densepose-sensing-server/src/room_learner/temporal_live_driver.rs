//! Default-inactive, bounded continuous INPUT diagnostic, not a runtime/selected
//! detector. Every window uses the same TemporalModel opaque builder/infer path.
use super::{
    model::RoomPrediction,
    native::CanonicalPacket,
    observation::MAX_AGE_NS,
    temporal_acquisition::{WindowInputPolicy, WindowAdmission},
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
    WindowComplete,
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
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservationAvailability {
    Unavailable,
    Available,
}
#[derive(Debug, Clone, Serialize)]
pub struct ObservationStatus {
    pub availability: ObservationAvailability,
    pub reason: Option<&'static str>,
    pub digest: Option<String>,
    pub first_receiver_ns: Option<String>,
    pub completed_ns: Option<String>,
    pub expires_ns: Option<String>,
    pub age_ns: Option<String>,
    pub source_count: usize,
}
impl ObservationStatus {
    fn unavailable(reason: &'static str) -> Self {
        Self {
            availability: ObservationAvailability::Unavailable,
            reason: Some(reason),
            digest: None,
            first_receiver_ns: None,
            completed_ns: None,
            expires_ns: None,
            age_ns: None,
            source_count: 0,
        }
    }
}
#[derive(Clone, Serialize)]
struct SourceBounds {
    source_id: u8,
    first_ns: u64,
    last_ns: u64,
}
struct CompletedObservation {
    prediction: RoomPrediction,
    model_sha256: String,
    policy_sha256: String,
    context_digest: String,
    source_bounds: Vec<SourceBounds>,
    first_ns: u64,
    completed_ns: u64,
    expires_ns: u64,
    digest: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct LiveDiagnostic {
    /// Buffer progress only; observation availability is separate.
    pub buffer_state: LiveState,
    pub reason: Option<&'static str>,
    pub model_sha256: String,
    pub policy_sha256: String,
    pub target_horizon_ms: u64,
    pub cadence: &'static str,
    pub buffer_sources_with_rows: usize,
    pub required_sources: usize,
    pub observation: ObservationStatus,
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
    policy: WindowInputPolicy,
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
    observation: Option<CompletedObservation>,
    #[cfg(test)] last_input_error:Option<String>,
}
impl ContinuousTemporalInput {
    pub fn new(model: TemporalModel) -> Result<Self, String> {
        let policy = model.window_input_policy()?;
        let bytes = model.export()?;
        let status = LiveDiagnostic {
            buffer_state: LiveState::Inactive,
            reason: Some("inactive"),
            model_sha256: format!("{:x}", Sha256::digest(bytes)),
            policy_sha256: model.plan_sha256().into(),
            target_horizon_ms: policy.target_span_ms(),
            cadence: "nonoverlap_full_native_horizon",
            buffer_sources_with_rows: 0,
            required_sources: policy.context().sources.len(),
            observation: ObservationStatus::unavailable("inactive"),
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
            observation: None,
            #[cfg(test)] last_input_error:None,
        })
    }
    pub fn activate(&mut self) {
        self.active = true;
        self.withdraw("collecting_full_horizon");
        self.status.buffer_state = LiveState::Warming;
    }
    pub fn deactivate(&mut self) {
        self.active = false;
        self.withdraw("inactive");
        self.status.buffer_state = LiveState::Inactive;
    }
    fn withdraw(&mut self, reason: &'static str) {
        self.input = None;
        self.target_ns = None;
        self.end_ns = None;
        self.counts.clear();
        self.continuity.clear();
        self.observation = None;
        self.status.observation = ObservationStatus::unavailable(reason);
        self.status.current_prediction = None;
        self.status.buffer_sources_with_rows = 0;
        self.status.buffer_state = if self.active {
            LiveState::Withdrawn
        } else {
            LiveState::Inactive
        };
        self.status.reason = Some(reason);
    }
    fn refresh_observation(&mut self, now_ns: u64) {
        let Some(observation) = &self.observation else {
            self.status.current_prediction = None;
            return;
        };
        if !self.active
            || observation.model_sha256 != self.status.model_sha256
            || observation.policy_sha256 != self.status.policy_sha256
            || observation.context_digest != self.policy.context().digest()
        {
            self.withdraw("observation_identity_changed");
            return;
        }
        // Required sources, not just whichever source most recently delivered.
        if self.policy.source_policies().iter().any(|p| {
            self.continuity
                .get(&p.source_id)
                .is_none_or(|last| now_ns < *last || now_ns - *last > p.max_gap_ns)
        }) {
            self.withdraw("source_gap");
            return;
        }
        let Some(age) = now_ns.checked_sub(observation.first_ns) else {
            self.withdraw("receiver_clock_reversed");
            return;
        };
        let available = now_ns <= observation.expires_ns;
        self.status.current_prediction = if available {
            Some(observation.prediction.clone())
        } else {
            None
        };
        self.status.observation = ObservationStatus {
            availability: if available {
                ObservationAvailability::Available
            } else {
                ObservationAvailability::Unavailable
            },
            reason: if available {
                None
            } else {
                Some("original_first_row_expired")
            },
            digest: Some(observation.digest.clone()),
            first_receiver_ns: Some(observation.first_ns.to_string()),
            completed_ns: Some(observation.completed_ns.to_string()),
            expires_ns: Some(observation.expires_ns.to_string()),
            age_ns: Some(age.to_string()),
            source_count: observation.source_bounds.len(),
        };
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
        self.refresh_observation(now_ns);
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
        self.status.buffer_state = LiveState::Warming;
        self.status.reason = Some("collecting_full_horizon");
        let admission = match self.input.as_mut().unwrap().ingest(p) {
            Ok(v) => v,
            Err(error) => {
                #[cfg(test)]{self.last_input_error=Some(error.clone());}
                #[cfg(not(test))]let _=error;
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
        self.status.buffer_sources_with_rows = self.counts.len();
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
            let mut source_bounds = Vec::with_capacity(self.policy.context().sources.len());
            let mut first = u64::MAX;
            let mut expires = u64::MAX;
            for spec in &self.policy.context().sources {
                let count = self.counts.get(&spec.source_id).unwrap();
                let Some(deadline) = count.first_ns.checked_add(MAX_AGE_NS) else {
                    self.withdraw("observation_time_overflow");
                    return self.status.clone();
                };
                first = first.min(count.first_ns);
                expires = expires.min(deadline);
                source_bounds.push(SourceBounds {
                    source_id: spec.source_id,
                    first_ns: count.first_ns,
                    last_ns: count.last_ns,
                });
            }
            let encoded = match serde_json::to_vec(&(
                &self.status.model_sha256,
                &self.status.policy_sha256,
                &source_bounds,
                &timing,
                &prediction,
            )) {
                Ok(v) => v,
                Err(_) => {
                    self.withdraw("observation_encoding_failed");
                    return self.status.clone();
                }
            };
            self.observation = Some(CompletedObservation {
                prediction,
                model_sha256: self.status.model_sha256.clone(),
                policy_sha256: self.status.policy_sha256.clone(),
                context_digest: self.policy.context().digest(),
                source_bounds,
                first_ns: first,
                completed_ns: now_ns,
                expires_ns: expires,
                digest: format!("{:x}", Sha256::digest(encoded)),
            });
            self.ordinal = next;
            self.target_ns = None;
            self.end_ns = None;
            self.counts.clear();
            self.status.buffer_state = LiveState::WindowComplete;
            self.status.reason = None;
            self.status.last_completed = Some(timing);
        }
        self.refresh_observation(now_ns);
        self.status.clone()
    }
}
#[cfg(test)]
#[path = "temporal_live_driver_tests.rs"]
mod tests;

/// Bounded current observation projection. This never changes inference time.
pub(crate) struct SelectedTemporalObservation {
    pub prediction:RoomPrediction,pub digest:String,pub first_ns:u64,pub end_ns:u64,pub expires_ns:u64,pub source_latest_ns:u64,
}
impl ContinuousTemporalInput {
    pub(crate) fn from_selected(selected:&crate::room_learner::artifact::StoredTemporalSelection)->Result<Self,String>{
        let mut driver=Self::new(selected.model().clone())?;
        let held=selected.held();held.validate_data()?;
        for floor in &held.body.final_source_fences {
            driver.fences.insert(floor.source_id,Fence{sequence:floor.sequence.try_into().map_err(|_|"selected_sequence_overflow")?,time_ns:floor.receiver_ns.parse().map_err(|_|"selected_receiver_clock")?,frame_id:floor.native_frame_id.parse().map_err(|_|"selected_native_frame")?});
        }
        driver.last_now=Some(held.body.complete_persisted_receiver_ns.parse().map_err(|_|"selected_receiver_clock")?);
        driver.activate();Ok(driver)
    }
    pub(crate) fn advance_selected_floors(&mut self,floors:&[(u8,u64,u64)],receiver_floor:u64)->Result<(),String>{
        if floors.len()!=3||floors.iter().any(|(id,seq,_)|*seq>u32::MAX as u64||!self.policy.context().sources.iter().any(|s|s.source_id==*id)){return Err("selected_source_floor_identity".into());}
        for(id,seq,time)in floors{let frame=(u128::from(*time)<<40)|(u128::from(*id)<<32)|u128::from(*seq);if let Some(old)=self.fences.get(id){if *seq<u64::from(old.sequence)||*time<old.time_ns{return Err("selected_source_floor_regression".into());}}
            self.fences.insert(*id,Fence{sequence:*seq as u32,time_ns:*time,frame_id:frame});}
        if self.last_now.is_some_and(|old|receiver_floor<old){return Err("selected_receiver_floor_regression".into());}self.last_now=Some(receiver_floor);Ok(())
    }
    pub(crate) fn selected_observation(&mut self,now:u64)->Result<SelectedTemporalObservation,String>{
        self.poll(now);
        if self.status.observation.availability!=ObservationAvailability::Available{return Err(self.status.observation.reason.unwrap_or("selected_native_input_unavailable").into());}
        let o=self.observation.as_ref().ok_or("selected_native_input_unavailable")?;
        if o.prediction.decision==super::model::PresenceDecision::Abstain{return Err("selected_model_abstained".into());}
        let source_latest_ns=self.policy.context().sources.iter().map(|s|self.continuity.get(&s.source_id).copied().ok_or("selected_source_missing")).collect::<Result<Vec<_>,_>>()?.into_iter().min().ok_or("selected_source_missing")?;
        if now<source_latest_ns||now-source_latest_ns>1_000_000_000{return Err("selected_source_stale".into());}
        Ok(SelectedTemporalObservation{prediction:o.prediction.clone(),digest:o.digest.clone(),first_ns:o.first_ns,end_ns:o.source_bounds.iter().map(|s|s.last_ns).max().ok_or("selected_source_missing")?,expires_ns:o.expires_ns,source_latest_ns})
    }
}

#[cfg(test)]
impl ContinuousTemporalInput{pub(crate) fn c4c_replay_floor_probe(&self)->Vec<(u8,u32,u64,u128)>{let mut out=self.fences.iter().map(|(id,f)|(*id,f.sequence,f.time_ns,f.frame_id)).collect::<Vec<_>>();out.sort_by_key(|v|v.0);out}}

#[cfg(test)]impl ContinuousTemporalInput{pub(crate) fn c4c_last_input_failure(&self)->Option<&str>{self.last_input_error.as_deref()}}
