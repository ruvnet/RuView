//! Read-only reference custody in the SAME current C4C Runtime.
use super::*;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceScope {
    pub(crate) owner_scope: String,
    pub(crate) installation_id: String,
    pub(crate) runtime_epoch: String,
    pub(crate) generation: u64,
    pub(crate) context_digest: String,
    pub(crate) room_digest: String,
    pub(crate) room_revision: u64,
    pub(crate) model_sha256: String,
    pub(crate) model_kind: String,
    pub(crate) fit_record_sha256: String,
    pub(crate) training_proof_sha256: String,
    pub(crate) run: String,
    pub(crate) trial_id: String,
    pub(crate) session_id: String,
    pub(crate) role: DataRole,
}
impl RoomLearningRuntime {
    /// Caller keeps publication stable. Ingress neither changes a mutation
    /// guard nor touches native receiver counters, frames or inference state.
    pub fn with_reference_scope<T>(
        &self,
        room_digest: &str,
        room_revision: u64,
        source_ids: &[u8],
        received_ns: u64,
        operation: impl FnOnce(ReferenceScope, u64) -> Result<T, String>,
    ) -> Result<T, String> {
        let state = self
            .inner
            .lock()
            .map_err(|_| "reference_runtime_unavailable")?;
        // Transport receipt remains original. Admission freshness is sampled
        // only after serialization; a later status tick cannot reverse it.
        let admitted_ns = self.receiver_now_ns();
        if received_ns > admitted_ns || admitted_ns - received_ns > 3_000_000_000 {
            return Err("reference_ingress_queue_time_invalid".into());
        }
        if busy(&state.job) {
            return Err("reference_runtime_busy".into());
        }
        let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
            return Err("reference_current_temporal_required".into());
        };
        if c.failure.is_some()
            || c.owner_generation != state.generation
            || admitted_ns < c.last_now_ns
        {
            return Err("reference_runtime_changed".into());
        }
        let context = &c.context;
        if context.owner_scope != self.owner
            || context.installation_id != self.installation
            || context.room_digest != room_digest
            || room_revision == 0
            || context
                .sources
                .iter()
                .map(|s| s.source_id)
                .collect::<Vec<_>>()
                != source_ids
        {
            return Err("reference_publication_mismatch".into());
        }
        let proof = c
            .training_proof
            .as_ref()
            .ok_or("reference_training_proof_required")?;
        let id = &proof.body.identity;
        if id.owner_scope != self.owner
            || id.installation_id != self.installation
            || id.context_digest != context.digest()
            || id.room_digest != room_digest
            || id.artifact_kind != crate::room_learner::temporal_model_input::managed::SCHEMA_V3
        {
            return Err("reference_whole_model_changed".into());
        }
        let (trial_id, session_id) = c
            .core
            .as_ref()
            .ok_or("reference_no_capture")?
            .reference_phase(admitted_ns)?;
        operation(
            ReferenceScope {
                owner_scope: self.owner.clone(),
                installation_id: self.installation.clone(),
                runtime_epoch: self.runtime_epoch.clone(),
                generation: state.generation,
                context_digest: context.digest(),
                room_digest: room_digest.into(),
                room_revision,
                model_sha256: id.artifact_sha256.clone(),
                model_kind: id.artifact_kind.clone(),
                fit_record_sha256: id.verified_fit_record_sha256.clone(),
                training_proof_sha256: proof.sha256.clone(),
                run: id.run.clone(),
                trial_id,
                session_id,
                role: DataRole::HeldOut,
            },
            admitted_ns,
        )
    }
}

/// Non-serde current-read custody across bounded off-lock filesystem IO.
/// This authorizes no source, role, training, qualification or sensing action.
pub struct TemporalComparisonReadback {
    generation: u64,
    evaluation: Arc<crate::room_learner::artifact::StoredTemporalEvaluation>,
    room_digest: String,
    room_revision: u64,
    context_digest: String,
    value: serde_json::Value,
}
impl TemporalComparisonReadback {
    pub fn finish(
        self,
        runtime: &RoomLearningRuntime,
        publication_digest: &str,
        publication_revision: u64,
    ) -> Result<serde_json::Value, String> {
        let state = runtime
            .inner
            .lock()
            .map_err(|_| "reference_runtime_unavailable")?;
        if publication_digest != self.room_digest
            || publication_revision != self.room_revision
            || state.generation != self.generation
            || !matches!(&state.capture,Some(CaptureWorkflow::Temporal(c))
            if c.failure.is_none() && c.owner_generation==self.generation
                && c.context.room_digest==self.room_digest && c.context.digest()==self.context_digest
                && c.evaluation.as_ref().is_some_and(|e|Arc::ptr_eq(e,&self.evaluation)))
        {
            return Err("reference_runtime_changed".into());
        }
        Ok(self.value)
    }
}
impl RoomLearningRuntime {
    /// Filesystem work is outside Runtime/App/Room locks. The opaque finish()
    /// must run after IO under the caller's rechecked owner/publication scope.
    pub fn temporal_comparison_readback(
        &self,
        room_digest: &str,
        room_revision: u64,
    ) -> Result<TemporalComparisonReadback, String> {
        let (generation, evaluation, context_digest) = {
            let state = self
                .inner
                .lock()
                .map_err(|_| "reference_runtime_unavailable")?;
            let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
                return Err("reference_current_temporal_required".into());
            };
            if c.failure.is_some()
                || c.owner_generation != state.generation
                || c.context.room_digest != room_digest
            {
                return Err("reference_runtime_changed".into());
            }
            let e = c
                .evaluation
                .as_ref()
                .ok_or("reference_current_evaluation_required")?;
            (state.generation, e.clone(), c.context.digest())
        };
        let value = self
            .store
            .read_temporal_comparison(&evaluation, room_revision)?;
        Ok(TemporalComparisonReadback {
            generation,
            evaluation,
            room_digest: room_digest.into(),
            room_revision,
            context_digest,
            value,
        })
    }
}
