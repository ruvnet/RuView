use super::*;
use crate::room_learner::{assessment::AssessmentEnvelope, qualification::QualificationEnvelope};
impl ArtifactStore {
    fn assessment_dir(&self, model: &RoomModel, create: bool) -> Result<Option<PathBuf>, String> {
        let Some(dir) = self.context_dir(model.context(), create)? else {
            return Ok(None);
        };
        let path = dir.join("assessments");
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound && !create => return Ok(None),
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        }
        private_dir(&path).map_err(|e| e.to_string())?;
        Ok(Some(path))
    }
    fn verify_assessment(&self, model: &RoomModel, a: &AssessmentEnvelope) -> Result<(), String> {
        let e = self.load_exact_evaluation(
            model,
            crate::room_learner::observation::DataRole::HeldOut,
            &a.receipt.evaluation_receipt_sha256,
        )?;
        a.validate(model, &e)?;
        if let Some(sha) = &a.receipt.qualification_sha256 {
            let q = self.load_qualification(model, sha)?;
            if q.receipt.evaluation_receipt_sha256 != a.receipt.evaluation_receipt_sha256
                || q.receipt.sealed_history_sha256 != a.receipt.sealed_history_sha256
            {
                return Err("assessment qualification differs".into());
            }
        }
        Ok(())
    }
    pub(crate) fn load_latest_assessment(
        &self,
        model: &RoomModel,
    ) -> Result<Option<AssessmentEnvelope>, String> {
        let Some(dir) = self.assessment_dir(model, false)? else {
            return Ok(None);
        };
        let path = dir.join(format!("{}.latest.json", digest(&model.export()?)));
        if !regular_or_missing(&path)? {
            return Ok(None);
        }
        let a: AssessmentEnvelope =
            serde_json::from_slice(&private_read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        self.verify_assessment(model, &a)?;
        let immutable = dir.join(format!("{}.json", a.receipt_sha256));
        if private_read(&immutable).map_err(|e| e.to_string())? != encode(&a)? {
            return Err("assessment pointer differs from immutable history".into());
        }
        Ok(Some(a))
    }
    pub(crate) fn save_assessment(
        &self,
        model: &RoomModel,
        a: &AssessmentEnvelope,
    ) -> Result<(), String> {
        self.verify_assessment(model, a)?;
        if let Some(previous) = self.load_latest_assessment(model)? {
            if previous == *a {
                return Ok(());
            }
            if a.receipt.evaluated_at_unix_ms < previous.receipt.evaluated_at_unix_ms
                || a.receipt.last_held_out_observed_ns <= previous.receipt.last_held_out_observed_ns
            {
                return Err(
                    "older qualification evidence cannot replace a newer assessment".into(),
                );
            }
        }
        let dir = self
            .assessment_dir(model, true)?
            .ok_or("assessment directory unavailable")?;
        let bytes = encode(a)?;
        evaluation_store::write_immutable(&dir.join(format!("{}.json", a.receipt_sha256)), &bytes)?;
        atomic_private_write(
            &dir.join(format!("{}.latest.json", a.receipt.model_sha256)),
            &bytes,
        )
        .map_err(|e| e.to_string())?;
        if self.load_latest_assessment(model)?.as_ref() != Some(a) {
            return Err("assessment durable readback mismatch".into());
        }
        Ok(())
    }
    pub(crate) fn current_qualification_assessment(
        &self,
        model: &RoomModel,
        q: &QualificationEnvelope,
    ) -> Result<AssessmentEnvelope, String> {
        let a = self
            .load_latest_assessment(model)?
            .ok_or("latest completed qualification assessment unavailable")?;
        if a.receipt.failure_reason.is_some() {
            return Err("newer_qualification_failed".into());
        }
        if a.receipt.qualification_sha256.as_deref() != Some(q.receipt_sha256.as_str()) {
            return Err("new qualification requires explicit selection".into());
        }
        Ok(a)
    }
}
