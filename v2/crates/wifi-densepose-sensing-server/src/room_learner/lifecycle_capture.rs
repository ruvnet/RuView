//! Managed capture controls and source/provenance admission.
use super::*;

/// Transient metadata for one exact native packet. Deliberately not Serialize.
/// Relative receiver durations are not a source measurement clock.
///
/// ```compile_fail
/// use wifi_densepose_sensing_server::room_learner::lifecycle::ReceiverPipelineTiming;
/// fn requires_serialize<T: serde::Serialize>() {}
/// requires_serialize::<ReceiverPipelineTiming<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct ReceiverPipelineTiming<'a> {
    source_id: u8,
    sequence: u32,
    frame_id: u128,
    grid_id: &'a str,
    frequency_hz: u64,
    bandwidth_hz: u64,
    durations: [Option<u32>; 4],
}
impl<'a> ReceiverPipelineTiming<'a> {
    pub fn for_packet(packet: &'a crate::room_learner::native::CanonicalPacket,
        dequeue_interval_us: Option<u32>, parse_us: Option<u32>, state_lock_wait_us: Option<u32>, dequeue_to_stamp_us: Option<u32>) -> Self {
        Self { source_id: packet.source_id, sequence: packet.sequence, frame_id: packet.native_frame.frame_id,
            grid_id: &packet.grid_id, frequency_hz: packet.frequency_hz, bandwidth_hz: packet.bandwidth_hz,
            durations: [dequeue_interval_us, parse_us, state_lock_wait_us, dequeue_to_stamp_us] }
    }
    fn failure_detail(&self, packet: &crate::room_learner::native::CanonicalPacket, primary: &str) -> Option<String> {
        if self.source_id != packet.source_id || self.sequence != packet.sequence || self.frame_id != packet.native_frame.frame_id
            || self.grid_id != packet.grid_id || self.frequency_hz != packet.frequency_hz || self.bandwidth_hz != packet.bandwidth_hz { return None; }
        crate::room_learner::temporal_model_input::managed::execution::receiver_pipeline_failure_detail(primary, self.durations)
    }
}
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
        self.inner.lock().map(|state|state.capture_context().is_some_and(|c|c.sources.iter().any(|s|s.source_id==source_id&&s.grid_id==grid_id))||state.selection.temporal.as_ref().is_some_and(|selected|selected.stored.envelope().receipt.selected.context.sources.iter().any(|s|s.source_id==source_id&&s.grid_id==grid_id))).unwrap_or(false)
    }
    pub fn capture_active(&self) -> Result<bool, String> {
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        Ok(match state.capture.as_ref() {
            Some(CaptureWorkflow::Legacy(c)) => c.phase_active(),
            Some(CaptureWorkflow::Temporal(c)) => c.active(),
            None => false,
        })
    }
    pub fn context(&self) -> Option<CaptureContext> {
        self.inner.lock().ok()?.capture_context().cloned()
    }
    pub fn begin_phase(
        &self,
        expected: &MutationGuard,
        request: PhaseRequest,
        now_ns: u64,
    ) -> Result<(), String> {
        let mut state = self.command_state(expected)?;
        state.require_legacy_kind()?;
        self.expire_loaded_recovery(&mut state, now_ns)?;
        self.require_recovery_ready(&state)?;
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
        if request.role==DataRole::HeldOut {
            let readiness=self.qualification_readiness_locked(&state,now_ns);
            if readiness["eligible_for_held_out"]!=true {
                return Err(readiness["detail"].as_str().unwrap_or("managed training lineage unavailable before held-out capture").into());
            }
        }
        state.legacy_capture_mut()
            .ok_or("configure the current room and source set first")?
            .begin_phase(request, now_ns)?;
        state.input_error = None;
        self.checkpoint_recovery(&mut state, now_ns, true)?;
        Ok(())
    }
    pub fn ingest(&self, frame: RawSourceFrame<'_>, now_ns: u64) -> Result<(), String> {
        let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        state.require_legacy_kind()?;
        let Some(capture) = state.legacy_capture_mut() else {
            return Ok(());
        };
        let result = capture.ingest(frame, now_ns);
        if let Err(error) = &result {
            capture.clear_live_windows();
            state.input_error = Some(bounded(error.clone()));
        } else if capture.live_cohort(now_ns).is_ok() {
            state.input_error = None;
        }
        let saved = self.checkpoint_recovery(&mut state, now_ns, false);
        result.and(saved)
    }
    pub fn ingest_native(
        &self,
        packet: &crate::room_learner::native::CanonicalPacket,
        now_ns: u64,
    ) -> Result<(), String> {
        self.ingest_native_with_receiver_timing(packet, now_ns, None)
    }
    pub fn ingest_native_with_receiver_timing(
        &self,
        packet: &crate::room_learner::native::CanonicalPacket,
        now_ns: u64,
        receiver_timing: Option<ReceiverPipelineTiming<'_>>,
    ) -> Result<(), String> {
        if packet.provenance
            != crate::room_learner::observation::CaptureProvenance::LiveReceiverObserved
            || packet.native_frame.provenance.class
                != ruview_unified::frame::ProvenanceClass::Measured
        {
            self.record_native_rejection(packet.source_id,&packet.grid_id,"synthetic/replayed native provenance");
            return Err("live managed capture rejects synthetic/replayed native provenance".into());
        }
        // Selected whole-model inference runs outside Runtime.inner. A stale
        // Arc cannot publish evidence after selection is replaced; reads fence
        // the exact current Arc again before returning a usable decision.
        let selected={self.inner.lock().map_err(|_|"learner state unavailable")?.selection.temporal.clone()};
        let selected_present=selected.is_some();
        if let Some(selected)=selected{if selected.stored.envelope().receipt.selected.context.sources.iter().any(|s|s.source_id==packet.source_id&&s.grid_id==packet.grid_id&&s.center_frequency_hz==packet.frequency_hz&&s.bandwidth_hz==packet.bandwidth_hz){
            let mut driver=selected.driver.lock().map_err(|_|"selected_native_input_unavailable")?;
            // Poll/inference may have advanced processing time while this
            // packet waited for the driver. Measure the trusted processing
            // clock HERE; preserve its original receive timestamp/identities.
            let processing_now=self.receiver_now_ns();
            if processing_now<now_ns{return Err("selected_receiver_processing_clock_reversed".into());}
            driver.ingest(packet,processing_now);
        }}
        // Declared BEFORE the lock: every return/unwind releases inner before
        // any accepted/rejected ACK payload or replaced raw state is reclaimed.
        let mut retired=self.deferred_reclamation();
        let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        let now_ns=if selected_present{let processing_now=self.receiver_now_ns();if processing_now<now_ns{return Err("selected_receiver_processing_clock_reversed".into());}processing_now}else{now_ns};
        let owner_generation=state.generation;
        if let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() {
            if capture.owner_generation()!=owner_generation {capture.invalidate_owner();return Err("temporal_owner_generation_changed".into());}
            let mut result = capture.ingest(self.temporal_store()?, &self.runtime_epoch, packet, now_ns,&mut retired);
            if let (Err(primary), Some(timing)) = (&result, receiver_timing) {
                if let Some(detail) = timing.failure_detail(packet, primary) {
                    if capture.annotate_receiver_failure(primary, detail.clone()) { result = Err(detail); }
                }
            }
            #[cfg(test)] if retired.has_ack() && self.panic_after_reclamation_ack.swap(false,Ordering::AcqRel){panic!("synthetic panic after actual ACK admission");}
            if let Err(error) = &result { state.input_error = Some(bounded(error.clone())); }
            return result;
        }
        let Some(capture) = state.legacy_capture_mut() else {
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
            if packet.grid_id==source.grid_id && packet.frequency_hz==source.center_frequency_hz && packet.bandwidth_hz==source.bandwidth_hz {
                capture.clear_live_windows();
                state.input_error=Some("native RF provenance does not match the selected source".into());
            }
            return Err("native RF provenance does not match the selected source".into());
        }
        // Another PHY class from the same receiver is not this selected source
        // view. Ignore it; required-cohort freshness still gates every decision.
        if packet.frequency_hz != source.center_frequency_hz
            || packet.bandwidth_hz != source.bandwidth_hz
        {
            return Ok(());
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
            capture.clear_live_windows();
            state.input_error = Some(bounded(error.clone()));
        } else if capture.live_cohort(now_ns).is_ok() {
            state.input_error = None;
        }
        let saved = self.checkpoint_recovery(&mut state, now_ns, false);
        result.and(saved)
    }
    /// Bounded protected diagnostics only; never grants admission or advances a
    /// workflow revision. Ignore sources/grids outside the current context.
    pub fn record_native_rejection(&self, source_id: u8, grid_id: &str, reason: &str) {
        if let Ok(mut state) = self.inner.lock() {
            if let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() {
                capture.reject(source_id, grid_id, reason);
                return;
            }

            if state.legacy_capture().is_some_and(|c| {
                c.context()
                    .sources
                    .iter()
                    .any(|s| s.source_id == source_id && s.grid_id == grid_id)
            }) {
                if let Some(capture)=state.legacy_capture_mut() {capture.clear_live_windows();}
                state.input_error = Some(bounded(format!(
                    "Node {source_id} native input rejected: {reason}"
                )));
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
        state.require_legacy_kind()?;
        self.expire_loaded_recovery(&mut state, now_ns)?;
        let recovered_complete = state.recovery.disposition
            == Some(
                crate::room_learner::capture::RecoveryPhaseDisposition::CompletedNeedsConfirmation,
            );
        let capture = state.legacy_capture_mut()
            .ok_or("no learner capture configured")?;
        if !conditions_held {
            capture.cancel_phase();
            let _ = self.checkpoint_recovery(&mut state, now_ns, true);
            return Err(
                "operator did not confirm conditions held; in-flight examples discarded".into(),
            );
        }
        if recovered_complete {
            capture.live_cohort(now_ns)?;
        }
        // A phase is not published as sealed until its metadata-only replay
        // fence is durable. Failure leaves the complete original phase retryable.
        let mut completed=capture.clone();
        let receipt=completed.finish_phase(now_ns)?;
        if let Some(model)=state.model.as_ref() {
            if let Some(proof)=self.store.load_managed_training_proof(model)? {
                completed.checkpoint(now_ns)?;
                let seed=completed.managed_fence_seed(&proof)?;
                self.store.save_managed_capture_fence(model,&proof,seed)?;
            }
        }
        state.capture=Some(CaptureWorkflow::Legacy(completed));
        state.recovery.disposition = None;
        self.checkpoint_recovery(&mut state, now_ns, true)?;
        Ok(receipt)
    }
    pub fn cancel_phase(&self, expected: &MutationGuard) -> Result<(), String> {
        let mut state = self.command_state(expected)?;
        state.require_legacy_kind()?;
        state.legacy_capture_mut()
            .ok_or("no learner capture configured")?
            .cancel_phase();
        let now = self.checkpoint_now(&state);
        self.checkpoint_recovery(&mut state, now, true)?;
        Ok(())
    }
    pub fn cancel_training(&self, expected: &MutationGuard) -> Result<JobPhase, String> {
        let mut state = self.command_state(expected)?;
        state.require_legacy_kind()?;
        if busy(&state.job) {
            state.job.cancel.store(true, Ordering::Release);
            state.job.phase = JobPhase::Cancelling;
        }
        let now = self.checkpoint_now(&state);
        self.checkpoint_recovery(&mut state, now, true)?;
        Ok(state.job.phase)
    }
}

#[cfg(test)]
#[path = "receiver_pipeline_timing_tests.rs"]
mod receiver_timing_tests;
