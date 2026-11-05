//! Opt-in comparison references, never preview, calibration control or measured teachers.
//! All derived observations are RAM-only. Production timing/ROI resolution is Unknown.
use crate::{
    companion::{Companion, Grant},
    room_learner::lifecycle::ReferenceScope,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

pub const MAX_BYTES: usize = 8192;
const MAX_ROWS: usize = 4096;
const MAX_RATE: usize = 15;
const SCHEMA: &str = "ruview.companion.calibration-reference.v1";
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lineage {
    pub capture_session_id: String,
    pub coordinate_frame_id: String,
    pub clock_model_id: String,
    pub room_binding_digest: String,
    pub room_revision: u64,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReferenceMessage {
    Begin {
        schema: String,
        lineage: Lineage,
    },
    Observation {
        schema: String,
        reference_session_id: String,
        sequence: u64,
        lineage: Lineage,
        monotonic_timestamp_seconds: String,
        captured_at_unix_ms: u64,
        tracking_state: TrackingState,
        detector_completed: bool,
        visible_body_count: u8,
    },
}
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrackingState {
    Normal,
    Limited,
    Unavailable,
}
fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.is_ascii()
        && !s
            .bytes()
            .any(|c| c.is_ascii_control() || c.is_ascii_whitespace())
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
impl Lineage {
    fn valid(&self) -> bool {
        [
            &self.capture_session_id,
            &self.coordinate_frame_id,
            &self.clock_model_id,
        ]
        .iter()
        .all(|s| id(s))
            && digest(&self.room_binding_digest)
            && self.room_revision > 0
    }
    fn same(&self, other: &Self) -> bool {
        self.capture_session_id == other.capture_session_id
            && self.coordinate_frame_id == other.coordinate_frame_id
            && self.clock_model_id == other.clock_model_id
            && self.room_binding_digest == other.room_binding_digest
            && self.room_revision == other.room_revision
    }
}
impl ReferenceMessage {
    pub fn decode(bytes: &[u8], now_ms: u64) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("reference_message_budget".into());
        }
        let message: Self =
            serde_json::from_slice(bytes).map_err(|_| "reference_invalid_message")?;
        message.validate(now_ms)?;
        Ok(message)
    }
    fn validate(&self, now_ms: u64) -> Result<(), String> {
        let (schema, lineage) = match self {
            Self::Begin { schema, lineage }
            | Self::Observation {
                schema, lineage, ..
            } => (schema, lineage),
        };
        if schema != SCHEMA || !lineage.valid() {
            return Err("reference_invalid_lineage".into());
        }
        if let Self::Observation {
            reference_session_id,
            sequence,
            monotonic_timestamp_seconds,
            captured_at_unix_ms,
            visible_body_count,
            ..
        } = self
        {
            let time = monotonic_timestamp_seconds
                .parse::<f64>()
                .map_err(|_| "reference_invalid_source_time")?;
            if !id(reference_session_id)
                || *sequence == 0
                || *sequence > 9_007_199_254_740_991
                || *visible_body_count > 4
                || *captured_at_unix_ms > 9_007_199_254_740_991
                || captured_at_unix_ms.abs_diff(now_ms) > 3000
                || !time.is_finite()
                || time < 0.
                || monotonic_timestamp_seconds.len() > 32
                || monotonic_timestamp_seconds.is_empty()
                || !monotonic_timestamp_seconds
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b == b'.')
            {
                return Err("reference_invalid_observation".into());
            }
        }
        Ok(())
    }
    pub fn lineage(&self) -> &Lineage {
        match self {
            Self::Begin { lineage, .. } | Self::Observation { lineage, .. } => lineage,
        }
    }
}
struct Session {
    id: String,
    lineage: Lineage,
    scope: ReferenceScope,
    sequence: u64,
    source_time: f64,
    captured_ms: u64,
}
struct Permit {
    issued: u64,
    deadline: std::time::Instant,
    device_id: String,
    until: u64,
    generation: u64,
    recording: bool,
    revoked: watch::Receiver<bool>,
    purpose_stop: watch::Sender<bool>,
    session: Option<Session>,
    rate: Vec<u64>,
    stopped: bool,
    access_until: u64,
    access_revoked: Option<watch::Receiver<bool>>,
}
#[derive(Clone)]
pub(crate) struct ReferenceRow {
    pub scope: ReferenceScope,
    pub received_ns: u64,
    pub source_time: String,
    pub captured_ms: u64,
    pub detector_completed: bool,
    pub visible_body_count: u8,
    pub tracking: TrackingState,
}
struct Inner {
    generation: u64,
    permit: Option<Permit>,
    rows: Vec<ReferenceRow>,
}
pub struct ReferenceStore {
    companion: Arc<Companion>,
    inner: Mutex<Inner>,
}
#[derive(Debug, Serialize)]
pub struct ReferenceReply {
    pub schema: &'static str,
    pub reference_session_id: String,
    pub expires_at_unix_seconds: u64,
    pub state: &'static str,
    pub physically_validated: bool,
}
/// Private runtime snapshot; not a trusted input DTO or source-clock receipt.
#[derive(Clone)]
pub struct ReferenceSnapshot {
    pub(crate) rows: Vec<ReferenceRow>,
    pub(crate) generation: u64,
    pub(crate) until: u64,
    pub(crate) device_id: String,
    pub(crate) revoked: watch::Receiver<bool>,
    pub(crate) access_revoked: watch::Receiver<bool>,
    pub(crate) purpose_revoked: watch::Receiver<bool>,
    pub(crate) deadline: std::time::Instant,
}
impl ReferenceStore {
    pub fn new(companion: Arc<Companion>) -> Arc<Self> {
        Arc::new(Self {
            companion,
            inner: Mutex::new(Inner {
                generation: 0,
                permit: None,
                rows: vec![],
            }),
        })
    }
    /// Production integration starts one weak-owner task. It never retains the
    /// account store across a timer wait or starts camera/capture activity.
    pub fn spawn_expiry_sweeper(self: &Arc<Self>) {
        let store = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(std::time::Duration::from_millis(250));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                timer.tick().await;
                let Some(store) = store.upgrade() else { break };
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|time| time.as_secs()).unwrap_or(0);
                store.expire(now);
            }
        });
    }
    fn expire_locked(&self, state: &mut Inner, now: u64) {
        let expired = state.permit.as_ref().is_some_and(|permit| {
            !self.live(permit, None, now)
                || permit.access_until != 0 && (permit.access_until <= now
                    || permit.access_revoked.as_ref().is_some_and(|revoked| *revoked.borrow()))
        });
        if expired {
            if let Some(permit) = state.permit.take() {
                permit.purpose_stop.send_replace(true);
            }
            state.rows.clear();
        }
    }
    fn expire(&self, now: u64) {
        if let Ok(mut state) = self.inner.lock() {
            self.expire_locked(&mut state, now);
        }
    }
    /// Exact-owner middleware must guard this call. Separate purpose; never hands off control.
    pub fn permit(&self, id: &str, recording: bool, now: u64) -> Result<u64, String> {
        let (_, expires) = self
            .companion
            .paired_device(id)
            .ok_or("reference_device_not_paired")?;
        let revoked = self
            .companion
            .device_revocation(id)
            .ok_or("reference_device_not_paired")?;
        if expires <= now || *revoked.borrow() {
            return Err("reference_device_expired".into());
        }
        let mut s = self.inner.lock().map_err(|_| "reference_unavailable")?;
        s.generation = s
            .generation
            .checked_add(1)
            .ok_or("reference_generation_exhausted")?;
        let generation = s.generation;
        if let Some(old) = &s.permit {
            old.purpose_stop.send_replace(true);
        }
        let until = now.saturating_add(900).min(expires);
        s.permit = Some(Permit {
            issued: now,
            deadline: std::time::Instant::now()
                + std::time::Duration::from_secs(until.saturating_sub(now)),
            device_id: id.into(),
            until,
            generation,
            recording,
            revoked,
            purpose_stop: watch::channel(false).0,
            session: None,
            rate: vec![],
            stopped: false,
            access_until: 0,
            access_revoked: None,
        });
        s.rows.clear();
        Ok(until)
    }
    /// Privacy/owner-context withdrawal is ephemeral only; persisted RF/model
    /// history is owned by the existing account-erasure flow.
    pub fn withdraw(&self) {
        if let Ok(mut state) = self.inner.lock() {
            if let Some(permit) = state.permit.take() {
                permit.purpose_stop.send_replace(true);
            }
            state.rows.clear();
        }
    }
    pub fn revoke(&self, id: &str) -> Result<(), String> {
        let mut s = self.inner.lock().map_err(|_| "reference_unavailable")?;
        if s.permit.as_ref().is_some_and(|p| p.device_id == id) {
            if let Some(p) = &s.permit {
                p.purpose_stop.send_replace(true);
            }
            s.permit = None;
            s.rows.clear();
        }
        Ok(())
    }
    fn live(&self, p: &Permit, grant: Option<&Grant>, now: u64) -> bool {
        p.until > now
            && now >= p.issued
            && std::time::Instant::now() < p.deadline
            && !*p.revoked.borrow()
            && self.companion.paired_device(&p.device_id).is_some()
            && grant.is_none_or(|g| {
                g.device_id == p.device_id && g.expires_at > now && !*g.revoked.borrow()
            })
    }
    /// Scope is created by the actual runtime while publication/phase locks remain held.
    pub fn admit(
        &self,
        grant: &Grant,
        scope: ReferenceScope,
        message: ReferenceMessage,
        now_ns: u64,
    ) -> Result<ReferenceReply, String> {
        message.validate(now_ns / 1_000_000)?;
        if !self
            .companion
            .reference_owner_matches(&scope.owner_scope, &scope.installation_id)
        {
            return Err("reference_owner_installation_mismatch".into());
        }
        let mut s = self.inner.lock().map_err(|_| "reference_unavailable")?;
        let now = now_ns / 1_000_000_000;
        self.expire_locked(&mut s, now);
        let rows_len = s.rows.len();
        let p = s
            .permit
            .as_mut()
            .filter(|p| self.live(p, Some(grant), now))
            .ok_or("reference_purpose_required")?;
        if p.stopped || rows_len >= MAX_ROWS {
            p.stopped = true;
            return Err("reference_row_budget".into());
        }
        p.rate
            .retain(|at| now_ns.saturating_sub(*at) < 1_000_000_000);
        if p.rate.len() >= MAX_RATE {
            return Err("reference_rate_limit".into());
        }
        p.rate.push(now_ns);
        p.access_until = grant.expires_at;
        p.access_revoked = Some(grant.revoked.clone());
        let lineage = message.lineage();
        if scope.room_digest != lineage.room_binding_digest {
            return Err("reference_room_mismatch".into());
        }
        let mut row = None;
        match message {
            ReferenceMessage::Begin { lineage, .. } => {
                // A new phase may begin, but cannot relabel rows belonging to another model/run.
                if p.session.as_ref().is_some_and(|a| {
                    a.scope.runtime_epoch != scope.runtime_epoch
                        || a.scope.generation != scope.generation
                        || a.scope.context_digest != scope.context_digest
                        || a.scope.model_sha256 != scope.model_sha256
                }) {
                    return Err("reference_context_changed".into());
                }
                let previous = p
                    .session
                    .as_ref()
                    .filter(|a| a.lineage.same(&lineage))
                    .map(|a| (a.source_time, a.captured_ms))
                    .unwrap_or((-1., 0));
                p.session = Some(Session {
                    id: crate::companion::secret(),
                    lineage,
                    scope,
                    sequence: 0,
                    source_time: previous.0,
                    captured_ms: previous.1,
                });
            }
            ReferenceMessage::Observation {
                reference_session_id,
                sequence,
                lineage,
                monotonic_timestamp_seconds,
                captured_at_unix_ms,
                tracking_state,
                detector_completed,
                visible_body_count,
                ..
            } => {
                let a = p.session.as_mut().ok_or("reference_begin_required")?;
                let source_time = monotonic_timestamp_seconds
                    .parse::<f64>()
                    .map_err(|_| "reference_invalid_source_time")?;
                if a.id != reference_session_id || a.scope != scope || !a.lineage.same(&lineage) {
                    return Err("reference_session_changed".into());
                }
                if sequence <= a.sequence
                    || source_time <= a.source_time
                    || captured_at_unix_ms < a.captured_ms
                {
                    return Err("reference_replayed_or_reversed".into());
                }
                a.sequence = sequence;
                a.source_time = source_time;
                a.captured_ms = captured_at_unix_ms;
                row = Some(ReferenceRow {
                    scope,
                    received_ns: now_ns,
                    source_time: monotonic_timestamp_seconds,
                    captured_ms: captured_at_unix_ms,
                    detector_completed,
                    visible_body_count,
                    tracking: tracking_state,
                });
            }
        }
        let reply = ReferenceReply {
            schema: SCHEMA,
            reference_session_id: p
                .session
                .as_ref()
                .ok_or("reference_begin_required")?
                .id
                .clone(),
            expires_at_unix_seconds: p.until.min(grant.expires_at),
            state: "received_unresolved_clock_and_roi",
            physically_validated: false,
        };
        if let Some(row) = row {
            s.rows.push(row);
        }
        Ok(reply)
    }
    /// Recording requires the separate owner flag AND the still-current purpose.
    pub fn snapshot(&self, now: u64) -> Result<ReferenceSnapshot, String> {
        let mut s = self.inner.lock().map_err(|_| "reference_unavailable")?;
        self.expire_locked(&mut s, now);
        let p = s
            .permit
            .as_ref()
            .filter(|p| {
                self.live(p, None, now)
                    && p.recording
                    && p.access_until > now
                    && p.access_revoked.as_ref().is_some_and(|r| !*r.borrow())
            })
            .ok_or("reference_recording_consent_required")?;
        Ok(ReferenceSnapshot {
            rows: s.rows.clone(),
            generation: p.generation,
            until: p.until.min(p.access_until),
            device_id: p.device_id.clone(),
            revoked: p.revoked.clone(),
            purpose_revoked: p.purpose_stop.subscribe(),
            deadline: p.deadline,
            access_revoked: p.access_revoked.clone().ok_or("reference_begin_required")?,
        })
    }
    pub fn summary(&self, now: u64) -> serde_json::Value {
        let Ok(mut s) = self.inner.lock() else {
            return serde_json::json!({"state":"unavailable"});
        };
        self.expire_locked(&mut s, now);
        let active = s.permit.as_ref().is_some_and(|p| self.live(p, None, now));
        serde_json::json!({"schema":SCHEMA,"active":active,"ingress_budget_exhausted":s.permit.as_ref().is_some_and(|p|p.stopped),"retained_ram_rows":if active{s.rows.len()}else{0},"comparison_only":true,"physically_validated":false,"adoption_authorized":false})
    }
}
#[cfg(test)]
#[path = "companion_references_tests.rs"]
pub(crate) mod tests;

impl ReferenceSnapshot {
    pub fn rows_room_digest(&self) -> Option<&str> {
        self.rows.first().map(|r| r.scope.room_digest.as_str())
    }
    pub(crate) fn ensure_live(&self) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "reference_clock_unavailable")?
            .as_secs();
        if now >= self.until
            || std::time::Instant::now() >= self.deadline
            || *self.revoked.borrow()
            || *self.access_revoked.borrow()
            || *self.purpose_revoked.borrow()
        {
            Err("reference_purpose_expired_or_revoked".into())
        } else {
            Ok(())
        }
    }
}
