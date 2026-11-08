//! Immutable whole-Temporal metadata in the SAME ArtifactStore owner root.
//! Raw decoder returns DATA only. Protected source/result is required to write
//! or claim a current evaluation; no parallel DB/key/latest pointer is added.
use super::*;
use crate::room_learner::{
    observation::DataRole,
    temporal_evaluation as data,
    temporal_model_input::managed::execution::full_run::{
        TemporalEvaluationBinding, VerifiedTemporalEvaluationResult,
        VerifiedTemporalEvaluationSnapshot, VerifiedTemporalTrainSnapshot,
    },
};
use std::sync::atomic::{AtomicBool, Ordering};
const CHILD: &str = "managed-evaluation";
pub(crate) struct StoredTemporalEvaluation {
    binding: TemporalEvaluationBinding,
    train: data::TemporalTrainProofData,
    held: data::TemporalHeldOutEvidenceData,
    receipt: data::TemporalEvaluationReceiptData,
    lookup_sha256: String,
}
impl StoredTemporalEvaluation {
    pub(crate) fn binding(&self) -> &TemporalEvaluationBinding {
        &self.binding
    }
    pub(crate) fn train(&self) -> &data::TemporalTrainProofData {
        &self.train
    }
    pub(crate) fn held_out(&self) -> &data::TemporalHeldOutEvidenceData {
        &self.held
    }
    pub(crate) fn receipt(&self) -> &data::TemporalEvaluationReceiptData {
        &self.receipt
    }
    pub(crate) fn lookup_sha256(&self) -> &str {
        &self.lookup_sha256
    }
}
fn lookup(source: &VerifiedTemporalEvaluationSnapshot) -> Result<String, String> {
    let identity = &source.proof().body.identity;
    crate::room_learner::qualification::hash(&(
        data::EVALUATION_SCHEMA,
        "lookup",
        &identity.artifact_kind,
        &identity.artifact_sha256,
        &source.proof().sha256,
        &source.held_out().sha256,
        DataRole::HeldOut,
    ))
}
fn allowed(
    cancel: &AtomicBool,
    clock: &mut dyn FnMut() -> Result<u64, String>,
) -> Result<u64, String> {
    if cancel.load(Ordering::Acquire) {
        return Err("temporal_evaluation_cancelled".into());
    }
    clock()
}
impl ArtifactStore {
    pub(crate) fn persist_current_temporal_train_proof(
        &self,
        source: &VerifiedTemporalTrainSnapshot,
        cancel: &AtomicBool,
        clock: &mut dyn FnMut() -> Result<u64, String>,
        io: &mut dyn FnMut(&'static str) -> Result<(), String>,
    ) -> Result<data::TemporalTrainProofData, String> {
        source.require_same_store(self)?;
        allowed(cancel, clock)?;
        let directory = self
            .temporal_evaluation_directory(source.model().context(), true)?
            .ok_or("temporal_evaluation_directory_unavailable")?;
        let bytes = source.proof().encode_data()?;
        let path = directory.join(format!("train-proof-{}.json", source.proof().sha256));
        temporal_artifacts::immutable_with_io(&path, &bytes, &mut |_| {
            io("train_proof_install_before_directory_fsync")
        })?;
        allowed(cancel, clock)?;
        let read = data::TemporalTrainProofData::decode_data(&temporal_artifacts::bounded_read(
            &path,
            data::MAX_METADATA_BYTES,
        )?)?;
        if read != *source.proof() {
            return Err("temporal_train_proof_readback_changed".into());
        }
        temporal_artifacts::recertify_immutable(
            &path,
            &bytes,
            data::MAX_METADATA_BYTES,
            &mut |stage| {
                allowed(cancel, clock)?;
                io(if stage == "before:file_fsync" {
                    "train_proof_recertify_before_file_fsync"
                } else {
                    "train_proof_recertify_before_directory_fsync"
                })
            },
        )?;
        self.recertify_temporal_metadata_directories(&directory, cancel, clock, io)?;
        Ok(read)
    }
    pub(super) fn temporal_evaluation_directory(
        &self,
        context: &CaptureContext,
        create: bool,
    ) -> Result<Option<PathBuf>, String> {
        let Some(root) = self.temporal_directory(context, create)? else {
            return Ok(None);
        };
        let directory = root.join(CHILD);
        if !temporal_artifacts::directory(&directory, create)? {
            return Ok(None);
        }
        Ok(Some(directory))
    }
    /// The current protected Complete snapshot supplies exact paths/identities.
    /// A generic imported receipt/proof never reaches this return constructor.
    pub(super) fn load_current_temporal_evaluation(
        &self,
        source: &VerifiedTemporalEvaluationSnapshot,
    ) -> Result<Option<StoredTemporalEvaluation>, String> {
        source.require_same_store(self)?;
        let Some(directory) =
            self.temporal_evaluation_directory(source.model().context(), false)?
        else {
            return Ok(None);
        };
        let key = lookup(source)?;
        let path = directory.join(format!("evaluation-{key}.json"));
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("temporal_evaluation_file_unavailable".into()),
            Ok(_) => {}
        }
        let receipt = data::TemporalEvaluationReceiptData::decode_data(
            &temporal_artifacts::bounded_read(&path, data::MAX_METADATA_BYTES)?,
        )?;
        let train = data::TemporalTrainProofData::decode_data(&temporal_artifacts::bounded_read(
            &directory.join(format!("train-proof-{}.json", source.proof().sha256)),
            data::MAX_METADATA_BYTES,
        )?)?;
        let held =
            data::TemporalHeldOutEvidenceData::decode_data(&temporal_artifacts::bounded_read(
                &directory.join(format!(
                    "heldout-evidence-{}.json",
                    source.held_out().sha256
                )),
                data::MAX_METADATA_BYTES,
            )?)?;
        receipt.validate_data_links(&train, &held)?;
        if train != *source.proof() || held != *source.held_out() {
            return Err("temporal_evaluation_current_source_changed".into());
        }
        Ok(Some(StoredTemporalEvaluation {
            binding: source.binding().clone(),
            train,
            held,
            receipt,
            lookup_sha256: key,
        }))
    }
    /// Existing compute and immutable IO; exact retry loads the previous record
    /// BEFORE inference, preserving the ORIGINAL evaluation timestamp/report.
    pub(crate) fn evaluate_temporal_snapshot(
        &self,
        source: VerifiedTemporalEvaluationSnapshot,
        cancel: &AtomicBool,
        clock: &mut dyn FnMut() -> Result<u64, String>,
        io: &mut dyn FnMut(&'static str) -> Result<(), String>,
    ) -> Result<StoredTemporalEvaluation, String> {
        allowed(cancel, clock)?;
        source.require_same_store(self)?;
        if let Some(existing) = self.load_current_temporal_evaluation(&source)? {
            let now = allowed(cancel, clock)?;
            let evaluated = existing
                .receipt()
                .body
                .evaluated_at_receiver_ns
                .parse::<u64>()
                .map_err(|_| "temporal_evaluation_data_integer")?;
            if now < evaluated {
                return Err("temporal_evaluation_stored_clock_reversed".into());
            }
            return self.recertify_current_temporal_evaluation(&source, existing, cancel, clock, io);
        }
        let key = lookup(&source)?;
        let computed = source.compute_verified(cancel, clock)?;
        self.persist_temporal_evaluation_result(&source, &computed, &key, cancel, clock, io)
    }
    fn persist_temporal_evaluation_result(
        &self,
        source: &VerifiedTemporalEvaluationSnapshot,
        result: &VerifiedTemporalEvaluationResult,
        key: &str,
        cancel: &AtomicBool,
        clock: &mut dyn FnMut() -> Result<u64, String>,
        io: &mut dyn FnMut(&'static str) -> Result<(), String>,
    ) -> Result<StoredTemporalEvaluation, String> {
        source.require_same_store(self)?;
        result.require_same_store(self)?;
        if result.binding() != source.binding() {
            return Err("temporal_evaluation_operation_changed".into());
        }
        allowed(cancel, clock)?;
        let directory = self
            .temporal_evaluation_directory(source.model().context(), true)?
            .ok_or("temporal_evaluation_directory_unavailable")?;
        let writes = [
            (
                format!("train-proof-{}.json", result.train().sha256),
                result.train().encode_data()?,
            ),
            (
                format!("heldout-evidence-{}.json", result.held_out().sha256),
                result.held_out().encode_data()?,
            ),
            (
                format!("evaluation-{key}.json"),
                result.receipt().encode_data()?,
            ),
        ];
        for (name, bytes) in writes {
            allowed(cancel, clock)?;
            // Same immutable private stage/link/fsync primitive as whole models.
            let stage = if name.starts_with("evaluation-") {
                "evaluation_receipt_install_before_directory_fsync"
            } else if name.starts_with("train-proof-") {
                "train_proof_install_before_directory_fsync"
            } else {
                "heldout_evidence_install_before_directory_fsync"
            };
            temporal_artifacts::immutable_with_io(&directory.join(&name), &bytes, &mut |_| {
                io(stage)
            })?;
            allowed(cancel, clock)?;
            if temporal_artifacts::bounded_read(&directory.join(name), data::MAX_METADATA_BYTES)?
                != bytes
            {
                return Err("temporal_evaluation_immutable_readback_changed".into());
            }
        }
        allowed(cancel, clock)?;
        let stored = self
            .load_current_temporal_evaluation(source)?
            .ok_or("temporal_evaluation_readback_missing")?;
        self.recertify_current_temporal_evaluation(source, stored, cancel, clock, io)
    }
    fn recertify_current_temporal_evaluation(
        &self,
        source: &VerifiedTemporalEvaluationSnapshot,
        stored: StoredTemporalEvaluation,
        cancel: &AtomicBool,
        clock: &mut dyn FnMut() -> Result<u64, String>,
        io: &mut dyn FnMut(&'static str) -> Result<(), String>,
    ) -> Result<StoredTemporalEvaluation, String> {
        source.require_same_store(self)?;
        allowed(cancel, clock)?;
        let directory = self
            .temporal_evaluation_directory(source.model().context(), false)?
            .ok_or("temporal_evaluation_directory_unavailable")?;
        let records = [
            (
                format!("train-proof-{}.json", stored.train.sha256),
                stored.train.encode_data()?,
                "train",
            ),
            (
                format!("heldout-evidence-{}.json", stored.held.sha256),
                stored.held.encode_data()?,
                "held",
            ),
            (
                format!("evaluation-{}.json", stored.lookup_sha256),
                stored.receipt.encode_data()?,
                "evaluation",
            ),
        ];
        for (name, bytes, kind) in records {
            temporal_artifacts::recertify_immutable(
                &directory.join(name),
                &bytes,
                data::MAX_METADATA_BYTES,
                &mut |stage| {
                    allowed(cancel, clock)?;
                    io(match (kind, stage) {
                        ("evaluation", "before:file_fsync") => {
                            "evaluation_receipt_recertify_before_file_fsync"
                        }
                        ("evaluation", _) => "evaluation_receipt_recertify_before_directory_fsync",
                        ("train", "before:file_fsync") => "train_proof_recertify_before_file_fsync",
                        ("train", _) => "train_proof_recertify_before_directory_fsync",
                        (_, "before:file_fsync") => "heldout_evidence_recertify_before_file_fsync",
                        _ => "heldout_evidence_recertify_before_directory_fsync",
                    })
                },
            )?;
        }
        self.recertify_temporal_metadata_directories(&directory, cancel, clock, io)?;
        allowed(cancel, clock)?;
        let read = self
            .load_current_temporal_evaluation(source)?
            .ok_or("temporal_evaluation_readback_missing")?;
        if read.train != stored.train || read.held != stored.held || read.receipt != stored.receipt
        {
            return Err("temporal_evaluation_recertify_changed".into());
        }
        Ok(read)
    }
    fn recertify_temporal_metadata_directories(
        &self,
        directory: &Path,
        cancel: &AtomicBool,
        clock: &mut dyn FnMut() -> Result<u64, String>,
        io: &mut dyn FnMut(&'static str) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut path = directory;
        loop {
            allowed(cancel, clock)?;
            temporal_artifacts::directory(path, false)?;
            io("metadata_directory_chain_before_fsync")?;
            std::fs::File::open(path)
                .and_then(|f| f.sync_all())
                .map_err(|_| "temporal_evaluation_directory_sync_failed")?;
            if path == self.root {
                break;
            }
            path = path
                .parent()
                .ok_or("temporal_evaluation_directory_outside_owner")?;
            if !path.starts_with(&self.root) {
                return Err("temporal_evaluation_directory_outside_owner".into());
            }
        }
        Ok(())
    }
    /// Explicit inventory for privacy/reporting; contents include labels,
    /// run/trial and receiver/frame/sequence lineage metadata, not aggregates only.
    pub(crate) fn temporal_evaluation_inventory(
        &self,
        context: &CaptureContext,
    ) -> Result<Vec<(String, u64)>, String> {
        let Some(directory) = self.temporal_evaluation_directory(context, false)? else {
            return Ok(vec![]);
        };
        let mut files = vec![];
        for entry in
            std::fs::read_dir(directory).map_err(|_| "temporal_evaluation_inventory_unavailable")?
        {
            if files.len() >= 512 {
                return Err("temporal_evaluation_inventory_too_many_records".into());
            }
            let entry = entry.map_err(|_| "temporal_evaluation_inventory_unavailable")?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !(name.starts_with("train-proof-")
                || name.starts_with("heldout-evidence-")
                || name.starts_with("evaluation-")
                || name.starts_with("latest-assessment-")
                || name.starts_with("assessment-")
                || name.starts_with("qualification-")
                || name.starts_with("temporal-stage-"))
            {
                return Err("temporal_evaluation_inventory_unknown_file".into());
            }
            let bytes = temporal_artifacts::bounded_read(&entry.path(), data::MAX_METADATA_BYTES)?;
            files.push((name, bytes.len() as u64));
        }
        files.sort();
        Ok(files)
    }
}

pub(crate) struct StoredTemporalAssessment {
    binding: TemporalEvaluationBinding,
    pub(crate) evaluation: StoredTemporalEvaluation,
    pub(crate) qualification:
        Option<crate::room_learner::temporal_qualification::TemporalQualificationEnvelope>,
    pub(crate) assessment: crate::room_learner::temporal_qualification::TemporalAssessmentEnvelope,
}
impl StoredTemporalAssessment {
    pub(crate) fn binding(&self) -> &TemporalEvaluationBinding {
        &self.binding
    }
}
impl ArtifactStore {
    pub(crate) fn qualify_temporal_snapshot(
        &self,
        source: &VerifiedTemporalEvaluationSnapshot,
        cancel: &AtomicBool,
        clock: &mut dyn FnMut() -> Result<u64, String>,
        io: &mut dyn FnMut(&'static str) -> Result<(), String>,
    ) -> Result<StoredTemporalAssessment, String> {
        use crate::room_learner::temporal_qualification::*;
        source.require_same_store(self)?;
        allowed(cancel, clock)?;
        // A qualification operation consumes only an already durable evaluation;
        // it does not invent or silently recompute an evaluation report.
        let existing = self
            .load_current_temporal_evaluation(&source)?
            .ok_or("temporal_evaluation_required")?;
        let evaluation =
            self.recertify_current_temporal_evaluation(&source, existing, cancel, clock, io)?;
        let directory = self
            .temporal_evaluation_directory(source.model().context(), false)?
            .ok_or("temporal_evaluation_directory_unavailable")?;
        let key = crate::room_learner::qualification::hash(&(
            crate::room_learner::temporal_wire::ASSESSMENT_SCHEMA,
            &evaluation.receipt().sha256,
            crate::room_learner::temporal_wire::TemporalQualificationPolicyWire::current(),
        ))?;
        let path = directory.join(format!("assessment-{key}.json"));
        let (assessment, qualification) = match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let computed = ComputedTemporalAssessment::issue_current(
                    &source,
                    &evaluation,
                    allowed(cancel, clock)?,
                )?;
                if let Some(q) = &computed.qualification {
                    let bytes =
                        serde_json::to_vec(q).map_err(|_| "temporal_qualification_encoding")?;
                    temporal_artifacts::immutable_with_io(
                        &directory.join(format!("qualification-{}.json", q.receipt_sha256)),
                        &bytes,
                        &mut |_| {
                            allowed(cancel, clock)?;
                            io("qualification_install_before_directory_fsync")
                        },
                    )?;
                }
                let bytes = serde_json::to_vec(&computed.assessment)
                    .map_err(|_| "temporal_assessment_encoding")?;
                temporal_artifacts::immutable_with_io(&path, &bytes, &mut |_| {
                    allowed(cancel, clock)?;
                    io("assessment_install_before_directory_fsync")
                })?;
                (computed.assessment, computed.qualification)
            }
            Err(_) => return Err("temporal_assessment_unavailable".into()),
            Ok(_) => {
                let assessment: TemporalAssessmentEnvelope = serde_json::from_slice(
                    &temporal_artifacts::bounded_read(&path, data::MAX_METADATA_BYTES)?,
                )
                .map_err(|_| "temporal_assessment_encoding")?;
                let qualification = match &assessment.receipt.summary.qualification_sha256 {
                    Some(sha) => {
                        if !hex_digest(sha) {
                            return Err("temporal_qualification_digest".into());
                        }
                        Some(
                            serde_json::from_slice(&temporal_artifacts::bounded_read(
                                &directory.join(format!("qualification-{sha}.json")),
                                data::MAX_METADATA_BYTES,
                            )?)
                            .map_err(|_| "temporal_qualification_encoding")?,
                        )
                    }
                    None => None,
                };
                (assessment, qualification)
            }
        };
        assessment.validate(&evaluation, qualification.as_ref())?;
        if allowed(cancel, clock)? / 1_000_000 < assessment.receipt.summary.assessed_at_unix_ms {
            return Err("temporal_assessment_clock_reversed".into());
        }
        let mut records = vec![(
            path,
            serde_json::to_vec(&assessment).map_err(|_| "temporal_assessment_encoding")?,
        )];
        if let Some(q) = &qualification {
            records.push((
                directory.join(format!("qualification-{}.json", q.receipt_sha256)),
                serde_json::to_vec(q).map_err(|_| "temporal_qualification_encoding")?,
            ));
        }
        for (path, bytes) in records {
            temporal_artifacts::recertify_immutable(
                &path,
                &bytes,
                data::MAX_METADATA_BYTES,
                &mut |_| {
                    allowed(cancel, clock)?;
                    io("assessment_recertify_before_sync")
                },
            )?;
        }
        self.recertify_temporal_metadata_directories(&directory, cancel, clock, io)?;
        allowed(cancel, clock)?;
        let latest = serde_json::json!({"schema":"ruview.room-learner.temporal-current-assessment.v1","model_sha256":source.binding().model_sha256(),"context_digest":source.binding().context_digest(),"assessment_sha256":assessment.receipt_sha256,"assessment_lookup_sha256":key});
        allowed(cancel, clock)?;
        atomic_private_write(
            &directory.join(format!(
                "latest-assessment-{}.json",
                source.binding().model_sha256()
            )),
            &serde_json::to_vec(&latest).map_err(|_| "temporal_assessment_encoding")?,
        )
        .map_err(|_| "temporal_assessment_pointer_write")?;
        allowed(cancel, clock)?;
        Ok(StoredTemporalAssessment {
            binding: source.binding().clone(),
            evaluation,
            qualification,
            assessment,
        })
    }
}

impl ArtifactStore {
    pub(crate) fn reconcile_temporal_snapshot(
        &self,
        source: VerifiedTemporalEvaluationSnapshot,
        cancel: &AtomicBool,
        clock: &mut dyn FnMut() -> Result<u64, String>,
        io: &mut dyn FnMut(&'static str) -> Result<(), String>,
    ) -> Result<
        (
            data::TemporalTrainProofData,
            Option<StoredTemporalEvaluation>,
        ),
        String,
    > {
        source.require_same_store(self)?;
        let proof =
            self.persist_current_temporal_train_proof(source.train_snapshot(), cancel, clock, io)?;
        let existing = self.load_current_temporal_evaluation(&source)?;
        if let Some(existing) = &existing {
            let evaluated = existing
                .receipt()
                .body
                .evaluated_at_receiver_ns
                .parse::<u64>()
                .map_err(|_| "temporal_evaluation_data_integer")?;
            if allowed(cancel, clock)? < evaluated {
                return Err("temporal_evaluation_stored_clock_reversed".into());
            }
        }
        let stored = existing
            .map(|existing| {
                self.recertify_current_temporal_evaluation(&source, existing, cancel, clock, io)
            })
            .transpose()?;
        allowed(cancel, clock)?;
        Ok((proof, stored))
    }
}
