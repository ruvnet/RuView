//! Source-preserving RF views and frozen, leakage-free capture cohorts.
//! All declarations are input claims: validation never grants physical authority.
use ruview_unified::{LinkGeometry, RfModality, RfTensor};
use ruview_unified::frame::{FieldAxis, PhaseState, ProvenanceClass, RfFrameV2};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

pub const MAX_SOURCES: usize = 16;
pub const MAX_FEATURES: usize = 512;
pub const MAX_ROWS: usize = 256;
pub const MAX_COHORTS: usize = 128;
pub const MAX_TOTAL_VALUES: usize = 2_000_000;
pub const MAX_AGE_NS: u64 = 5_000_000_000;
pub const MAX_WINDOW_NS: u64 = 60_000_000_000;
pub const MAX_CLOCK_UNCERTAINTY_NS: u64 = 100_000_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SourceSpec {
    pub source_id: u8,
    pub device_id: String,
    pub grid_id: String,
    pub feature_count: usize,
    pub signal_kind: RfModality,
    pub center_frequency_hz: u64,
    pub bandwidth_hz: u64,
    pub link_id: String,
    pub link_index: usize,
    pub geometry: Option<LinkGeometry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CaptureContext {
    pub owner_scope: String,
    pub installation_id: String,
    pub room_digest: String,
    pub revision: u64,
    pub sources: Vec<SourceSpec>,
}

impl CaptureContext {
    /// SHA-256 of the full ordered source/configuration contract, not RF data.
    pub fn digest(&self) -> String {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        format!("{:x}", Sha256::digest(bytes))
    }

    pub fn validate(&self) -> Result<(), String> {
        for s in [&self.owner_scope, &self.installation_id, &self.room_digest] { identifier(s)?; }
        if self.revision == 0 || self.sources.is_empty() || self.sources.len() > MAX_SOURCES {
            return Err("invalid context revision or source count".into());
        }
        if self.sources.windows(2).any(|w| w[0].source_id >= w[1].source_id) {
            return Err("source specs must be sorted uniquely by source id".into());
        }
        let mut ids = HashSet::new();
        let mut links = HashSet::new();
        for s in &self.sources {
            for id in [&s.device_id, &s.grid_id, &s.link_id] { identifier(id)?; }
            if !ids.insert(s.source_id) || !links.insert((&s.device_id, &s.link_id))
                || s.feature_count == 0 || s.feature_count > MAX_FEATURES || s.link_index >= MAX_SOURCES
                || s.center_frequency_hz == 0 || s.bandwidth_hz == 0 {
                return Err("duplicate source/link or invalid feature count".into());
            }
            if let Some(g) = s.geometry { geometry(g)?; }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClockKind { SourceMeasurement, ReceiverOnly, Unknown }
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClockInfo { pub kind: ClockKind, pub uncertainty_ns: u64 }
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CaptureProvenance { Synthetic, CapturedReplay, LiveReceiverObserved }
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PhaseAvailability { Raw, Sanitized, Calibrated, MagnitudeOnly, Unknown }

/// Explicit per-row identities/times supplied by ingestion; tensor timestamp alone
/// does not establish a source clock (#1710).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMetadata {
    pub frame_ids: Vec<u128>,
    pub sequence_ids: Vec<u64>,
    pub first_time_ns: u64,
    pub last_time_ns: u64,
    pub clock: ClockInfo,
    pub phase: PhaseAvailability,
    pub link_index: usize,
    pub provenance: CaptureProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceView {
    pub source_id: u8,
    pub device_id: String,
    pub grid_id: String,
    pub signal_kind: RfModality,
    pub center_frequency_hz: u64,
    pub bandwidth_hz: u64,
    pub link_id: String,
    pub link_index: usize,
    pub geometry: Option<LinkGeometry>,
    pub samples: Vec<Vec<f32>>,
    pub frame_ids: Vec<u128>,
    pub sequence_ids: Vec<u64>,
    pub first_time_ns: u64,
    pub last_time_ns: u64,
    pub clock: ClockInfo,
    pub sample_age_ns: u64,
    pub phase: PhaseAvailability,
    pub provenance: CaptureProvenance,
}

impl SourceView {
    /// Reads magnitudes without per-window centering, scaling or gain removal.
    /// Unified adapter normalization averages away per-link gain: it is not
    /// invertible. Such tensors must use the native-frame entry point instead.
    pub fn from_tensor(source: &SourceSpec, tensor: &RfTensor, meta: ViewMetadata) -> Result<Self, String> {
        let (links, bins, rows) = tensor.dims();
        if tensor.calibration.gain_offset_db != 0.0 || tensor.calibration.baseline_id.is_some() {
            return Err("gain-normalized/baseline-subtracted tensor cannot preserve source amplitude".into());
        }
        if links != 1 || meta.link_index != source.link_index || meta.link_index >= links || tensor.links.len() != links || bins != source.feature_count
            || tensor.device_id != source.device_id || tensor.modality != source.signal_kind
            || tensor.center_freq_hz != source.center_frequency_hz as f64
            || tensor.bandwidth_hz != source.bandwidth_hz as f64
            || !tensor.sample_age_s.is_finite() || tensor.sample_age_s < 0.0
            || tensor.sample_age_s > MAX_AGE_NS as f64 / 1e9
            || rows == 0 || rows > MAX_ROWS || bins > MAX_FEATURES
            || tensor.timestamp_ns != meta.last_time_ns {
            return Err("tensor/source/window contract mismatch".into());
        }
        if meta.phase == PhaseAvailability::Raw || meta.phase == PhaseAvailability::Sanitized
            || (meta.phase == PhaseAvailability::Calibrated && !tensor.calibration.phase_calibrated) {
            return Err("tensor does not establish requested phase state".into());
        }
        let actual_geometry = tensor.links[meta.link_index];
        if source.geometry.is_some_and(|g| g != actual_geometry) {
            return Err("tensor link geometry changed".into());
        }
        let samples = (0..rows).map(|t| (0..bins)
            .map(|b| tensor.data[[meta.link_index, b, t]].norm() as f32).collect()).collect();
        let view = Self::build(source, samples, meta, (tensor.sample_age_s * 1e9) as u64,
            tensor.modality == RfModality::Synthetic);
        view.validate(source)?;
        Ok(view)
    }

    /// Native IQ avoids the existing canonical adapter's median normalization.
    /// Only an explicit `(link, bin, time)` layout with complete valid samples
    /// is accepted; missing bins cannot silently become measurements.
    pub fn from_frame(source: &SourceSpec, frame: &RfFrameV2, meta: ViewMetadata) -> Result<Self, String> {
        if frame.schema_version != 2 || frame.native_shape.len() != 3
            || frame.axes.len() != 3 || frame.axes[0] != FieldAxis::Antenna
            || !matches!(frame.axes[1], FieldAxis::Frequency | FieldAxis::Delay | FieldAxis::Range)
            || frame.axes[2] != FieldAxis::Time || frame.sample_age_ns > MAX_AGE_NS
            || frame.modality != source.signal_kind || frame.provenance.device_id != source.device_id
            || frame.centre_frequency_hz != source.center_frequency_hz as f64
            || frame.bandwidth_hz != source.bandwidth_hz as f64
            || frame.calibration.baseline_id.is_some() || frame.timestamp_ns != meta.last_time_ns {
            return Err("native frame/source/layout contract mismatch".into());
        }
        let (links, bins, rows) = (frame.native_shape[0], frame.native_shape[1], frame.native_shape[2]);
        if links == 0 || links > MAX_SOURCES || bins != source.feature_count || bins > MAX_FEATURES
            || rows == 0 || rows > MAX_ROWS || meta.link_index != source.link_index || meta.link_index >= links
            || frame.native_iq.len() != links * bins * rows || frame.valid_mask.len() != links * bins * rows
            || meta.frame_ids.iter().any(|id| *id != frame.frame_id) {
            return Err("native shape/frame identity mismatch".into());
        }
        let expected_phase = match frame.calibration.phase_state {
            PhaseState::Raw => PhaseAvailability::Raw,
            PhaseState::Sanitized => PhaseAvailability::Sanitized,
            PhaseState::Calibrated => PhaseAvailability::Calibrated,
            PhaseState::Unavailable => PhaseAvailability::MagnitudeOnly,
        };
        if meta.phase != expected_phase { return Err("native phase declaration mismatch".into()); }
        if let Some(g) = source.geometry {
            if frame.transmitter_pose.map(|p| p.position_m) != Some(g.tx_pos)
                || frame.receiver_pose.map(|p| p.position_m) != Some(g.rx_pos) {
                return Err("native geometry changed".into());
            }
        }
        let mut samples = Vec::with_capacity(rows);
        for t in 0..rows {
            let mut row = Vec::with_capacity(bins);
            for b in 0..bins {
                let i = meta.link_index * bins * rows + b * rows + t;
                if !frame.valid_mask[i] { return Err("native sample is invalid".into()); }
                row.push(frame.native_iq[i].norm() as f32);
            }
            samples.push(row);
        }
        let view = Self::build(source, samples, meta, frame.sample_age_ns,
            frame.provenance.class == ProvenanceClass::Synthetic);
        view.validate(source)?;
        Ok(view)
    }

    fn build(s: &SourceSpec, samples: Vec<Vec<f32>>, m: ViewMetadata, sample_age_ns: u64, synthetic: bool) -> Self {
        Self { source_id: s.source_id, device_id: s.device_id.clone(), grid_id: s.grid_id.clone(),
            signal_kind: s.signal_kind, center_frequency_hz: s.center_frequency_hz, bandwidth_hz: s.bandwidth_hz,
            link_id: s.link_id.clone(), link_index: s.link_index, geometry: s.geometry, samples,
            frame_ids: m.frame_ids, sequence_ids: m.sequence_ids, first_time_ns: m.first_time_ns,
            last_time_ns: m.last_time_ns, clock: m.clock, sample_age_ns, phase: m.phase,
            provenance: if synthetic { CaptureProvenance::Synthetic } else { m.provenance } }
    }

    pub fn validate(&self, s: &SourceSpec) -> Result<(), String> {
        if self.source_id != s.source_id || self.grid_id != s.grid_id || self.device_id != s.device_id
            || self.link_id != s.link_id || self.link_index != s.link_index || self.signal_kind != s.signal_kind || self.geometry != s.geometry
            || self.center_frequency_hz != s.center_frequency_hz || self.bandwidth_hz != s.bandwidth_hz {
            return Err("source identity, PHY grid, modality or geometry changed".into());
        }
        if (self.signal_kind == RfModality::Synthetic && self.provenance != CaptureProvenance::Synthetic)
            || self.samples.len() < 2 || self.samples.len() > MAX_ROWS
            || self.frame_ids.len() != self.samples.len() || self.sequence_ids.len() != self.samples.len()
            || self.first_time_ns == 0 || self.last_time_ns <= self.first_time_ns
            || self.last_time_ns - self.first_time_ns > MAX_WINDOW_NS || self.sample_age_ns > MAX_AGE_NS
            || self.frame_ids.contains(&0) || self.sequence_ids.windows(2).any(|w| w[1] <= w[0])
            || (self.clock.kind == ClockKind::SourceMeasurement
                && self.clock.uncertainty_ns > MAX_CLOCK_UNCERTAINTY_NS) {
            return Err("invalid bounded source window, identities, age or clock".into());
        }
        if self.samples.iter().any(|row| row.len() != s.feature_count
            || row.iter().any(|v| !v.is_finite() || *v < 0.0)) {
            return Err("invalid magnitude sample/feature grid".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DataRole { Train, Validation, HeldOut, Inference }
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RoomLabel { Empty, OccupiedStatic, OccupiedMoving, AdjacentOnly, Unknown }
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TeacherKind { Camera, Radar }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeacherEvidence {
    pub kind: TeacherKind,
    pub source_id: String,
    /// Unverified receipt claim; only an external paired registry can resolve it.
    pub validation_receipt: String,
    pub context_digest: String,
    pub room_digest: String,
    pub trial_id: String,
    pub label: RoomLabel,
    pub frame_ids: Vec<u128>,
    pub first_time_ns: u64,
    pub last_time_ns: u64,
    pub clock: ClockInfo,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureCohort {
    pub id: String,
    pub context_digest: String,
    pub trial_id: String,
    pub session_id: String,
    pub role: DataRole,
    pub label: RoomLabel,
    pub views: Vec<SourceView>,
    pub teacher: Option<TeacherEvidence>,
    pub provenance: CaptureProvenance,
}

/// Read-only after construction: held-out rows cannot accidentally be fitted.
#[derive(Debug, Clone)]
pub struct ValidatedDataset {
    context: CaptureContext,
    train: Vec<CaptureCohort>,
    validation: Vec<CaptureCohort>,
    held_out: Vec<CaptureCohort>,
    measurement_clock_available: bool,
}
type SourceRoleIntervals = HashMap<(u8, String), Vec<(u64, u64, DataRole)>>;
impl ValidatedDataset {
    pub fn new(context: CaptureContext, cohorts: Vec<CaptureCohort>) -> Result<Self, String> {
        context.validate()?;
        if cohorts.is_empty() || cohorts.len() > MAX_COHORTS { return Err("invalid cohort count".into()); }
        let digest = context.digest();
        let mut result = Self { context, train: vec![], validation: vec![], held_out: vec![], measurement_clock_available: true };
        let mut total_values = 0usize;
        let mut ids = HashSet::new();
        let mut trials = HashMap::new();
        let mut sessions = HashMap::new();
        let mut frames = HashMap::new();
        let mut sequences = HashMap::new();
        let mut intervals: SourceRoleIntervals = HashMap::new();
        for c in cohorts {
            if c.role == DataRole::Inference || c.label == RoomLabel::Unknown {
                return Err("inference/unlabelled input cannot enter a labelled dataset".into());
            }
            validate_cohort(&result.context, &c)?;
            for id in [&c.id, &c.trial_id, &c.session_id] { identifier(id)?; }
            if !ids.insert(c.id.clone()) || c.context_digest != digest || c.views.len() != result.context.sources.len() {
                return Err("duplicate cohort, wrong context or incomplete selected sources".into());
            }
            for (map, key) in [(&mut trials, &c.trial_id), (&mut sessions, &c.session_id)] {
                if map.insert(key.clone(), c.role).is_some_and(|role| role != c.role) {
                    return Err("trial/session crosses frozen roles".into());
                }
            }
            let mut selected = HashSet::new();
            let mut first = 0;
            let mut last = u64::MAX;
            for v in &c.views {
                let spec = result.context.sources.iter().find(|s| s.source_id == v.source_id)
                    .ok_or("unknown source")?;
                v.validate(spec)?;
                total_values += v.samples.len() * spec.feature_count;
                if total_values > MAX_TOTAL_VALUES { return Err("dataset value budget exceeded".into()); }
                if !selected.insert(v.source_id) { return Err("duplicate source view".into()); }
                result.measurement_clock_available &= v.provenance != CaptureProvenance::Synthetic
                    && v.clock.kind == ClockKind::SourceMeasurement;
                first = first.max(v.first_time_ns);
                last = last.min(v.last_time_ns);
                for id in &v.frame_ids {
                    if frames.insert((v.source_id, *id), c.role).is_some_and(|r| r != c.role) {
                        return Err("frame overlap crosses roles".into());
                    }
                }
                for seq in &v.sequence_ids {
                    if sequences.insert((v.source_id, c.session_id.clone(), *seq), c.id.clone())
                        .is_some_and(|id| id != c.id) { return Err("reused source sequence".into()); }
                }
                let ranges = intervals.entry((v.source_id, c.session_id.clone())).or_default();
                if ranges.iter().any(|(a,b,r)| *r != c.role && v.first_time_ns <= *b && v.last_time_ns >= *a) {
                    return Err("source time window overlaps roles".into());
                }
                ranges.push((v.first_time_ns, v.last_time_ns, c.role));
            }
            if first >= last { return Err("selected source windows are not aligned".into()); }
            if let Some(t) = &c.teacher { validate_teacher(t, &c, &result.context, first, last)?; }
            // Labels without an independently validated teacher are still useful
            // experimental/manual cohorts, never physical empty-room evidence.

            match c.role { DataRole::Train => result.train.push(c), DataRole::Validation => result.validation.push(c), DataRole::HeldOut => result.held_out.push(c), DataRole::Inference => return Err("inference dataset rejected".into()) }
        }
        Ok(result)
    }
    pub fn context(&self) -> &CaptureContext { &self.context }
    pub fn train(&self) -> &[CaptureCohort] { &self.train }
    pub fn validation(&self) -> &[CaptureCohort] { &self.validation }
    pub fn held_out(&self) -> &[CaptureCohort] { &self.held_out }
    /// Clock/provenance input claims only; never an activation or qualification receipt.
    pub fn measurement_clock_available(&self) -> bool { self.measurement_clock_available }
}

/// Validates one inference/capture boundary without assigning or changing splits.
pub fn validate_cohort(context: &CaptureContext, c: &CaptureCohort) -> Result<(), String> {
    context.validate()?;
    if (c.role == DataRole::Inference) != (c.label == RoomLabel::Unknown)
        || (c.role == DataRole::Inference && c.teacher.is_some()) {
        return Err("inference must be unlabelled and cannot carry teacher labels".into());
    }
    for id in [&c.id, &c.trial_id, &c.session_id] { identifier(id)?; }
    if c.context_digest != context.digest() || c.views.len() != context.sources.len() {
        return Err("cohort context or selected source count mismatch".into());
    }
    let mut ids = HashSet::new();
    let (mut first, mut last) = (0, u64::MAX);
    for v in &c.views {
        let spec = context.sources.iter().find(|s| s.source_id == v.source_id).ok_or("unknown source")?;
        v.validate(spec)?;
        if !ids.insert(v.source_id) || v.provenance != c.provenance {
            return Err("duplicate view or inconsistent provenance".into());
        }
        first = first.max(v.first_time_ns);
        last = last.min(v.last_time_ns);
    }
    if first >= last { return Err("selected source windows are not aligned".into()); }
    if let Some(t) = &c.teacher { validate_teacher(t, c, context, first, last)?; }
    Ok(())
}

fn validate_teacher(t: &TeacherEvidence, c: &CaptureCohort, context: &CaptureContext, first: u64, last: u64) -> Result<(), String> {
    // A client-supplied receipt string is never proof of a validated source.
    // Until a paired-source resolver exists, only synthetic fixtures can attach
    // teacher claims. Real capture uses explicit operator labels.
    if c.provenance != CaptureProvenance::Synthetic {
        return Err("teacher registry resolver unavailable for measured capture".into());
    }
    identifier(&t.source_id)?;
    identifier(&t.validation_receipt)?;
    if t.context_digest != c.context_digest || t.room_digest != context.room_digest || t.trial_id != c.trial_id
        || t.label != c.label || t.first_time_ns > first || t.last_time_ns < last
        || c.views.iter().any(|v| t.first_time_ns > v.first_time_ns || t.last_time_ns < v.last_time_ns)
        || t.clock.kind != ClockKind::SourceMeasurement || t.clock.uncertainty_ns > MAX_CLOCK_UNCERTAINTY_NS
        || t.frame_ids.is_empty() || t.frame_ids.len() > MAX_ROWS || t.frame_ids.contains(&0) {
        return Err("teacher is not validated and bound to this room/phase/time/frame".into());
    }
    let rf_frames: HashSet<_> = c.views.iter().flat_map(|v| v.frame_ids.iter()).collect();
    if !t.frame_ids.iter().all(|id| rf_frames.contains(id))
        || !rf_frames.iter().all(|id| t.frame_ids.contains(id)) {
        return Err("teacher references an unknown RF frame".into());
    }
    Ok(())
}
fn identifier(s: &str) -> Result<(), String> {
    if s.is_empty() || s.len() > 256 || s.trim() != s || !s.is_ascii() || s.chars().any(char::is_control) {
        Err("invalid bounded identifier".into())
    } else { Ok(()) }
}
fn geometry(g: LinkGeometry) -> Result<(), String> {
    if g.tx_pos.iter().chain(g.rx_pos.iter()).any(|x| !x.is_finite()) {
        Err("non-finite link geometry".into())
    } else { Ok(()) }
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;
