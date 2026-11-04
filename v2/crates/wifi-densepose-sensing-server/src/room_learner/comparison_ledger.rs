//! Opt-in, owner-private comparison provenance. No reference ingest or authority.
use super::*;
use crate::room_learner::{
    model::{PresenceDecision, RoomPrediction},
    observation::{
        CaptureCohort, CaptureProvenance, ClockKind, RoomLabel, MAX_COHORTS, MAX_SOURCES,
        MAX_WINDOW_NS,
    },
    training::{BinaryConfusion, EvaluationReport},
};
use std::{collections::HashSet, fs, io::Read};

const SCHEMA: &str = "ruview.room-learner.comparison-ledger.v1";
const MAX_BYTES: usize = 1_048_576;
const DIRECTORY: &str = "comparison-ledgers-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ComparisonLedgerSummary {
    pub schema: String,
    pub ledger_sha256: String,
    pub receipt_sha256: String,
    pub cohort_count: usize,
    pub evaluated_at_unix_ms: u64,
    pub comparison_only: bool,
    pub physically_validated: bool,
    pub adoption_authorized: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct SourceTiming {
    source_id: u8,
    grid_id: String,
    // Decimal strings preserve full receiver/source nanoseconds through JSON.
    first_time_ns: String,
    last_time_ns: String,
    clock_kind: ClockKind,
    uncertainty_ns: String,
    frame_commitment_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Outcome {
    cohort_sha256: String,
    cohort_id_commitment: String,
    trial_commitment: String,
    session_commitment: String,
    role: DataRole,
    condition: RoomLabel,
    provenance: CaptureProvenance,
    decision: PresenceDecision,
    occupied_probability: Option<f32>,
    sources: Vec<SourceTiming>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ComparisonLedger {
    schema: String,
    owner_scope: String,
    installation_id: String,
    room_digest: String,
    context_digest: String,
    model_sha256: String,
    model_version: u32,
    evaluation_receipt_sha256: String,
    dataset_sha256: String,
    training_data_sha256: String,
    managed_training_proof_sha256: Option<String>,
    sealed_phase_fence_sha256: Option<String>,
    role: DataRole,
    evaluated_at_unix_ms: u64,
    historical_train_lineage_checked: bool,
    measurement_clock_available: bool,
    outcomes: Vec<Outcome>,
    comparison_only: bool,
    physically_validated: bool,
    adoption_authorized: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    ledger_sha256: String,
    ledger: ComparisonLedger,
}
fn bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "comparison encoding failed")?;
    if bytes.len() > MAX_BYTES {
        return Err("comparison metadata budget exceeded".into());
    }
    Ok(bytes)
}
fn commitment<T: Serialize>(domain: &str, value: &T) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(serde_json::to_vec(value).map_err(|_| "comparison commitment failed")?);
    Ok(format!("{:x}", hasher.finalize()))
}
fn decimal(value: &str) -> Result<u64, String> {
    let number = value
        .parse::<u64>()
        .map_err(|_| "invalid comparison nanoseconds")?;
    if number.to_string() != value {
        return Err("noncanonical comparison nanoseconds".into());
    }
    Ok(number)
}
// Same bounded, no-follow pattern as encrypted recovery, with its own explicit
// budget. The global 64KiB private_read limit and all its callers stay unchanged.
fn read(path: &Path) -> Result<Vec<u8>, String> {
    fn metadata(meta: &fs::Metadata) -> Result<(), String> {
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_BYTES as u64 {
            return Err("unsafe comparison file".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err("unsafe comparison permissions".into());
            }
        }
        Ok(())
    }
    metadata(&fs::symlink_metadata(path).map_err(|_| "comparison file unavailable")?)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        #[cfg(target_os = "macos")]
        options.custom_flags(0x100);
        #[cfg(target_os = "linux")]
        options.custom_flags(0x20000);
    }
    let file = options
        .open(path)
        .map_err(|_| "comparison file unavailable")?;
    metadata(&file.metadata().map_err(|_| "comparison file unavailable")?)?;
    let mut output = Vec::new();
    file.take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut output)
        .map_err(|_| "comparison read failed")?;
    if output.len() > MAX_BYTES {
        return Err("comparison metadata budget exceeded".into());
    }
    Ok(output)
}
fn install(path: &Path, content: &[u8]) -> Result<(), String> {
    if content.len() > MAX_BYTES {
        return Err("comparison metadata budget exceeded".into());
    }
    if regular_or_missing(path)? {
        return if read(path)? == content {
            Ok(())
        } else {
            Err("immutable comparison collision/corruption".into())
        };
    }
    let directory = path.parent().ok_or("comparison directory unavailable")?;
    let staging = directory.join(format!("comparison-stage-{}", crate::companion::secret()));
    atomic_private_write(&staging, content).map_err(|_| "comparison write failed")?;
    let installed = fs::hard_link(&staging, path);
    let cleanup = fs::remove_file(&staging);
    match installed {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            if read(path)? != content {
                return Err("immutable comparison collision/corruption".into());
            }
        }
        Err(_) => return Err("comparison install failed".into()),
    }
    cleanup.map_err(|_| "comparison staging cleanup failed")?;
    fs::File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| "comparison sync failed".into())
}
fn is_occupied(label: RoomLabel) -> bool {
    matches!(label, RoomLabel::OccupiedStatic | RoomLabel::OccupiedMoving)
}
fn observe(confusion: &mut BinaryConfusion, row: &Outcome) {
    let occupied = is_occupied(row.condition);
    if row.decision == PresenceDecision::Abstain || row.occupied_probability.is_none() {
        if occupied {
            confusion.abstained_occupied += 1;
            if row.occupied_probability.is_none() {
                confusion.missing_occupied += 1;
            }
        } else {
            confusion.abstained_empty += 1;
            if row.occupied_probability.is_none() {
                confusion.missing_empty += 1;
            }
        }
    } else {
        match (occupied, row.decision) {
            (false, PresenceDecision::Empty) => confusion.true_empty += 1,
            (false, PresenceDecision::Occupied) => confusion.false_occupied += 1,
            (true, PresenceDecision::Empty) => confusion.false_empty += 1,
            (true, PresenceDecision::Occupied) => confusion.true_occupied += 1,
            _ => unreachable!(),
        }
    }
}
fn verify_counts(rows: &[Outcome], report: &EvaluationReport) -> Result<(), String> {
    let mut total = BinaryConfusion::default();
    let mut conditions: [BinaryConfusion; 4] = Default::default();
    let mut samples = [0usize; 4];
    let mut probability_counts = [0usize; 4];
    let mut losses = [0.0f64; 4];
    let mut total_loss = 0.0f64;
    let mut trials = HashSet::new();
    let mut sessions = HashSet::new();
    let mut condition_trials: [HashSet<&str>; 4] = Default::default();
    let mut condition_sessions: [HashSet<&str>; 4] = Default::default();
    let labels = [
        RoomLabel::Empty,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ];
    for row in rows {
        let index = labels
            .iter()
            .position(|label| *label == row.condition)
            .ok_or("unknown comparison condition")?;
        observe(&mut total, row);
        observe(&mut conditions[index], row);
        samples[index] += 1;
        trials.insert(row.trial_commitment.as_str());
        sessions.insert(row.session_commitment.as_str());
        condition_trials[index].insert(row.trial_commitment.as_str());
        condition_sessions[index].insert(row.session_commitment.as_str());
        if let Some(p) = row.occupied_probability {
            if !p.is_finite() || !(0.0..=1.0).contains(&p) {
                return Err("invalid comparison probability".into());
            }
            let p = p.clamp(1e-6, 1.0 - 1e-6) as f64;
            let loss = -if is_occupied(row.condition) {
                p.ln()
            } else {
                (1.0 - p).ln()
            };
            losses[index] += loss;
            total_loss += loss;
            probability_counts[index] += 1;
        }
    }
    let probabilities = probability_counts.iter().sum::<usize>();
    let mean = |loss: f64, count: usize| {
        if count == 0 {
            None
        } else {
            Some((loss / count as f64) as f32)
        }
    };
    if rows.len() != report.samples
        || total != report.confusion
        || trials.len() != report.declared_trial_count
        || sessions.len() != report.declared_session_count
        || probabilities != report.binary_cross_entropy_samples
        || mean(total_loss, probabilities) != report.binary_cross_entropy
    {
        return Err("comparison outcomes differ from evaluation totals".into());
    }
    for (i, condition) in report.per_condition.iter().enumerate() {
        if condition.condition != labels[i]
            || samples[i] != condition.samples
            || conditions[i] != condition.confusion
            || probability_counts[i] != condition.binary_cross_entropy_samples
            || mean(losses[i], probability_counts[i]) != condition.binary_cross_entropy
            || condition_trials[i].len() != condition.declared_trial_count
            || condition_sessions[i].len() != condition.declared_session_count
        {
            return Err("comparison outcomes differ from condition totals".into());
        }
    }
    Ok(())
}
impl ComparisonLedger {
    fn summary(&self) -> Result<ComparisonLedgerSummary, String> {
        Ok(ComparisonLedgerSummary {
            schema: SCHEMA.into(),
            ledger_sha256: digest(&bytes(self)?),
            receipt_sha256: self.evaluation_receipt_sha256.clone(),
            cohort_count: self.outcomes.len(),
            evaluated_at_unix_ms: self.evaluated_at_unix_ms,
            comparison_only: true,
            physically_validated: false,
            adoption_authorized: false,
        })
    }
    fn validate(
        &self,
        store: &ArtifactStore,
        model: &RoomModel,
        receipt: &EvaluationReceipt,
    ) -> Result<(), String> {
        receipt.validate(model, receipt.role)?;
        if self.schema != SCHEMA
            || self.owner_scope != receipt.owner_scope
            || self.installation_id != receipt.installation_id
            || self.room_digest != receipt.room_digest
            || self.context_digest != receipt.context_digest
            || self.model_sha256 != receipt.model_sha256
            || self.model_version != receipt.model_version
            || self.evaluation_receipt_sha256 != receipt.sha256()?
            || self.dataset_sha256 != receipt.dataset_sha256
            || self.training_data_sha256 != receipt.training_data_sha256
            || self.role != receipt.role
            || self.evaluated_at_unix_ms != receipt.evaluated_at_unix_ms
            || self.historical_train_lineage_checked != receipt.train_lineage_checked
            || self.measurement_clock_available != receipt.measurement_clock_available
            || !self.comparison_only
            || self.physically_validated
            || self.adoption_authorized
            || self.outcomes.is_empty()
            || self.outcomes.len() > MAX_COHORTS
        {
            return Err("comparison receipt binding mismatch".into());
        }
        match (
            &self.managed_training_proof_sha256,
            &self.sealed_phase_fence_sha256,
        ) {
            (None, None) => {}
            (Some(proof_sha), Some(fence_sha)) => {
                if !hex_digest(proof_sha) || !hex_digest(fence_sha) {
                    return Err("invalid comparison lineage digest".into());
                }
                let proof = store
                    .load_managed_training_proof(model)?
                    .ok_or("comparison managed proof unavailable")?;
                if proof.proof_sha256 != *proof_sha {
                    return Err("comparison managed proof mismatch".into());
                }
                store.load_managed_capture_fence_by_sha(model, &proof, fence_sha)?;
            }
            _ => return Err("partial comparison lineage claim".into()),
        }
        let mut ids = HashSet::new();
        let mut clocks = HashSet::new();
        let mut provenance = HashSet::new();
        for row in &self.outcomes {
            if row.role != self.role
                || !receipt.provenance.contains(&row.provenance)
                || [
                    &row.cohort_sha256,
                    &row.cohort_id_commitment,
                    &row.trial_commitment,
                    &row.session_commitment,
                ]
                .iter()
                .any(|id| !hex_digest(id))
                || !ids.insert(&row.cohort_id_commitment)
                || row.sources.len() != model.context().sources.len()
                || row.sources.is_empty()
                || row.sources.len() > MAX_SOURCES
            {
                return Err("invalid comparison outcome provenance".into());
            }
            provenance.insert(format!("{:?}", row.provenance));
            for (source, spec) in row.sources.iter().zip(&model.context().sources) {
                let first = decimal(&source.first_time_ns)?;
                let last = decimal(&source.last_time_ns)?;
                let uncertainty = decimal(&source.uncertainty_ns)?;
                if source.source_id != spec.source_id
                    || source.grid_id != spec.grid_id
                    || !hex_digest(&source.frame_commitment_sha256)
                    || first == 0
                    || last <= first
                    || last - first > MAX_WINDOW_NS
                    || (source.clock_kind == ClockKind::SourceMeasurement
                        && uncertainty > crate::room_learner::observation::MAX_CLOCK_UNCERTAINTY_NS)
                    || !receipt.clock_claims.contains(&source.clock_kind)
                {
                    return Err("invalid comparison source timing".into());
                }
                clocks.insert(format!("{:?}", source.clock_kind));
            }
        }
        if clocks.len() != receipt.clock_claims.len()
            || provenance.len() != receipt.provenance.len()
        {
            return Err("comparison clock/provenance coverage differs".into());
        }
        verify_counts(&self.outcomes, &receipt.report)
    }
}
impl ArtifactStore {
    fn comparison_directory(
        &self,
        model: &RoomModel,
        role: DataRole,
        create: bool,
    ) -> Result<Option<PathBuf>, String> {
        let Some(parent) = self.evaluation_directory(model, role, create)? else {
            return Ok(None);
        };
        private_subdirectory(&parent, DIRECTORY, create)
    }
    pub(crate) fn load_comparison_ledger_summary(
        &self,
        model: &RoomModel,
        role: DataRole,
        receipt_sha: &str,
    ) -> Result<Option<ComparisonLedgerSummary>, String> {
        self.read_comparison_ledger(model, role, receipt_sha)?
            .map(|ledger| ledger.summary())
            .transpose()
    }
    fn read_comparison_ledger(
        &self,
        model: &RoomModel,
        role: DataRole,
        receipt_sha: &str,
    ) -> Result<Option<ComparisonLedger>, String> {
        let receipt = self.load_exact_evaluation(model, role, receipt_sha)?;
        let Some(directory) = self.comparison_directory(model, role, false)? else {
            return Ok(None);
        };
        let path = directory.join(format!("{receipt_sha}.json"));
        if !regular_or_missing(&path)? {
            return Ok(None);
        }
        let envelope: Envelope =
            serde_json::from_slice(&read(&path)?).map_err(|_| "comparison decode failed")?;
        if envelope.schema != SCHEMA
            || !hex_digest(&envelope.ledger_sha256)
            || envelope.ledger_sha256 != digest(&bytes(&envelope.ledger)?)
        {
            return Err("comparison integrity mismatch".into());
        }
        envelope.ledger.validate(self, model, &receipt)?;
        Ok(Some(envelope.ledger))
    }
    pub(super) fn save_comparison_ledger(
        &self,
        model: &RoomModel,
        dataset: &ValidatedDataset,
        receipt: &EvaluationReceipt,
        predictions: Vec<RoomPrediction>,
    ) -> Result<ComparisonLedgerSummary, String> {
        self.validate_context(model.context())?;
        if dataset.context() != model.context()
            || !matches!(receipt.role, DataRole::Validation | DataRole::HeldOut)
        {
            return Err("comparison dataset/candidate mismatch".into());
        }
        let rows = if receipt.role == DataRole::HeldOut {
            dataset.held_out()
        } else {
            dataset.validation()
        };
        if data_sha256(rows)? != receipt.dataset_sha256 || rows.len() != predictions.len() {
            return Err("comparison retained data mismatch".into());
        }
        let mut outcomes = Vec::with_capacity(rows.len());
        for (row, prediction) in rows.iter().zip(predictions) {
            outcomes.push(outcome(row, prediction)?);
        }
        // A retained exact dataset supplies a stronger retry oracle than aggregate
        // counts: each outcome, timestamp and frame commitment must reproduce.
        // Appended fences or lost temporary Train rows never rewrite old claims.
        if let Some(existing) =
            self.read_comparison_ledger(model, receipt.role, &receipt.sha256()?)?
        {
            if existing.outcomes != outcomes {
                return Err("comparison outcomes differ from retained data".into());
            }
            return existing.summary();
        }
        let (proof, fence) = match self.load_managed_training_proof(model)? {
            Some(proof) => {
                let fence = self.load_managed_capture_fence(model, &proof)?;
                (Some(proof.proof_sha256), Some(fence.fence_sha256))
            }
            None => (None, None),
        };
        let ledger = ComparisonLedger {
            schema: SCHEMA.into(),
            owner_scope: receipt.owner_scope.clone(),
            installation_id: receipt.installation_id.clone(),
            room_digest: receipt.room_digest.clone(),
            context_digest: receipt.context_digest.clone(),
            model_sha256: receipt.model_sha256.clone(),
            model_version: receipt.model_version,
            evaluation_receipt_sha256: receipt.sha256()?,
            dataset_sha256: receipt.dataset_sha256.clone(),
            training_data_sha256: receipt.training_data_sha256.clone(),
            managed_training_proof_sha256: proof,
            sealed_phase_fence_sha256: fence,
            role: receipt.role,
            evaluated_at_unix_ms: receipt.evaluated_at_unix_ms,
            historical_train_lineage_checked: receipt.train_lineage_checked,
            measurement_clock_available: receipt.measurement_clock_available,
            outcomes,
            comparison_only: true,
            physically_validated: false,
            adoption_authorized: false,
        };
        ledger.validate(self, model, receipt)?;
        let summary = ledger.summary()?;
        let envelope = Envelope {
            schema: SCHEMA.into(),
            ledger_sha256: summary.ledger_sha256.clone(),
            ledger,
        };
        let directory = self
            .comparison_directory(model, receipt.role, true)?
            .ok_or("comparison directory unavailable")?;
        install(
            &directory.join(format!("{}.json", receipt.sha256()?)),
            &bytes(&envelope)?,
        )?;
        self.load_comparison_ledger_summary(model, receipt.role, &receipt.sha256()?)?
            .ok_or("comparison readback unavailable".into())
    }
}
fn outcome(row: &CaptureCohort, prediction: RoomPrediction) -> Result<Outcome, String> {
    let mut sources = Vec::with_capacity(row.views.len());
    for view in &row.views {
        sources.push(SourceTiming {
            source_id: view.source_id,
            grid_id: view.grid_id.clone(),
            first_time_ns: view.first_time_ns.to_string(),
            last_time_ns: view.last_time_ns.to_string(),
            clock_kind: view.clock.kind,
            uncertainty_ns: view.clock.uncertainty_ns.to_string(),
            frame_commitment_sha256: commitment(
                "ruview.comparison.frames.v1",
                &(&view.frame_ids, &view.sequence_ids),
            )?,
        });
    }
    // Valid RF cohorts resolve views by source ID, independent of input order.
    // Canonicalize only this metadata; retain the original cohort/data hashes.
    sources.sort_by_key(|source| source.source_id);
    Ok(Outcome {
        cohort_sha256: data_sha256(std::slice::from_ref(row))?,
        cohort_id_commitment: commitment("ruview.comparison.cohort-id.v1", &row.id)?,
        trial_commitment: commitment("ruview.comparison.trial.v1", &row.trial_id)?,
        session_commitment: commitment("ruview.comparison.session.v1", &row.session_id)?,
        role: row.role,
        condition: row.label,
        provenance: row.provenance,
        decision: prediction.decision,
        occupied_probability: prediction.occupied_probability,
        sources,
    })
}

#[cfg(test)]
#[path = "comparison_ledger_tests.rs"]
mod tests;
