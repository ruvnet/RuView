//! Private native-session provenance; no imported rows or invented phase acknowledgements.
use super::*;
use crate::room_learner::{
    managed_training::{
        commitment, ManagedTrainingProof, PhaseClaim, SourceWatermark, TrainingProofSeed,
        TrainingSourceRange,
    },
    qualification::{hash, CONDITIONS},
};
impl CaptureSession {
    pub(super) fn sealed_cohort_capacity_used(&self) -> usize {
        self.metadata_lineage.as_ref().map_or(0, |s| {
            s.phase_claims.iter().map(|p| p.cohort_count).sum::<usize>()
        }) + self.cohorts.len()
    }
    pub(crate) fn minimum_receiver_time_ns(&self) -> u64 {
        self.last_seen_ns
    }
    pub(crate) fn metadata_lineage(&self) -> Option<&TrainingProofSeed> {
        self.metadata_lineage.as_ref()
    }
    pub(super) fn phase_index(&self) -> usize {
        self.metadata_lineage
            .as_ref()
            .map_or(0, |s| s.phase_claims.len())
            + self.history.len()
    }
    pub(crate) fn managed_training_seed(&self) -> Result<TrainingProofSeed, String> {
        if self.phase_active() || self.metadata_lineage.is_some() {
            return Err(
                "managed training proof requires original completed native Train capture".into(),
            );
        }
        // Validate the actual capture's frame namespace, source counters, phase
        // identity and sealed-row correspondence, not a caller's Live claim.
        self.checkpoint(self.last_seen_ns)?;
        let dataset = self.validated_dataset()?;
        let rows = dataset.train();
        let counts = CONDITIONS.map(|label| rows.iter().filter(|r| r.label == label).count());
        if counts.iter().any(|n| *n < COHORTS_PER_PHASE) {
            return Err("managed fit requires four sealed Train cohorts per condition".into());
        }
        let mut phases = Vec::new();
        for row in rows {
            let matches: Vec<_> = self
                .history
                .iter()
                .filter(|p| {
                    p.role == DataRole::Train
                        && p.status == PhaseStatus::Sealed
                        && p.trial_id == row.trial_id
                        && p.session_id == row.session_id
                        && p.label == row.label
                })
                .collect();
            if matches.len() != 1
                || rows
                    .iter()
                    .filter(|r| {
                        r.trial_id == row.trial_id
                            && r.session_id == row.session_id
                            && r.label == row.label
                    })
                    .count()
                    != matches[0].cohort_count
            {
                return Err("Train rows lack unique managed sealed acknowledgement".into());
            }
            if !phases.iter().any(|p: &&PhaseReceipt| {
                p.trial_id == row.trial_id && p.session_id == row.session_id
            }) {
                phases.push(matches[0]);
            }
        }
        // Hash identities, times and native metadata; RF sample values never
        // enter the metadata artifact (the existing Train SHA remains exact).
        let metadata: Vec<_> = rows
            .iter()
            .map(|r| {
                (
                    &r.id,
                    &r.trial_id,
                    &r.session_id,
                    r.role,
                    r.label,
                    r.views
                        .iter()
                        .map(|v| {
                            (
                                v.source_id,
                                &v.frame_ids,
                                &v.sequence_ids,
                                v.first_time_ns,
                                v.last_time_ns,
                                v.provenance,
                                v.clock,
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let seed = TrainingProofSeed {
            context: self.context.clone(),
            run_id: self.run_id.clone(),
            training_data_sha256: hash(&rows)?,
            training_metadata_sha256: hash(&metadata)?,
            sealed_train_history_sha256: hash(&phases)?,
            train_counts: counts,
            phase_claims: self
                .history
                .iter()
                .map(|p| {
                    Ok(PhaseClaim {
                        trial_sha256: commitment("trial", &p.trial_id)?,
                        session_sha256: commitment("session", &p.session_id)?,
                        role: p.role,
                        label: p.label,
                        status: p.status,
                        cohort_count: p.cohort_count,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
            sources: self
                .context
                .sources
                .iter()
                .map(|s| {
                    let w = &self.sources[&s.source_id];
                    Ok(SourceWatermark {
                        source_id: s.source_id,
                        sequence: w.sequence.ok_or("missing original wire sequence")?,
                        ordinal: w.ordinal,
                        last_time_ns: w.last_time_ns,
                        admitted_packets: w.admitted_packets,
                        train_range: TrainingSourceRange {
                            first_ordinal: rows
                                .iter()
                                .flat_map(|c| &c.views)
                                .filter(|v| v.source_id == s.source_id)
                                .flat_map(|v| &v.sequence_ids)
                                .copied()
                                .min()
                                .ok_or("missing Train ordinal")?,
                            last_ordinal: rows
                                .iter()
                                .flat_map(|c| &c.views)
                                .filter(|v| v.source_id == s.source_id)
                                .flat_map(|v| &v.sequence_ids)
                                .copied()
                                .max()
                                .ok_or("missing Train ordinal")?,
                            first_time_ns: rows
                                .iter()
                                .flat_map(|c| &c.views)
                                .filter(|v| v.source_id == s.source_id)
                                .map(|v| v.first_time_ns)
                                .min()
                                .ok_or("missing Train time")?,
                            last_time_ns: rows
                                .iter()
                                .flat_map(|c| &c.views)
                                .filter(|v| v.source_id == s.source_id)
                                .map(|v| v.last_time_ns)
                                .max()
                                .ok_or("missing Train time")?,
                        },
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
            first_observed_ns: rows
                .iter()
                .flat_map(|c| &c.views)
                .map(|v| v.first_time_ns)
                .min()
                .ok_or("no managed Train time")?,
            last_seen_ns: self.last_seen_ns,
            source_epoch_verified: false,
            receiver_clock_domain: "utc_anchored_monotonic_v1".into(),
        };
        seed.validate()?;
        Ok(seed)
    }
    pub(crate) fn resume_managed_proof(proof: &ManagedTrainingProof) -> Result<Self, String> {
        proof.seed.validate()?;
        let seed = &proof.seed;
        let mut session = Self::new(seed.context.clone(), seed.run_id.clone())?;
        session.last_seen_ns = seed.last_seen_ns;
        for w in &seed.sources {
            session.sources.insert(
                w.source_id,
                SourceBuffer {
                    sequence: Some(w.sequence),
                    ordinal: w.ordinal,
                    last_time_ns: w.last_time_ns,
                    admitted_packets: w.admitted_packets,
                    ..SourceBuffer::default()
                },
            );
        }
        session.metadata_lineage = Some(seed.clone());
        Ok(session)
    }
}
impl CaptureSession {
    pub(crate) fn managed_fence_seed(
        &self,
        proof: &ManagedTrainingProof,
    ) -> Result<TrainingProofSeed, String> {
        if self.context != proof.seed.context || self.run_id != proof.seed.run_id {
            return Err("capture has a different managed namespace".into());
        }
        let mut seed = proof.seed.clone();
        seed.phase_claims = self
            .metadata_lineage
            .as_ref()
            .map_or_else(Vec::new, |m| m.phase_claims.clone());
        for p in &self.history {
            seed.phase_claims.push(PhaseClaim {
                trial_sha256: commitment("trial", &p.trial_id)?,
                session_sha256: commitment("session", &p.session_id)?,
                role: p.role,
                label: p.label,
                status: p.status,
                cohort_count: p.cohort_count,
            });
        }
        seed.sources = self
            .context
            .sources
            .iter()
            .map(|s| {
                let w = &self.sources[&s.source_id];
                Ok(SourceWatermark {
                    source_id: s.source_id,
                    sequence: w.sequence.ok_or("missing original source sequence")?,
                    ordinal: w.ordinal,
                    last_time_ns: w.last_time_ns,
                    admitted_packets: w.admitted_packets,
                    train_range: proof
                        .seed
                        .sources
                        .iter()
                        .find(|old| old.source_id == s.source_id)
                        .ok_or("unknown original Train range")?
                        .train_range
                        .clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        seed.last_seen_ns = self.last_seen_ns;
        seed.ensure_extends(&proof.seed)?;
        if let Some(original) = &self.metadata_lineage {
            seed.ensure_extends(original)?;
        }
        Ok(seed)
    }
    pub(crate) fn resume_managed_fence(
        proof: &ManagedTrainingProof,
        fence: &crate::room_learner::managed_capture_fences::ManagedCaptureFence,
    ) -> Result<Self, String> {
        fence.validate(proof)?;
        let mut session = Self::resume_managed_proof(proof)?;
        session.last_seen_ns = fence.seed.last_seen_ns;
        for w in &fence.seed.sources {
            session.sources.insert(
                w.source_id,
                SourceBuffer {
                    sequence: Some(w.sequence),
                    ordinal: w.ordinal,
                    last_time_ns: w.last_time_ns,
                    admitted_packets: w.admitted_packets,
                    ..SourceBuffer::default()
                },
            );
        }
        session.metadata_lineage = Some(fence.seed.clone());
        Ok(session)
    }
}
