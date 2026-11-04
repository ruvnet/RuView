//! Owner-private immutable qualification and guarded durable selected-model pointer.
use super::*;
use crate::room_learner::{
    qualification::{hash, QualificationEnvelope},
    selection::{SelectionEnvelope, SelectionTarget},
};
const SELECTION: &str = "model-selection-v1.json";
impl ArtifactStore {
    fn qualification_dir(
        &self,
        context: &CaptureContext,
        create: bool,
    ) -> Result<Option<PathBuf>, String> {
        let Some(dir) = self.context_dir(context, create)? else {
            return Ok(None);
        };
        let path = dir.join("qualifications");
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound && !create => return Ok(None),
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        }
        private_dir(&path).map_err(|e| e.to_string())?;
        Ok(Some(path))
    }
    pub(crate) fn save_qualification(
        &self,
        model: &RoomModel,
        q: &QualificationEnvelope,
    ) -> Result<(), String> {
        self.load_exact_candidate(model.context(), &q.receipt.model_sha256)?;
        let eval = self.load_exact_evaluation(
            model,
            crate::room_learner::observation::DataRole::HeldOut,
            &q.receipt.evaluation_receipt_sha256,
        )?;
        q.validate(model, &eval)?;
        if let Some(proof)=self.load_managed_training_proof(model)? {self.load_managed_capture_fence(model,&proof)?;}

        if let Some(expected)=&q.receipt.managed_training_proof_sha256 {
            let proof=self.load_managed_training_proof(model)?.ok_or("qualification managed training proof missing")?;
            if &proof.proof_sha256!=expected {return Err("qualification managed training proof changed".into());}
            self.require_managed_capture_fence(model,&proof,q.receipt.managed_sealed_phase_fence_sha256.as_deref().ok_or("qualification sealed phase fence missing")?)?;
        }
        let dir = self
            .qualification_dir(model.context(), true)?
            .ok_or("qualification directory unavailable")?;
        evaluation_store::write_immutable(
            &dir.join(format!("{}.json", q.receipt_sha256)),
            &encode(q)?,
        )?;
        self.load_qualification(model, &q.receipt_sha256)?;
        let pointer = serde_json::json!({"receipt_sha256":q.receipt_sha256,"model_sha256":q.receipt.model_sha256,"context_digest":model.context().digest()});
        atomic_private_write(
            &dir.join(format!("{}.latest.json", q.receipt.model_sha256)),
            &encode(&pointer)?,
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub(crate) fn load_qualification(
        &self,
        model: &RoomModel,
        sha: &str,
    ) -> Result<QualificationEnvelope, String> {
        if !hex_digest(sha) {
            return Err("invalid qualification digest".into());
        }
        let dir = self
            .qualification_dir(model.context(), false)?
            .ok_or("qualification unavailable")?;
        let q: QualificationEnvelope = serde_json::from_slice(
            &private_read(&dir.join(format!("{sha}.json"))).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if q.receipt_sha256 != sha {
            return Err("qualification hash mismatch".into());
        }
        let eval = self.load_exact_evaluation(
            model,
            crate::room_learner::observation::DataRole::HeldOut,
            &q.receipt.evaluation_receipt_sha256,
        )?;
        q.validate(model, &eval)?;
        if let Some(proof)=self.load_managed_training_proof(model)? {self.load_managed_capture_fence(model,&proof)?;}

        if let Some(expected)=&q.receipt.managed_training_proof_sha256 {
            let proof=self.load_managed_training_proof(model)?.ok_or("qualification managed training proof missing")?;
            if &proof.proof_sha256!=expected {return Err("qualification managed training proof changed".into());}
            self.require_managed_capture_fence(model,&proof,q.receipt.managed_sealed_phase_fence_sha256.as_deref().ok_or("qualification sealed phase fence missing")?)?;
        }
        Ok(q)
    }
    pub(crate) fn load_latest_qualification(
        &self,
        model: &RoomModel,
    ) -> Result<Option<QualificationEnvelope>, String> {
        let Some(dir) = self.qualification_dir(model.context(), false)? else {
            return Ok(None);
        };
        let sha = digest(&model.export()?);
        let path = dir.join(format!("{sha}.latest.json"));
        if !regular_or_missing(&path)? {
            return Ok(None);
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Pointer {
            receipt_sha256: String,
            model_sha256: String,
            context_digest: String,
        }
        let p: Pointer = serde_json::from_slice(&private_read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if p.model_sha256 != sha || p.context_digest != model.context().digest() {
            return Err("qualification pointer identity mismatch".into());
        }
        let q = self.load_qualification(model, &p.receipt_sha256)?;
        let Some(a) = self.load_latest_assessment(model)? else {
            return Ok(None);
        };
        if a.receipt.failure_reason.is_some() {
            return Ok(None);
        }
        if a.receipt.qualification_sha256.as_deref() != Some(q.receipt_sha256.as_str()) {
            return Err("latest qualification pointer differs from completed assessment".into());
        }
        Ok(Some(q))
    }
    pub(crate) fn resolve_selection_target(
        &self,
        t: &SelectionTarget,
    ) -> Result<(RoomModel, QualificationEnvelope), String> {
        self.validate_context(&t.context)?;
        let model = self.load_exact_candidate(&t.context, &t.model_sha256)?;
        let q = self.load_qualification(&model, &t.qualification_sha256)?;
        if model.version != t.model_version || q.receipt.model_sha256 != t.model_sha256 {
            return Err("selection model identity mismatch".into());
        }
        Ok((model, q))
    }
    fn verify_no_selection_history(&self) -> Result<(), String> {
        let history = self.root.join("selections");
        let metadata = match std::fs::symlink_metadata(&history) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(_) => {
                return Err(
                    "selection history cannot be inspected while its pointer is missing".into(),
                )
            }
            Ok(m) => m,
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("selection history is unsafe while its pointer is missing".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            unsafe extern "C" {
                fn geteuid() -> u32;
            }
            // SAFETY: POSIX geteuid has no arguments or memory side effects.
            let uid = unsafe { geteuid() };
            if metadata.permissions().mode() & 0o777 != 0o700 || metadata.uid() != uid {
                return Err(
                    "selection history is not privately readable while its pointer is missing"
                        .into(),
                );
            }
        }
        let mut entries = std::fs::read_dir(&history)
            .map_err(|_| "selection history cannot be read while its pointer is missing")?;
        match entries.next() {
            None=>Ok(()),
            Some(Ok(_))=>Err("selection pointer is missing but retained selection history exists; explicit recovery is required".into()),
            Some(Err(_))=>Err("selection history cannot be read while its pointer is missing".into()),
        }
    }
    pub(crate) fn load_selection(&self) -> Result<Option<SelectionEnvelope>, String> {
        self.verify_owner()?;
        let path = self.root.join(SELECTION);
        if !regular_or_missing(&path)? {
            // Absence is only known when no selection commit was attempted.
            // Immutable history may outlive a lost pointer or a failed first
            // pointer write. Never guess a replacement from that history.
            self.verify_no_selection_history()?;
            return Ok(None);
        }
        let envelope: SelectionEnvelope =
            serde_json::from_slice(&private_read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let r = &envelope.receipt;
        if r.schema != "ruview.room-learner.model-selection.v1"
            || envelope.receipt_sha256 != hash(r)?
            || r.owner_scope != self.owner_scope
            || r.installation_id != self.installation_id
            || r.revision == 0
            || r.selected_at_unix_ms == 0
            || r.selected_at_unix_ms > 253_402_300_799_999
        {
            return Err("selected model pointer identity mismatch".into());
        }
        for t in [&r.selected, &r.previous].into_iter().flatten() {
            self.validate_context(&t.context)?;
            if !hex_digest(&t.model_sha256) || !hex_digest(&t.qualification_sha256) {
                return Err("invalid selection target digest".into());
            }
        }
        Ok(Some(envelope))
    }
    pub(crate) fn save_selection(
        &self,
        envelope: &SelectionEnvelope,
        expected_revision: u64,
    ) -> Result<(), String> {
        let current = self.load_selection()?;
        if current.as_ref().map_or(0, |e| e.receipt.revision) != expected_revision
            || envelope.receipt.revision
                != expected_revision
                    .checked_add(1)
                    .ok_or("selection revision exhausted")?
            || envelope.receipt.owner_scope != self.owner_scope
            || envelope.receipt.installation_id != self.installation_id
            || envelope.receipt.schema != "ruview.room-learner.model-selection.v1"
            || envelope.receipt_sha256 != hash(&envelope.receipt)?
        {
            return Err("selection pointer changed or invalid".into());
        }
        if let Some(t) = &envelope.receipt.selected {
            self.resolve_selection_target(t)?;
        }
        let bytes = encode(envelope)?;
        // Save history before atomically replacing the sole authoritative pointer.
        let history = self.root.join("selections");
        private_dir(&history).map_err(|e| e.to_string())?;
        evaluation_store::write_immutable(
            &history.join(format!("{}.json", envelope.receipt_sha256)),
            &bytes,
        )?;
        atomic_private_write(&self.root.join(SELECTION), &bytes).map_err(|e| e.to_string())?;
        if self.load_selection()?.as_ref() != Some(envelope) {
            return Err("selection durable readback mismatch".into());
        }
        Ok(())
    }
}
