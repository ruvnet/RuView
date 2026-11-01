//! Managed capture controls and source/provenance admission.
use super::*;
impl RoomLearningRuntime {
    pub fn bind_context(
        &self,
        room_digest: String,
        revision: u64,
        sources: Vec<crate::room_learner::observation::SourceSpec>,
    ) -> Result<CaptureContext, String> {
        let context = CaptureContext {
            owner_scope: self.owner.clone(),
            installation_id: self.installation.clone(),
            room_digest,
            revision,
            sources,
        };
        context.validate()?;
        Ok(context)
    }
    pub fn selects_grid(&self, source_id: u8, grid_id: &str) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|s| {
                s.capture.as_ref().map(|c| {
                    c.context()
                        .sources
                        .iter()
                        .any(|v| v.source_id == source_id && v.grid_id == grid_id)
                })
            })
            .unwrap_or(false)
    }
    pub fn capture_active(&self) -> Result<bool, String> {
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        Ok(state.capture.as_ref().is_some_and(|c| c.phase_active()))
    }
    pub fn context(&self) -> Option<CaptureContext> {
        self.inner
            .lock()
            .ok()?
            .capture
            .as_ref()
            .map(|c| c.context().clone())
    }
    pub fn begin_phase(
        &self,
        expected: &MutationGuard,
        request: PhaseRequest,
        now_ns: u64,
    ) -> Result<(), String> {
        let mut state = self.command_state(expected)?;
        if busy(&state.job) {
            return Err("training is active".into());
        }
        if request.role == DataRole::Train && state.frozen_training {
            return Err(
                "training data is frozen; explicitly start a new capture run to retrain".into(),
            );
        }
        if request.role == DataRole::HeldOut && state.model.is_none() {
            return Err(
                "held-out capture starts only after a candidate is trained or restored".into(),
            );
        }
        state
            .capture
            .as_mut()
            .ok_or("configure the current room and source set first")?
            .begin_phase(request, now_ns)?;
        state.input_error = None;
        Ok(())
    }
    pub fn ingest(&self, frame: RawSourceFrame<'_>, now_ns: u64) -> Result<(), String> {
        let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        let Some(capture) = state.capture.as_mut() else {
            return Ok(());
        };
        let result = capture.ingest(frame, now_ns);
        if let Err(error) = &result {
            state.input_error = Some(bounded(error.clone()));
        }
        result
    }
    pub fn ingest_native(
        &self,
        packet: &crate::room_learner::native::CanonicalPacket,
        now_ns: u64,
    ) -> Result<(), String> {
        if packet.provenance
            != crate::room_learner::observation::CaptureProvenance::LiveReceiverObserved
            || packet.native_frame.provenance.class
                != ruview_unified::frame::ProvenanceClass::Measured
        {
            return Err("live managed capture rejects synthetic/replayed native provenance".into());
        }
        let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        let Some(capture) = state.capture.as_mut() else {
            return Ok(());
        };
        let source = capture
            .context()
            .sources
            .iter()
            .find(|s| s.source_id == packet.source_id)
            .ok_or("unselected native source")?;
        if packet.native_frame.provenance.device_id != source.device_id
            || packet.native_frame.modality != source.signal_kind
        {
            return Err("native RF provenance does not match the selected source".into());
        }
        let result = capture.ingest(
            RawSourceFrame {
                source_id: packet.source_id,
                grid_id: &packet.grid_id,
                frequency_hz: packet.frequency_hz,
                bandwidth_hz: packet.bandwidth_hz,
                sequence: packet.sequence,
                amplitudes: &packet.amplitudes,
            },
            now_ns,
        );
        if let Err(error) = &result {
            state.input_error = Some(bounded(error.clone()));
        }
        result
    }
    /// Bounded protected diagnostics only; never grants admission or advances a
    /// workflow revision. Ignore sources/grids outside the current context.
    pub fn record_native_rejection(&self, source_id:u8, grid_id:&str, reason:&str) {
        if let Ok(mut state)=self.inner.lock() {
            if state.capture.as_ref().is_some_and(|c|c.context().sources.iter().any(|s|s.source_id==source_id && s.grid_id==grid_id)) {
                state.input_error=Some(bounded(format!("Node {source_id} native input rejected: {reason}")));
            }
        }
    }
    pub fn finish_phase(
        &self,
        expected: &MutationGuard,
        now_ns: u64,
        conditions_held: bool,
    ) -> Result<PhaseReceipt, String> {
        let mut state = self.command_state(expected)?;
        let capture = state
            .capture
            .as_mut()
            .ok_or("no learner capture configured")?;
        if !conditions_held {
            capture.cancel_phase();
            return Err(
                "operator did not confirm conditions held; in-flight examples discarded".into(),
            );
        }
        capture.finish_phase(now_ns)
    }
    pub fn cancel_phase(&self, expected: &MutationGuard) -> Result<(), String> {
        let mut state = self.command_state(expected)?;
        state
            .capture
            .as_mut()
            .ok_or("no learner capture configured")?
            .cancel_phase();
        Ok(())
    }
    pub fn cancel_training(&self, expected: &MutationGuard) -> Result<JobPhase, String> {
        let mut state = self.command_state(expected)?;
        if busy(&state.job) {
            state.job.cancel.store(true, Ordering::Release);
            state.job.phase = JobPhase::Cancelling;
        }
        Ok(state.job.phase)
    }
}
