//! Protected C4C projections from the SAME current FullRun producer.
//! Raw prefix/DATA decoding cannot create either projection or this pending
//! one-use source. All current-store IO, validation and hashing run off-lock.
use super::*;
use crate::room_learner::temporal_evaluation as data;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, PartialEq)]
pub(crate) struct TemporalEvaluationBinding {
    epoch: String,
    generation: u64,
    context: CaptureContext,
    context_sha256: String,
    run: String,
    token: TemporalToken,
    fit: String,
    model: String,
    complete: Option<String>,
}
impl TemporalEvaluationBinding {
    pub(crate) fn owner_matches(&self, epoch: &str, generation: u64) -> bool {
        self.epoch == epoch && self.generation == generation
    }
    pub(crate) fn context_digest(&self) -> &str {
        &self.context_sha256
    }
    pub(crate) fn context(&self) -> &CaptureContext {
        &self.context
    }
    pub(crate) fn recertify_current(&self, store: &RecoveryStore, now: u64) -> Result<(), String> {
        store.recertify_temporal(&self.token, now).map(|_| ())
    }
    pub(crate) fn model_sha256(&self) -> &str {
        &self.model
    }
    pub(crate) fn complete_sha256(&self) -> Option<&str> {
        self.complete.as_deref()
    }
}
/// No Clone/serde/import constructor. Minted below only from current owned
/// verified Frozen/Complete facts; consuming it authenticates the current store.
pub(crate) struct PendingTemporalEvidenceSource {
    binding: TemporalEvaluationBinding,
    model: Arc<TemporalModel>,
    artifact: Arc<ArtifactStore>,
    context: CaptureContext,
    start_floor: u64,
    deadline: u64,
    held_out: bool,
}
pub(crate) struct VerifiedTemporalTrainSnapshot {
    model: Arc<TemporalModel>,
    artifact: Arc<ArtifactStore>,
    proof: data::TemporalTrainProofData,
}
pub(crate) struct VerifiedTemporalEvaluationSnapshot {
    train: VerifiedTemporalTrainSnapshot,
    binding: TemporalEvaluationBinding,
    held_out: data::TemporalHeldOutEvidenceData,
    rows: Vec<SharedRecord<CaptureCohort>>,
}
impl VerifiedTemporalTrainSnapshot {
    pub(crate) fn model(&self) -> &Arc<TemporalModel> {
        &self.model
    }
    pub(crate) fn require_same_store(&self, store: &ArtifactStore) -> Result<(), String> {
        if !std::ptr::eq(store, self.artifact.as_ref()) {
            return Err("temporal_evaluation_same_store_required".into());
        }
        Ok(())
    }
    pub(crate) fn proof(&self) -> &data::TemporalTrainProofData {
        &self.proof
    }
}
impl VerifiedTemporalEvaluationSnapshot {
    pub(crate) fn train_snapshot(&self) -> &VerifiedTemporalTrainSnapshot {
        &self.train
    }
    pub(crate) fn proof(&self) -> &data::TemporalTrainProofData {
        self.train.proof()
    }
    pub(crate) fn held_out(&self) -> &data::TemporalHeldOutEvidenceData {
        &self.held_out
    }
    pub(crate) fn binding(&self) -> &TemporalEvaluationBinding {
        &self.binding
    }
    pub(crate) fn model(&self) -> &Arc<TemporalModel> {
        &self.train.model
    }
    pub(crate) fn require_same_store(&self, store: &ArtifactStore) -> Result<(), String> {
        if !std::ptr::eq(store, self.train.artifact.as_ref()) {
            return Err("temporal_evaluation_same_store_required".into());
        }
        Ok(())
    }
    pub(crate) fn evaluation_rows(&self) -> Vec<CaptureCohort> {
        self.rows.iter().map(|r| (**r).clone()).collect()
    }
    pub(crate) fn require_same_model(&self, model: &TemporalModel) -> Result<(), String> {
        if !std::ptr::eq(model, self.train.model.as_ref()) {
            return Err("temporal_evaluation_actual_model_required".into());
        }
        Ok(())
    }
}
impl TemporalWorkflow {
    pub(crate) fn prepare_temporal_evidence_source(
        &self,
        now: u64,
        held_out: bool,
        schedule: &ManagedScheduleContract,
    ) -> Result<PendingTemporalEvidenceSource, String> {
        let mode = self.full_run.as_ref().ok_or("temporal_full_run_required")?;
        let owner = mode
            .owner
            .as_ref()
            .ok_or("temporal_full_run_owner_custody_required")?;
        let verified = mode
            .verified
            .as_ref()
            .ok_or("temporal_verified_frozen_required")?;
        let fit = mode
            .frozen
            .as_ref()
            .ok_or("temporal_verified_fit_record_required")?;
        let token = self
            .commit_token
            .as_ref()
            .ok_or("temporal_current_token_required")?;
        // A paused suffix mapping cannot change the already fitted/confirmed
        // first64 Train rows. It is permitted ONLY for stable TrainProof, never
        // for a HeldOut evaluation/qualification/selection capability.
        let suffix_mapping = !held_out
            && self.stage == Stage::PausedNeedsReconfirmation
            && schedule.sha256() == fit.body.schedule_sha256
            && schedule
                .episodes()
                .get(self.episode)
                .is_some_and(|e| e.role == DataRole::HeldOut)
            && self.bridge.as_ref().is_some_and(|b| b.seals.len() == 7)
            && self.session.rows.len() >= 64;
        if self.mapping.is_some() && !suffix_mapping {
            return Err("temporal_train_projection_mapping_overlaps_active_or_train".into());
        }
        if self.session.failed
            || self.session.active
            || self.stage == Stage::BlockedDurability
            || self.pending_intent.is_some()
            || self.pending_prefix.is_some()
            || self
                .async_state
                .as_ref()
                .is_none_or(|a| a.pending.is_some() || a.busy_pause || a.error.is_some())
            || self.durable_windows != self.session.rows.len()
            || fit != &verified.record
            || now < self.last_now_ns
            || now / 1_000_000 >= self.descriptor.expires_at_ms
            || mode.pending.is_some()
            || mode.pending_complete.is_some()
        {
            return Err("temporal_evidence_current_durable_owner_required".into());
        }
        let complete = if held_out {
            let fact = mode
                .completed
                .as_ref()
                .ok_or("temporal_actual_complete_required")?;
            let record = mode
                .complete
                .as_ref()
                .ok_or("temporal_complete_record_required")?;
            if fact.token != *token
                || fact.record_sha256 != record.sha256
                || mode.held_out_seals() != 8
                || self.session.rows.len() != 128
            {
                return Err("temporal_current_complete_changed".into());
            }
            Some(record.sha256.clone())
        } else {
            if !(64..=128).contains(&self.session.rows.len()) {
                return Err("temporal_stable_train_projection_required".into());
            }
            None
        };
        // Arc refs and bounded identity only. Native tensors/records, SHA work,
        // full model validation and disk access are deliberately not done here.
        Ok(PendingTemporalEvidenceSource {
            binding: TemporalEvaluationBinding {
                epoch: owner.runtime_epoch.clone(),
                generation: owner.generation,
                context: self.context().clone(),
                context_sha256: fit.body.context_digest.clone(),
                run: self.session.run_id.clone(),
                token: token.clone(),
                fit: fit.sha256.clone(),
                model: fit.body.artifact_sha256.clone(),
                complete,
            },
            model: verified.model.clone(),
            artifact: verified.store.clone(),
            context: self.context().clone(),
            start_floor: now,
            deadline: self.descriptor.expires_at_ms,
            held_out,
        })
    }
    pub(crate) fn current_temporal_evidence_matches(&self, b: &TemporalEvaluationBinding) -> bool {
        self.full_run.as_ref().is_some_and(|m| {
            m.owner
                .as_ref()
                .is_some_and(|o| o.runtime_epoch == b.epoch && o.generation == b.generation)
                && m.frozen
                    .as_ref()
                    .is_some_and(|f| f.sha256 == b.fit && f.body.artifact_sha256 == b.model)
                && b.complete.as_ref().is_none_or(|sha| {
                    m.completed
                        .as_ref()
                        .is_some_and(|c| c.record_sha256 == *sha && c.token == b.token)
                })
        }) && self.commit_token.as_ref() == Some(&b.token)
            && self.session.run_id == b.run
            && self.context() == &b.context
            && !self.session.failed
    }
}
fn commitment(
    value: &crate::room_learner::recovery_store::temporal_commitment::TemporalCommitmentData,
) -> Result<data::CommitmentData, String> {
    Ok(data::CommitmentData {
        generation: value.generation,
        revision: value
            .revision
            .try_into()
            .map_err(|_| "temporal_commitment_revision")?,
        run: value.run.clone(),
        id: value.id.clone(),
        commitment_sha256: value.commitment.clone(),
    })
}
impl PendingTemporalEvidenceSource {
    pub(crate) fn binding(&self) -> &TemporalEvaluationBinding {
        &self.binding
    }
    pub(crate) fn expires_at_ms(&self) -> u64 {
        self.deadline
    }
    fn check(&self, now: u64, cancel: &AtomicBool) -> Result<(), String> {
        if cancel.load(Ordering::Acquire) {
            return Err("temporal_evaluation_cancelled".into());
        }
        if now < self.start_floor || now / 1_000_000 >= self.deadline {
            return Err("temporal_evaluation_clock_or_original_deadline".into());
        }
        Ok(())
    }
    /// One-use actual-current-store recertification; structural decoders have
    /// no constructor for this source and cannot reach the authority return.
    pub(crate) fn recertify_evaluation(
        self,
        store: &RecoveryStore,
        now: u64,
        cancel: &AtomicBool,
    ) -> Result<VerifiedTemporalEvaluationSnapshot, String> {
        if !self.held_out {
            return Err("temporal_complete_projection_required".into());
        }
        let (binding, train, held, rows) = self.recertify(store, now, cancel)?;
        Ok(VerifiedTemporalEvaluationSnapshot {
            train,
            binding,
            held_out: held.ok_or("temporal_complete_evidence_required")?,
            rows,
        })
    }
    pub(crate) fn recertify_train(
        self,
        store: &RecoveryStore,
        now: u64,
        cancel: &AtomicBool,
    ) -> Result<VerifiedTemporalTrainSnapshot, String> {
        let (_, train, _, _) = self.recertify(store, now, cancel)?;
        Ok(train)
    }
    fn recertify(
        self,
        store: &RecoveryStore,
        now: u64,
        cancel: &AtomicBool,
    ) -> Result<
        (
            TemporalEvaluationBinding,
            VerifiedTemporalTrainSnapshot,
            Option<data::TemporalHeldOutEvidenceData>,
            Vec<SharedRecord<CaptureCohort>>,
        ),
        String,
    > {
        self.check(now, cancel)?;
        let current = store.recertify_temporal(&self.binding.token, now)?;
        self.check(now, cancel)?;
        let w = checkpoint::decode(current.prefix.bytes(), current.prefix.descriptor(), now)?;
        let mode = w
            .full_run
            .as_ref()
            .ok_or("temporal_full_run_no_upconversion")?;
        let record = mode
            .frozen
            .as_ref()
            .ok_or("temporal_verified_fit_record_required")?;
        if w.context() != &self.context
            || w.session.run_id != self.binding.run
            || record.sha256 != self.binding.fit
            || record.body.artifact_sha256 != self.binding.model
        {
            return Err("temporal_evaluation_current_source_changed".into());
        }
        let loaded = self
            .artifact
            .load_exact_temporal_candidate(&self.context, &self.binding.model)?;
        verify_model_binding(record, &loaded)?;
        verify_model_binding(record, &self.model)?;
        checkpoint::validate_temporal_evaluation_train_prefix(&w, record)?;
        self.check(now, cancel)?;
        let identity = data::TemporalEvaluationIdentityData {
            owner_scope: w.descriptor.owner_scope.clone(),
            installation_id: w.descriptor.installation_id.clone(),
            context_digest: w.context().digest(),
            room_digest: w.context().room_digest.clone(),
            run: w.session.run_id.clone(),
            original_deadline: data::OriginalDeadlineData {
                id: w.descriptor.id.clone(),
                created_at_unix_ms: w.descriptor.created_at_ms,
                expires_at_unix_ms: w.descriptor.expires_at_ms,
            },
            artifact_kind: MODEL_KIND.into(),
            artifact_sha256: self.binding.model.clone(),
            verified_fit_record_sha256: record.sha256.clone(),
            prefix_schema: PREFIX_SCHEMA.into(),
            metadata_kind: META_KIND.into(),
            continuation_policy_sha256: record.body.continuation_policy_sha256.clone(),
            input_policy_sha256: record.body.input_policy_sha256.clone(),
            execution_policy_sha256: record.body.execution_policy_sha256.clone(),
            schedule_sha256: record.body.schedule_sha256.clone(),
            configured_fit_sha256: record.body.configured_fit_sha256.clone(),
            source_ids: w
                .context()
                .sources
                .iter()
                .map(|s| s.source_id)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "temporal_evaluation_three_sources_required")?,
        };
        let seals = &w
            .bridge
            .as_ref()
            .ok_or("temporal_train_seals_required")?
            .seals;
        let proof =
            data::TemporalTrainProofData::from_body_data(data::TemporalTrainProofBodyData {
                identity: identity.clone(),
                training_data_sha256: record.body.training_data_sha256.clone(),
                training_evidence_sha256: record.body.input_evidence_sha256.clone(),
                training_identities_sha256: record.body.identity_evidence_sha256.clone(),
                train_seal_sha256s: seals
                    .iter()
                    .map(|s| s.sha256.clone())
                    .collect::<Vec<_>>()
                    .try_into()
                    .map_err(|_| "temporal_train_seven_seals_required")?,
                train_seal_chain_sha256: record.body.train_seal_chain_sha256.clone(),
                train_windows: 64,
                windows_per_condition: [16; 4],
                declared_episodes_per_condition: [1, 2, 2, 2],
                accepted_train_commitment: commitment(&record.body.accepted_train_commitment)?,
                freeze_prepare_receiver_ns: record.body.freeze_prepare_receiver_ns.to_string(),
                source_floors: record
                    .body
                    .source_floors
                    .iter()
                    .map(|(id, seq, ns)| data::FrozenSourceFloorData {
                        source_id: *id,
                        sequence: *seq,
                        receiver_ns: ns.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .try_into()
                    .map_err(|_| "temporal_evaluation_three_sources_required")?,
                claims: data::DiagnosticClaimsData::diagnostic_only(),
            })?;
        let held = if self.held_out {
            let complete = mode
                .complete
                .as_ref()
                .ok_or("temporal_actual_complete_required")?;
            if Some(&complete.sha256) != self.binding.complete.as_ref()
                || w.session.rows.len() != 128
                || current.token != self.binding.token
            {
                return Err("temporal_current_complete_changed".into());
            }
            let managed = w
                .session
                .managed
                .as_ref()
                .ok_or("managed_session_required")?;
            let hs = &mode
                .held_out
                .as_ref()
                .ok_or("temporal_heldout_seals_required")?
                .seals;
            let evidence =
                data::TemporalHeldOutEvidenceData::from_body_data(data::TemporalHeldOutBodyData {
                    identity,
                    training_proof_sha256: proof.sha256.clone(),
                    held_out_data_sha256: hash(&(&w.session.rows[64..]))?,
                    held_out_evidence_sha256: hash(&(&w.session.evidence[64..]))?,
                    held_out_identities_sha256: hash(&(&managed.windows[64..]))?,
                    complete_rows_sha256: complete.body.raw_sha256.clone(),
                    complete_evidence_sha256: complete.body.evidence_sha256.clone(),
                    complete_identities_sha256: complete.body.identity_sha256.clone(),
                    complete_ledger_sha256: complete.body.ledger_sha256.clone(),
                    held_out_seal_sha256s: hs
                        .iter()
                        .map(|s| s.sha256.clone())
                        .collect::<Vec<_>>()
                        .try_into()
                        .map_err(|_| "temporal_eight_heldout_seals_required")?,
                    held_out_seal_chain_sha256: complete.body.held_out_chain_sha256.clone(),
                    complete_record_sha256: complete.sha256.clone(),
                    complete_commitment: commitment(&current.token.data())?,
                    complete_prepare_receiver_ns: complete
                        .body
                        .complete_prepare_receiver_ns
                        .to_string(),
                    complete_persisted_receiver_ns: w.last_now_ns.to_string(),
                    final_source_fences: w
                        .source_high
                        .iter()
                        .map(|(id, seq, ns)| data::FinalSourceFenceData {
                            source_id: *id,
                            sequence: *seq,
                            receiver_ns: ns.to_string(),
                            native_frame_id: ((u128::from(*ns) << 40)
                                | (u128::from(*id) << 32)
                                | u128::from(*seq))
                            .to_string(),
                        })
                        .collect::<Vec<_>>()
                        .try_into()
                        .map_err(|_| "temporal_evaluation_three_sources_required")?,
                    held_out_windows: 64,
                    windows_per_condition: [16; 4],
                    declared_episodes_per_condition: [2; 4],
                    claims: data::DiagnosticClaimsData::diagnostic_only(),
                })?;
            evidence.validate_data_links(&proof)?;
            Some(evidence)
        } else {
            None
        };
        let rows = if self.held_out {
            w.session.rows[64..].to_vec()
        } else {
            vec![]
        };
        self.check(now, cancel)?;
        Ok((
            self.binding,
            VerifiedTemporalTrainSnapshot {
                model: self.model,
                artifact: self.artifact,
                proof,
            },
            held,
            rows,
        ))
    }
}

/// No serde/Clone/from-DATA constructor: computed only from the protected
/// snapshot and the SAME whole model's existing evaluator.
pub(crate) struct VerifiedTemporalEvaluationResult {
    artifact: Arc<ArtifactStore>,
    binding: TemporalEvaluationBinding,
    train: data::TemporalTrainProofData,
    held: data::TemporalHeldOutEvidenceData,
    receipt: data::TemporalEvaluationReceiptData,
}
impl VerifiedTemporalEvaluationResult {
    pub(crate) fn require_same_store(&self, store: &ArtifactStore) -> Result<(), String> {
        if !std::ptr::eq(store, self.artifact.as_ref()) {
            return Err("temporal_evaluation_same_store_required".into());
        }
        Ok(())
    }
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
}
impl VerifiedTemporalEvaluationSnapshot {
    pub(crate) fn compute_verified(
        &self,
        cancel: &AtomicBool,
        clock: &mut dyn FnMut() -> Result<u64, String>,
    ) -> Result<VerifiedTemporalEvaluationResult, String> {
        let before = clock()?;
        if cancel.load(Ordering::Acquire) {
            return Err("temporal_evaluation_cancelled".into());
        }
        let deadline = &self.proof().body.identity.original_deadline;
        if before / 1_000_000 >= deadline.expires_at_unix_ms {
            return Err("temporal_evaluation_original_deadline_expired".into());
        }
        let report = self.model().evaluate_managed_held_out(self)?;
        // Existing compute is bounded64 rows. Cancellation withdraws admission
        // and drains the entire call; no per-row interruption is claimed.
        let after = clock()?;
        if cancel.load(Ordering::Acquire)
            || after < before
            || after / 1_000_000 >= deadline.expires_at_unix_ms
        {
            return Err("temporal_evaluation_cancelled_or_clock".into());
        }
        let receipt = data::TemporalEvaluationReceiptData::from_body_data(
            data::TemporalEvaluationBodyData {
                identity: self.proof().body.identity.clone(),
                training_proof_sha256: self.proof().sha256.clone(),
                held_out_evidence_sha256: self.held_out().sha256.clone(),
                complete_record_sha256: self.held_out().body.complete_record_sha256.clone(),
                training_data_sha256: self.proof().body.training_data_sha256.clone(),
                held_out_data_sha256: self.held_out().body.held_out_data_sha256.clone(),
                role: DataRole::HeldOut,
                window_unit: "native_receiver_2s_window".into(),
                evaluated_at_receiver_ns: after.to_string(),
                evaluated_at_unix_ms: after / 1_000_000,
                provenance: vec![CaptureProvenance::LiveReceiverObserved],
                clock_claims: vec![ClockKind::ReceiverOnly],
                train_lineage_checked: true,
                report,
                claims: data::DiagnosticClaimsData::diagnostic_only(),
            },
        )?;
        receipt.validate_data_links(self.proof(), self.held_out())?;
        Ok(VerifiedTemporalEvaluationResult {
            artifact: self.train.artifact.clone(),
            binding: self.binding.clone(),
            train: self.proof().clone(),
            held: self.held_out().clone(),
            receipt,
        })
    }
}
