//! Comparison-only server phase custody. No measured teacher or clock authority.
use super::*;

/// In-process scope only: never deserialized from a phone or owner request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceScope {
    pub(crate) owner_scope: String,
    pub(crate) installation_id: String,
    pub(crate) runtime_epoch: String,
    pub(crate) generation: u64,
    pub(crate) context_digest: String,
    pub(crate) room_digest: String,
    pub(crate) model_sha256: String,
    pub(crate) trial_id: String,
    pub(crate) session_id: String,
    pub(crate) role: DataRole,
}
impl RoomLearningRuntime {
    /// Caller holds publication stable; this lock keeps the actual server phase,
    /// frozen model and context stable through the small bounded admission callback.
    /// Reference ingress never consumes a MutationGuard or touches RF counters.
    pub fn with_reference_scope<T>(
        &self,
        room_digest: &str,
        source_ids: &[u8],
        now_ns: u64,
        operation: impl FnOnce(ReferenceScope) -> Result<T, String>,
    ) -> Result<T, String> {
        let state = self
            .inner
            .lock()
            .map_err(|_| "reference_runtime_unavailable")?;
        if !state.frozen_training || busy(&state.job) {
            return Err("reference_requires_frozen_candidate".into());
        }
        let capture = state.capture.as_ref().ok_or("reference_no_capture")?;
        let context = capture.context();
        if context.owner_scope != self.owner
            || context.installation_id != self.installation
            || context.room_digest != room_digest
            || context
                .sources
                .iter()
                .map(|s| s.source_id)
                .collect::<Vec<_>>()
                != source_ids
        {
            return Err("reference_publication_mismatch".into());
        }
        let model = state.receipt.as_ref().ok_or("reference_no_model")?;
        if model.context_digest != context.digest() {
            return Err("reference_model_mismatch".into());
        }
        let phase = capture
            .progress(now_ns)
            .phase
            .ok_or("reference_no_active_phase")?;
        if !matches!(phase.role, DataRole::Validation | DataRole::HeldOut)
            || phase.stage != "collecting"
            || phase.capture_elapsed_ms >= phase.target_ms
        {
            return Err("reference_phase_not_collecting".into());
        }
        operation(ReferenceScope {
            owner_scope: self.owner.clone(),
            installation_id: self.installation.clone(),
            runtime_epoch: self.runtime_epoch.clone(),
            generation: state.generation,
            context_digest: context.digest(),
            room_digest: room_digest.into(),
            model_sha256: model.model_sha256.clone(),
            trial_id: phase.trial_id,
            session_id: phase.session_id,
            role: phase.role,
        })
    }
}
