//! Current C4C association metadata, adapted from reviewed2453/2458.
//! SAME evaluation pass/store. No teacher, acquisition-clock or ROI authority.
use super::*;
use crate::{
    companion_references::ReferenceSnapshot,
    room_learner::{
        model::{PresenceDecision, RoomPrediction},
        observation::{CaptureCohort, CaptureProvenance, ClockKind, DataRole, RoomLabel},
        temporal_model_input::managed::execution::full_run::{
            VerifiedTemporalEvaluationResult, VerifiedTemporalEvaluationSnapshot,
        },
    },
};
use std::sync::atomic::{AtomicBool, Ordering};
const SCHEMA: &str = "ruview.temporal-reference-comparison.v1";
const MAX_BYTES: usize = 1_048_576;
const INTEGRITY: &str = "owner_private_checksummed_diagnostic_not_authenticated_per_window_truth";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct SourceTiming {
    source_id: u8,
    grid_id: String,
    first_time_ns: String,
    last_time_ns: String,
    clock_kind: ClockKind,
    uncertainty_ns: String,
    frame_commitment_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct WindowOutcome {
    cohort_sha256: String,
    trial_commitment: String,
    session_commitment: String,
    condition: RoomLabel,
    role: DataRole,
    provenance: CaptureProvenance,
    decision: PresenceDecision,
    occupied_probability: Option<f32>,
    sources: Vec<SourceTiming>,
    reference_state: String,
    associated_reference_rows: usize,
    reference_commitments: Vec<String>,
}
/// Private derived metadata only; no raw values, reference observations or
/// inner-v1 identity. Minted from the actual opaque whole-model snapshot.
#[derive(Clone)]
pub(crate) struct RecordedTemporalComparison {
    outcomes: Vec<WindowOutcome>,
}
fn sha<T: Serialize>(v: &T) -> Result<String, String> {
    crate::room_learner::qualification::hash(v)
}
impl RecordedTemporalComparison {
    pub(crate) fn clone_metadata(&self) -> Self {
        Self {
            outcomes: self.outcomes.clone(),
        }
    }
    pub(crate) fn from_actual(
        source: &VerifiedTemporalEvaluationSnapshot,
        predictions: &[RoomPrediction],
    ) -> Result<Self, String> {
        source.require_same_model(source.model())?;
        let rows = source.evaluation_rows();
        if rows.len() != 64 || rows.len() != predictions.len() {
            return Err("comparison_window_count".into());
        }
        let outcomes = rows
            .iter()
            .zip(predictions)
            .map(|(row, p)| {
                if row.role != DataRole::HeldOut
                    || row.teacher.is_some()
                    || p.physically_validated
                    || row.views.len() != 3
                {
                    return Err("comparison_original_heldout_required".into());
                }
                let mut views: Vec<_> = row.views.iter().collect();
                views.sort_by_key(|v| v.source_id);
                let sources = views
                    .into_iter()
                    .map(|v| {
                        Ok(SourceTiming {
                            source_id: v.source_id,
                            grid_id: v.grid_id.clone(),
                            first_time_ns: v.first_time_ns.to_string(),
                            last_time_ns: v.last_time_ns.to_string(),
                            clock_kind: v.clock.kind,
                            uncertainty_ns: v.clock.uncertainty_ns.to_string(),
                            frame_commitment_sha256: sha(&(&v.frame_ids, &v.sequence_ids))?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(WindowOutcome {
                    cohort_sha256: sha(row)?,
                    trial_commitment: sha(&row.trial_id)?,
                    session_commitment: sha(&row.session_id)?,
                    condition: row.label,
                    role: row.role,
                    provenance: row.provenance,
                    decision: p.decision,
                    occupied_probability: p.occupied_probability,
                    sources,
                    reference_state: "unknown_no_associated_reference".into(),
                    associated_reference_rows: 0,
                    reference_commitments: vec![],
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self { outcomes })
    }
    fn associate(
        &mut self,
        source: &VerifiedTemporalEvaluationSnapshot,
        snapshot: &ReferenceSnapshot,
    ) -> Result<(), String> {
        snapshot.ensure_live()?;
        let identity = &source.proof().body.identity;
        for reference in &snapshot.rows {
            let s = &reference.scope;
            if s.model_kind != identity.artifact_kind
                || s.model_sha256 != identity.artifact_sha256
                || s.fit_record_sha256 != identity.verified_fit_record_sha256
                || s.training_proof_sha256 != source.proof().sha256
                || s.run != identity.run
                || s.context_digest != identity.context_digest
                || s.room_digest != identity.room_digest
                || !source
                    .binding()
                    .owner_matches(&s.runtime_epoch, s.generation)
                || s.role != DataRole::HeldOut
            {
                return Err("comparison_reference_identity_changed".into());
            }
        }
        let rows = source.evaluation_rows();
        for (out, row) in self.outcomes.iter_mut().zip(&rows) {
            let first = row
                .views
                .iter()
                .map(|v| v.first_time_ns)
                .max()
                .ok_or("comparison_source_missing")?;
            let last = row
                .views
                .iter()
                .map(|v| v.last_time_ns)
                .min()
                .ok_or("comparison_source_missing")?;
            for reference in snapshot.rows.iter().filter(|r| {
                r.scope.trial_id == row.trial_id
                    && r.scope.session_id == row.session_id
                    && r.received_ns >= first
                    && r.received_ns <= last
            }) {
                // Association in host receiver time NEVER proves source-time or
                // whole-room coverage; zero visible bodies cannot label Empty.
                out.reference_commitments.push(sha(&(
                    reference.received_ns.to_string(),
                    &reference.source_time,
                    reference.captured_ms,
                    reference.detector_completed,
                    reference.visible_body_count,
                    format!("{:?}", reference.tracking),
                ))?);
            }
            out.associated_reference_rows = out.reference_commitments.len();
            if out.associated_reference_rows > 0 {
                out.reference_state = "unknown_unverified_clock_and_roi".into();
            }
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    schema: String,
    model_kind: String,
    model_sha256: String,
    context_digest: String,
    verified_fit_record_sha256: String,
    training_proof_sha256: String,
    held_out_evidence_sha256: String,
    complete_record_sha256: String,
    evaluation_receipt_sha256: String,
    room_revision: u64,
    purpose_generation: u64,
    device_commitment: String,
    outcomes: Vec<WindowOutcome>,
    comparison_only: bool,
    history_only: bool,
    integrity_claim: String,
    physically_validated: bool,
    adoption_authorized: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    sha256: String,
    comparison: Body,
}
fn path(
    store: &ArtifactStore,
    eval: &StoredTemporalEvaluation,
    create: bool,
) -> Result<Option<PathBuf>, String> {
    let Some(dir) = store.temporal_evaluation_directory(eval.binding().context(), create)? else {
        return Ok(None);
    };
    // Protected lookup from the current StoredEvaluation, never caller paths/IDs.
    Ok(Some(
        dir.join(format!("comparison-{}.json", eval.lookup_sha256())),
    ))
}
fn comparison_file_present(path: &Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("comparison_unsafe_metadata_entry".into())
        }
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err("comparison_metadata_unavailable".into()),
    }
}
fn lexical_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn validate_window_metadata(
    outcome: &WindowOutcome,
    context: &CaptureContext,
    allowed: &[CaptureProvenance],
) -> Result<(), String> {
    if !allowed.contains(&outcome.provenance)
        || ![
            &outcome.cohort_sha256,
            &outcome.trial_commitment,
            &outcome.session_commitment,
        ]
        .into_iter()
        .all(|h| lexical_hash(h))
        || !outcome
            .reference_commitments
            .iter()
            .all(|h| lexical_hash(h))
    {
        return Err("comparison_metadata_integrity_shape".into());
    }
    for source in &outcome.sources {
        if !lexical_hash(&source.frame_commitment_sha256)
            || !context
                .sources
                .iter()
                .any(|s| s.source_id == source.source_id && s.grid_id == source.grid_id)
        {
            return Err("comparison_source_metadata_changed".into());
        }
    }
    Ok(())
}
fn validate_outcomes(body: &Body, evaluation: &StoredTemporalEvaluation) -> Result<(), String> {
    use crate::room_learner::training::BinaryConfusion;
    use std::collections::HashSet;
    let mut hashes = HashSet::new();
    let mut counts = [0usize; 4];
    let mut confusion = BinaryConfusion::default();
    let mut n = 0;
    let mut loss = 0.0f64;
    let ids = &evaluation.train().body.identity.source_ids;
    let mut associated = 0usize;
    for outcome in &body.outcomes {
        validate_window_metadata(
            outcome,
            evaluation.binding().context(),
            &evaluation.receipt().body.provenance,
        )?;
        let condition = match outcome.condition {
            RoomLabel::Empty => 0,
            RoomLabel::OccupiedStatic => 1,
            RoomLabel::OccupiedMoving => 2,
            RoomLabel::AdjacentOnly => 3,
            _ => return Err("comparison_condition".into()),
        };
        counts[condition] += 1;
        if !hashes.insert(&outcome.cohort_sha256)
            || outcome.role != DataRole::HeldOut
            || outcome
                .sources
                .iter()
                .map(|s| s.source_id)
                .collect::<Vec<_>>()
                != ids.to_vec()
            || outcome.associated_reference_rows != outcome.reference_commitments.len()
            || (outcome.associated_reference_rows == 0
                && outcome.reference_state != "unknown_no_associated_reference")
            || (outcome.associated_reference_rows > 0
                && outcome.reference_state != "unknown_unverified_clock_and_roi")
        {
            return Err("comparison_window_metadata".into());
        }
        associated = associated
            .checked_add(outcome.associated_reference_rows)
            .ok_or("comparison_row_budget")?;
        for source in &outcome.sources {
            let first = source
                .first_time_ns
                .parse::<u64>()
                .map_err(|_| "comparison_time")?;
            let last = source
                .last_time_ns
                .parse::<u64>()
                .map_err(|_| "comparison_time")?;
            let uncertainty = source
                .uncertainty_ns
                .parse::<u64>()
                .map_err(|_| "comparison_time")?;
            if first.to_string() != source.first_time_ns
                || last.to_string() != source.last_time_ns
                || uncertainty.to_string() != source.uncertainty_ns
                || first == 0
                || last <= first
                || last - first > 2_150_000_000
                || source.clock_kind != ClockKind::ReceiverOnly
            {
                return Err("comparison_time_or_clock".into());
            }
        }
        let positive = matches!(
            outcome.condition,
            RoomLabel::OccupiedStatic | RoomLabel::OccupiedMoving
        );
        let probability = outcome
            .occupied_probability
            .filter(|p| p.is_finite() && (0.0..=1.0).contains(p));
        if probability != outcome.occupied_probability {
            return Err("comparison_probability".into());
        }
        if let Some(p) = probability {
            let p = p.clamp(1e-6, 1.0 - 1e-6) as f64;
            loss -= if positive { p.ln() } else { (1.0 - p).ln() };
            n += 1;
        }
        if outcome.decision == PresenceDecision::Abstain || probability.is_none() {
            if positive {
                confusion.abstained_occupied += 1;
                if probability.is_none() {
                    confusion.missing_occupied += 1;
                }
            } else {
                confusion.abstained_empty += 1;
                if probability.is_none() {
                    confusion.missing_empty += 1;
                }
            }
        } else {
            match (positive, outcome.decision) {
                (false, PresenceDecision::Empty) => confusion.true_empty += 1,
                (false, PresenceDecision::Occupied) => confusion.false_occupied += 1,
                (true, PresenceDecision::Empty) => confusion.false_empty += 1,
                (true, PresenceDecision::Occupied) => confusion.true_occupied += 1,
                _ => unreachable!(),
            }
        }
    }
    let report = &evaluation.receipt().body.report;
    let bce = if n == 0 {
        None
    } else {
        Some((loss / n as f64) as f32)
    };
    if counts != [16; 4]
        || associated > 4096
        || confusion != report.confusion
        || n != report.binary_cross_entropy_samples
        || bce != report.binary_cross_entropy
    {
        return Err("comparison_report_mismatch".into());
    }
    Ok(())
}
impl ArtifactStore {
    /// Existing RF receipt commits FIRST. Comparison failure never invalidates
    /// it. A pre-existing RF receipt is never reinferred or retroactively joined.
    pub(crate) fn evaluate_temporal_snapshot_with_comparison(
        &self,
        source: VerifiedTemporalEvaluationSnapshot,
        references: ReferenceSnapshot,
        cancel: &AtomicBool,
        clock: &mut dyn FnMut() -> Result<u64, String>,
        io: &mut dyn FnMut(&'static str) -> Result<(), String>,
    ) -> Result<StoredTemporalEvaluation, String> {
        source.require_same_store(self)?;
        if cancel.load(Ordering::Acquire) {
            return Err("temporal_evaluation_cancelled".into());
        }
        clock()?;
        if self.load_current_temporal_evaluation(&source)?.is_some() {
            return self.evaluate_temporal_snapshot(source, cancel, clock, io);
        }
        let result = source.compute_verified_recorded(cancel, clock)?;
        let key = super::temporal_evaluation_store::lookup(&source)?;
        let stored =
            self.persist_temporal_evaluation_result(&source, &result, &key, cancel, clock, io)?;
        let optional = references.with_recording(|| {
            let mut recorded = result.recorded().ok_or("comparison_recording_missing")?;
            recorded.associate(&source, &references)?;
            let room_revision = references
                .rows
                .first()
                .map(|r| r.scope.room_revision)
                .ok_or("comparison_no_references")?;
            let id = &stored.train().body.identity;
            let comparison = Body {
                schema: SCHEMA.into(),
                model_kind: id.artifact_kind.clone(),
                model_sha256: id.artifact_sha256.clone(),
                context_digest: id.context_digest.clone(),
                verified_fit_record_sha256: id.verified_fit_record_sha256.clone(),
                training_proof_sha256: stored.train().sha256.clone(),
                held_out_evidence_sha256: stored.held_out().sha256.clone(),
                complete_record_sha256: stored.held_out().body.complete_record_sha256.clone(),
                evaluation_receipt_sha256: stored.receipt().sha256.clone(),
                room_revision,
                purpose_generation: references.generation,
                device_commitment: sha(&references.device_id)?,
                outcomes: recorded.outcomes,
                comparison_only: true,
                history_only: true,
                integrity_claim: INTEGRITY.into(),
                physically_validated: false,
                adoption_authorized: false,
            };
            validate_outcomes(&comparison, &stored)?;
            let envelope = Envelope {
                schema: SCHEMA.into(),
                sha256: sha(&comparison)?,
                comparison,
            };
            let bytes = serde_json::to_vec(&envelope).map_err(|_| "comparison_encoding")?;
            if bytes.len() > MAX_BYTES {
                return Err("comparison_metadata_budget".into());
            }
            let target = path(self, &stored, true)?.ok_or("comparison_directory_missing")?;
            references.ensure_live()?;
            temporal_artifacts::immutable_with_io(&target, &bytes, &mut |stage| {
                if cancel.load(Ordering::Acquire) {
                    return Err("comparison_cancelled".into());
                }
                clock()?;
                references.ensure_live()?;
                io(stage)
            })?;
            references.ensure_live()?;
            if temporal_artifacts::bounded_read(&target, MAX_BYTES)? != bytes {
                return Err("comparison_readback_failed".into());
            }
            Ok::<(), String>(())
        });
        if let Err(error) = optional {
            tracing::warn!(%error,"Optional temporal comparison unavailable; RF receipt preserved");
        }
        Ok(stored)
    }
    pub(crate) fn read_temporal_comparison(
        &self,
        eval: &StoredTemporalEvaluation,
        room_revision: u64,
    ) -> Result<serde_json::Value, String> {
        self.validate_context(eval.binding().context())?;
        let Some(path) = path(self, eval, false)? else {
            return Ok(serde_json::json!({"schema":SCHEMA,"state":"not_recorded"}));
        };
        if !comparison_file_present(&path)? {
            return Ok(serde_json::json!({"schema":SCHEMA,"state":"not_recorded"}));
        }
        let bytes = temporal_artifacts::bounded_read(&path, MAX_BYTES)?;
        let envelope: Envelope =
            serde_json::from_slice(&bytes).map_err(|_| "comparison_invalid_metadata")?;
        let c = &envelope.comparison;
        let id = &eval.train().body.identity;
        if envelope.schema != SCHEMA
            || c.schema != SCHEMA
            || envelope.sha256 != sha(c)?
            || c.model_kind != id.artifact_kind
            || c.model_sha256 != id.artifact_sha256
            || c.room_revision != room_revision
            || c.context_digest != id.context_digest
            || c.verified_fit_record_sha256 != id.verified_fit_record_sha256
            || c.training_proof_sha256 != eval.train().sha256
            || c.held_out_evidence_sha256 != eval.held_out().sha256
            || c.complete_record_sha256 != eval.held_out().body.complete_record_sha256
            || c.evaluation_receipt_sha256 != eval.receipt().sha256
            || c.outcomes.len() != 64
            || !c.comparison_only
            || !c.history_only
            || c.integrity_claim != INTEGRITY
            || c.physically_validated
            || c.adoption_authorized
        {
            return Err("comparison_current_evidence_changed".into());
        }
        validate_outcomes(c, eval)?;
        serde_json::to_value(envelope).map_err(|_| "comparison_encoding".into())
    }
}

#[cfg(test)]
mod metadata_data_tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn only_actual_not_found_is_absence_dangling_or_nonregular_is_unavailable() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("comparison.json");
        assert_eq!(comparison_file_present(&path).unwrap(), false);
        std::os::unix::fs::symlink(directory.path().join("missing-target"), &path).unwrap();
        assert!(comparison_file_present(&path).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(comparison_file_present(&path).is_err());
        std::fs::remove_dir(&path).unwrap();
        crate::companion::atomic_private_write(&path, b"SYNTHETIC").unwrap();
        assert_eq!(comparison_file_present(&path).unwrap(), true);
    }
    #[test]
    fn metadata_shape_checks_never_authenticate_per_window_truth() {
        let data = crate::room_learner::training::learning_tests::dataset(8);
        let context = data.context();
        let source = &context.sources[0];
        let mut out = WindowOutcome {
            cohort_sha256: "a".repeat(64),
            trial_commitment: "b".repeat(64),
            session_commitment: "c".repeat(64),
            condition: RoomLabel::Empty,
            role: DataRole::HeldOut,
            provenance: CaptureProvenance::Synthetic,
            decision: PresenceDecision::Abstain,
            occupied_probability: None,
            sources: vec![SourceTiming {
                source_id: source.source_id,
                grid_id: source.grid_id.clone(),
                first_time_ns: "1".into(),
                last_time_ns: "2".into(),
                clock_kind: ClockKind::ReceiverOnly,
                uncertainty_ns: u64::MAX.to_string(),
                frame_commitment_sha256: "d".repeat(64),
            }],
            reference_state: "unknown_no_associated_reference".into(),
            associated_reference_rows: 0,
            reference_commitments: vec![],
        };
        assert!(validate_window_metadata(&out, context, &[CaptureProvenance::Synthetic]).is_ok());
        out.sources[0].grid_id = "different-grid".into();
        assert!(validate_window_metadata(&out, context, &[CaptureProvenance::Synthetic]).is_err());
        out.sources[0].grid_id = source.grid_id.clone();
        out.cohort_sha256 = "not-a-sha".into();
        assert!(validate_window_metadata(&out, context, &[CaptureProvenance::Synthetic]).is_err());
        out.cohort_sha256 = "a".repeat(64);
        out.provenance = CaptureProvenance::LiveReceiverObserved;
        assert!(validate_window_metadata(&out, context, &[CaptureProvenance::Synthetic]).is_err());
        assert_eq!(
            INTEGRITY,
            "owner_private_checksummed_diagnostic_not_authenticated_per_window_truth"
        );
    }
}
