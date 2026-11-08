//! Version2 of the SAME protected selection pointer and immutable journal.
//! Only current protected Complete assessment writes it. Cold loading trusts
//! that owner-private journal, then checks every exact immutable dependency;
//! it never creates a new recovery/Complete/evaluation authority after expiry.
use super::*;
use crate::room_learner::temporal_model_input::TemporalModel;
use crate::room_learner::{
    temporal_evaluation as data, temporal_qualification::*, temporal_wire::*,
};
const POINTER: &str = "model-selection-v1.json";
const SCHEMA: &str = "ruview.room-learner.model-selection.v2";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemporalSelectionTarget {
    pub context: CaptureContext,
    pub model: TemporalModelIdentityWire,
    pub training_proof_sha256: String,
    pub held_out_evidence_sha256: String,
    pub evaluation_lookup_sha256: String,
    pub evaluation_receipt_sha256: String,
    pub assessment_lookup_sha256: String,
    pub assessment_sha256: String,
    pub qualification_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemporalSelectionReceipt {
    pub schema: String,
    pub owner_scope: String,
    pub installation_id: String,
    pub revision: u64,
    pub selected_at_unix_ms: u64,
    pub selected: TemporalSelectionTarget,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemporalSelectionEnvelope {
    pub receipt_sha256: String,
    pub receipt: TemporalSelectionReceipt,
}
/// No serde/Clone constructor; obtained only through this store's protected
/// current pointer+same immutable journal validation or a current write.
pub(crate) struct StoredTemporalSelection {
    pub(super) envelope: TemporalSelectionEnvelope,
    model: TemporalModel,
    pub(super) proof: data::TemporalTrainProofData,
    pub(super) held: data::TemporalHeldOutEvidenceData,
    pub(super) evaluation: data::TemporalEvaluationReceiptData,
    pub(super) qualification: TemporalQualificationEnvelope,
    pub(super) assessment: TemporalAssessmentEnvelope,
}
impl StoredTemporalSelection {
    pub(crate) fn envelope(&self) -> &TemporalSelectionEnvelope {
        &self.envelope
    }
    pub(crate) fn model(&self) -> &TemporalModel {
        &self.model
    }
    pub(crate) fn held(&self) -> &data::TemporalHeldOutEvidenceData {
        &self.held
    }
    pub(crate) fn qualification(&self) -> &TemporalQualificationEnvelope {
        &self.qualification
    }
    pub(crate) fn capture_generation_id(&self) -> Result<String, String> {
        let id = &self.proof.body.identity;
        crate::room_learner::qualification::hash(&(
            "ruview.room-learner.capture-generation.v2",
            &id.original_deadline.id,
            &id.run,
            &id.context_digest,
        ))
    }
    pub(crate) fn identity(&self) -> TemporalSelectedIdentityWire {
        let r = &self.envelope.receipt;
        let q = &self.qualification.receipt.summary;
        TemporalSelectedIdentityWire {
            selection_sha256: self.envelope.receipt_sha256.clone(),
            selection_revision: r.revision,
            model_kind: WholeTemporalKind::ManagedTemporalV3,
            model_sha256: q.model_sha256.clone(),
            verified_fit_record_sha256: q.verified_fit_record_sha256.clone(),
            training_proof_sha256: q.training_proof_sha256.clone(),
            held_out_evidence_sha256: q.held_out_evidence_sha256.clone(),
            complete_record_sha256: q.complete_record_sha256.clone(),
            evaluation_receipt_sha256: q.evaluation_receipt_sha256.clone(),
            qualification_sha256: q.receipt_sha256.clone(),
            policy_sha256: q.policy_sha256.clone(),
            context_digest: q.context_digest.clone(),
            room_digest: q.room_digest.clone(),
            source_node_ids: r
                .selected
                .context
                .sources
                .iter()
                .map(|s| s.source_id)
                .collect(),
            selected_at_unix_ms: r.selected_at_unix_ms,
            evaluated_at_unix_ms: q.evaluated_at_unix_ms,
            authority_scope: q.authority_scope.clone(),
        }
    }
}
impl ArtifactStore {
    pub(crate) fn temporal_selection_present(&self) -> Result<bool, String> {
        self.verify_owner()?;
        let path = self.root.join(POINTER);
        if !regular_or_missing(&path)? {
            return Ok(false);
        }
        let value: serde_json::Value = serde_json::from_slice(
            &private_read(&path).map_err(|_| "selection pointer unavailable")?,
        )
        .map_err(|_| "selection pointer invalid")?;
        Ok(value
            .pointer("/receipt/schema")
            .and_then(serde_json::Value::as_str)
            == Some(SCHEMA))
    }
    pub(crate) fn selection_revision_any(&self) -> Result<u64, String> {
        if self.temporal_selection_present()? {
            let bytes = private_read(&self.root.join(POINTER))
                .map_err(|_| "temporal_selection_pointer_unavailable")?;
            let e: TemporalSelectionEnvelope =
                serde_json::from_slice(&bytes).map_err(|_| "temporal_selection_pointer_schema")?;
            if e.receipt.schema != SCHEMA
                || e.receipt.owner_scope != self.owner_scope
                || e.receipt.installation_id != self.installation_id
                || e.receipt.revision == 0
                || e.receipt_sha256 != crate::room_learner::qualification::hash(&e.receipt)?
                || private_read(
                    &self
                        .root
                        .join("selections")
                        .join(format!("{}.json", e.receipt_sha256)),
                )
                .map_err(|_| "temporal_selection_journal_unavailable")?
                    != bytes
            {
                return Err("temporal_selection_pointer_changed".into());
            }
            // CAS identity only; a withdrawn old assessment must not prevent an
            // explicitly authorized replacement from fresh protected evidence.
            return Ok(e.receipt.revision);
        }
        Ok(self.load_selection()?.map_or(0, |e| e.receipt.revision))
    }
    /// Final CAS for THIS already admitted write, not a history loader.
    /// The first operation itself just created one immutable journal entry;
    /// missing pointer+that exact entry is still revision0. Every other orphan
    /// or concurrent history remains a conservative refusal.
    fn revision_for_prepared_temporal_selection(
        &self,
        prepared: &TemporalSelectionEnvelope,
        bytes: &[u8],
    ) -> Result<u64, String> {
        if regular_or_missing(&self.root.join(POINTER))? {
            return self.selection_revision_any();
        }
        if prepared.receipt.revision != 1
            || prepared.receipt.schema != SCHEMA
            || prepared.receipt.owner_scope != self.owner_scope
            || prepared.receipt.installation_id != self.installation_id
            || prepared.receipt_sha256
                != crate::room_learner::qualification::hash(&prepared.receipt)?
            || serde_json::to_vec(prepared).map_err(|_| "selection_encoding")? != bytes
        {
            return Err("selection_pointer_changed".into());
        }
        let history = self.root.join("selections");
        if !temporal_artifacts::directory(&history, false)? {
            return Err("selection_pending_history_missing".into());
        }
        let mut entries =
            std::fs::read_dir(&history).map_err(|_| "selection_pending_history_unavailable")?;
        let only = entries
            .next()
            .ok_or("selection_pending_history_missing")?
            .map_err(|_| "selection_pending_history_unavailable")?;
        if only.path() != history.join(format!("{}.json", prepared.receipt_sha256))
            || entries.next().is_some()
            || temporal_artifacts::bounded_read(&only.path(), LIMIT)? != bytes
        {
            return Err("selection_foreign_or_changed_history".into());
        }
        Ok(0)
    }
    pub(crate) fn load_temporal_selection(
        &self,
    ) -> Result<Option<StoredTemporalSelection>, String> {
        if !self.temporal_selection_present()? {
            return Ok(None);
        }
        let bytes =
            private_read(&self.root.join(POINTER)).map_err(|_| "temporal_selection_unavailable")?;
        let e: TemporalSelectionEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| "temporal_selection_schema")?;
        let r = &e.receipt;
        let t = &r.selected;
        if r.schema != SCHEMA
            || e.receipt_sha256 != crate::room_learner::qualification::hash(r)?
            || r.owner_scope != self.owner_scope
            || r.installation_id != self.installation_id
            || r.revision == 0
            || r.selected_at_unix_ms == 0
            || r.selected_at_unix_ms > 253402300799999
        {
            return Err("temporal_selection_identity".into());
        }
        self.validate_context(&t.context)?;
        if [
            &t.model.model_sha256,
            &t.model.verified_fit_record_sha256,
            &t.training_proof_sha256,
            &t.held_out_evidence_sha256,
            &t.evaluation_lookup_sha256,
            &t.evaluation_receipt_sha256,
            &t.assessment_lookup_sha256,
            &t.assessment_sha256,
            &t.qualification_sha256,
        ]
        .iter()
        .any(|s| !hex_digest(s))
        {
            return Err("temporal_selection_digest".into());
        }
        if private_read(
            &self
                .root
                .join("selections")
                .join(format!("{}.json", e.receipt_sha256)),
        )
        .map_err(|_| "temporal_selection_journal_missing")?
            != bytes
        {
            return Err("temporal_selection_journal_changed".into());
        }
        let model = self.load_exact_temporal_candidate(&t.context, &t.model.model_sha256)?;
        if model.artifact_kind() != crate::room_learner::temporal_model_input::managed::SCHEMA_V3 {
            return Err("temporal_selection_model_kind".into());
        }
        let dir = self
            .temporal_evaluation_directory(&t.context, false)?
            .ok_or("temporal_selection_metadata_missing")?;
        let proof = data::TemporalTrainProofData::decode_data(&temporal_artifacts::bounded_read(
            &dir.join(format!("train-proof-{}.json", t.training_proof_sha256)),
            data::MAX_METADATA_BYTES,
        )?)?;
        let held =
            data::TemporalHeldOutEvidenceData::decode_data(&temporal_artifacts::bounded_read(
                &dir.join(format!(
                    "heldout-evidence-{}.json",
                    t.held_out_evidence_sha256
                )),
                data::MAX_METADATA_BYTES,
            )?)?;
        let evaluation =
            data::TemporalEvaluationReceiptData::decode_data(&temporal_artifacts::bounded_read(
                &dir.join(format!("evaluation-{}.json", t.evaluation_lookup_sha256)),
                data::MAX_METADATA_BYTES,
            )?)?;
        evaluation.validate_data_links(&proof, &held)?;
        let qualification: TemporalQualificationEnvelope =
            serde_json::from_slice(&temporal_artifacts::bounded_read(
                &dir.join(format!("qualification-{}.json", t.qualification_sha256)),
                data::MAX_METADATA_BYTES,
            )?)
            .map_err(|_| "temporal_selection_qualification_schema")?;
        let assessment: TemporalAssessmentEnvelope =
            serde_json::from_slice(&temporal_artifacts::bounded_read(
                &dir.join(format!("assessment-{}.json", t.assessment_lookup_sha256)),
                data::MAX_METADATA_BYTES,
            )?)
            .map_err(|_| "temporal_selection_assessment_schema")?;
        assessment.validate_links(&proof, &held, &evaluation, Some(&qualification))?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct CurrentAssessment {
            schema: String,
            model_sha256: String,
            context_digest: String,
            assessment_sha256: String,
            assessment_lookup_sha256: String,
        }
        let current: CurrentAssessment = serde_json::from_slice(&temporal_artifacts::bounded_read(
            &dir.join(format!("latest-assessment-{}.json", t.model.model_sha256)),
            data::MAX_METADATA_BYTES,
        )?)
        .map_err(|_| "temporal_selection_latest_assessment_unavailable")?;
        if current.schema != "ruview.room-learner.temporal-current-assessment.v1"
            || current.model_sha256 != t.model.model_sha256
            || current.context_digest != t.context.digest()
            || current.assessment_sha256 != t.assessment_sha256
            || current.assessment_lookup_sha256 != t.assessment_lookup_sha256
        {
            return Err("newer_qualification_failed_or_requires_selection".into());
        }

        let id = &proof.body.identity;
        if proof.sha256 != t.training_proof_sha256
            || held.sha256 != t.held_out_evidence_sha256
            || evaluation.sha256 != t.evaluation_receipt_sha256
            || qualification.receipt_sha256 != t.qualification_sha256
            || assessment.receipt_sha256 != t.assessment_sha256
            || id.artifact_sha256 != t.model.model_sha256
            || id.verified_fit_record_sha256 != t.model.verified_fit_record_sha256
            || id.context_digest != t.context.digest()
            || id.owner_scope != self.owner_scope
            || id.installation_id != self.installation_id
            || r.selected_at_unix_ms < qualification.receipt.qualified_at_unix_ms
            || r.selected_at_unix_ms >= id.original_deadline.expires_at_unix_ms
        {
            return Err("temporal_selection_chain_changed".into());
        }
        Ok(Some(StoredTemporalSelection {
            envelope: e,
            model,
            proof,
            held,
            evaluation,
            qualification,
            assessment,
        }))
    }
    pub(crate) fn save_current_temporal_selection(
        &self,
        source:&crate::room_learner::temporal_model_input::managed::execution::full_run::VerifiedTemporalEvaluationSnapshot,
        assessment: &StoredTemporalAssessment,
        expected_revision: u64,
        now: u64,
        cancel: &std::sync::atomic::AtomicBool,
        clock: &mut dyn FnMut() -> Result<u64, String>,
        before_install: &mut dyn FnMut() -> Result<(), String>,
        after_rename: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<StoredTemporalSelection, String> {
        use std::sync::atomic::Ordering;
        source.require_same_store(self)?;
        if source.binding() != assessment.binding() {
            return Err("temporal_selection_current_complete_changed".into());
        }
        let q = assessment
            .qualification
            .as_ref()
            .ok_or("temporal_candidate_not_qualified")?;
        assessment
            .assessment
            .validate(&assessment.evaluation, Some(q))?;
        if now / 1_000_000
            >= source
                .proof()
                .body
                .identity
                .original_deadline
                .expires_at_unix_ms
        {
            return Err("recovery_expired".into());
        }
        if self.selection_revision_any()? != expected_revision {
            return Err("selection pointer changed".into());
        }
        let t = TemporalSelectionTarget {
            context: source.binding().context().clone(),
            model: TemporalModelIdentityWire {
                model_kind: WholeTemporalKind::ManagedTemporalV3,
                model_sha256: source.proof().body.identity.artifact_sha256.clone(),
                verified_fit_record_sha256: source
                    .proof()
                    .body
                    .identity
                    .verified_fit_record_sha256
                    .clone(),
            },
            training_proof_sha256: source.proof().sha256.clone(),
            held_out_evidence_sha256: source.held_out().sha256.clone(),
            evaluation_lookup_sha256: assessment.evaluation.lookup_sha256().into(),
            evaluation_receipt_sha256: assessment.evaluation.receipt().sha256.clone(),
            assessment_lookup_sha256: crate::room_learner::qualification::hash(&(
                crate::room_learner::temporal_wire::ASSESSMENT_SCHEMA,
                &assessment.evaluation.receipt().sha256,
                TemporalQualificationPolicyWire::current(),
            ))?,
            assessment_sha256: assessment.assessment.receipt_sha256.clone(),
            qualification_sha256: q.receipt_sha256.clone(),
        };
        let receipt = TemporalSelectionReceipt {
            schema: SCHEMA.into(),
            owner_scope: self.owner_scope.clone(),
            installation_id: self.installation_id.clone(),
            revision: expected_revision
                .checked_add(1)
                .ok_or("selection revision exhausted")?,
            selected_at_unix_ms: now / 1_000_000,
            selected: t,
        };
        let envelope = TemporalSelectionEnvelope {
            receipt_sha256: crate::room_learner::qualification::hash(&receipt)?,
            receipt,
        };
        let bytes = serde_json::to_vec(&envelope).map_err(|_| "temporal_selection_encoding")?;
        if cancel.load(Ordering::Acquire) {
            return Err("temporal_selection_cancelled".into());
        }
        clock()?;
        let history = self.root.join("selections");
        private_dir(&history).map_err(|_| "selection history unavailable")?;
        let history_path = history.join(format!("{}.json", envelope.receipt_sha256));
        temporal_artifacts::immutable(&history_path, &bytes)?;
        temporal_artifacts::recertify_immutable(&history_path, &bytes, LIMIT, &mut |_| {
            if cancel.load(Ordering::Acquire) {
                return Err("temporal_selection_cancelled".into());
            }
            clock()?;
            Ok(())
        })?;
        if cancel.load(Ordering::Acquire) {
            return Err("temporal_selection_cancelled".into());
        }
        clock()?;
        // The existing atomic pointer replacement is the linearization point.
        // Revalidate the current Complete witness/owner/deadline after all
        // preceding potentially slow IO, immediately before installing it.
        crate::companion::atomic_private_write_checked(
            &self.root.join(POINTER),
            &bytes,
            &mut || {
                before_install().map_err(std::io::Error::other)?;
                if self
                    .revision_for_prepared_temporal_selection(&envelope, &bytes)
                    .map_err(std::io::Error::other)?
                    != expected_revision
                {
                    return Err(std::io::Error::other("selection pointer changed"));
                }
                if cancel.load(Ordering::Acquire) {
                    return Err(std::io::Error::other("temporal_selection_cancelled"));
                }
                clock().map_err(std::io::Error::other)?;
                Ok(())
            },
            &mut || after_rename().map_err(std::io::Error::other),
        )
        .map_err(|_| "temporal_selection_pointer_write_unknown")?;
        let read = self
            .load_temporal_selection()?
            .ok_or("temporal_selection_readback_missing")?;
        if read.envelope != envelope {
            return Err("temporal_selection_readback_changed".into());
        }
        Ok(read)
    }
}

impl ArtifactStore {
    pub(crate) fn withdraw_temporal_selection(
        &self,
        sha: &str,
        revision: u64,
        now: u64,
        allowed: &mut dyn FnMut() -> Result<(), String>,
        before_install: &mut dyn FnMut() -> Result<(), String>,
        after_rename: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<crate::room_learner::selection::SelectionEnvelope, String> {
        // Negative CAS needs only the authenticated pointer/journal identity;
        // raw expiry, unavailable RF or a failed assessment cannot block Off.
        self.verify_owner()?;
        let bytes = private_read(&self.root.join(POINTER))
            .map_err(|_| "temporal_selection_pointer_unavailable")?;
        let old: TemporalSelectionEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| "temporal_selection_pointer_schema")?;
        if old.receipt_sha256 != sha
            || old.receipt.revision != revision
            || old.receipt.owner_scope != self.owner_scope
            || old.receipt.installation_id != self.installation_id
            || old.receipt.schema != SCHEMA
            || old.receipt_sha256 != crate::room_learner::qualification::hash(&old.receipt)?
            || self.selection_revision_any()? != revision
        {
            return Err("selected_pointer_changed".into());
        }
        let receipt = crate::room_learner::selection::SelectionReceipt {
            schema: "ruview.room-learner.model-selection.v1".into(),
            owner_scope: self.owner_scope.clone(),
            installation_id: self.installation_id.clone(),
            revision: revision
                .checked_add(1)
                .ok_or("selection_revision_exhausted")?,
            selected_at_unix_ms: now / 1_000_000,
            selected: None,
            previous: None,
        };
        let envelope = crate::room_learner::selection::SelectionEnvelope {
            receipt_sha256: crate::room_learner::qualification::hash(&receipt)?,
            receipt,
        };
        let bytes = serde_json::to_vec(&envelope).map_err(|_| "selection_encoding")?;
        let history = self.root.join("selections");
        private_dir(&history).map_err(|_| "selection_history_unavailable")?;
        let path = history.join(format!("{}.json", envelope.receipt_sha256));
        temporal_artifacts::immutable(&path, &bytes)?;
        temporal_artifacts::recertify_immutable(&path, &bytes, LIMIT, &mut |_| allowed())?;
        crate::companion::atomic_private_write_checked(
            &self.root.join(POINTER),
            &bytes,
            &mut || {
                before_install().map_err(std::io::Error::other)?;
                if self
                    .selection_revision_any()
                    .map_err(std::io::Error::other)?
                    != revision
                {
                    return Err(std::io::Error::other("selected_pointer_changed"));
                }
                allowed().map_err(std::io::Error::other)
            },
            &mut || after_rename().map_err(std::io::Error::other),
        )
        .map_err(|_| "selection_pointer_write_unknown")?;
        if self.load_selection()?.as_ref() != Some(&envelope) {
            return Err("selection_pointer_readback_changed".into());
        }
        Ok(envelope)
    }
}
