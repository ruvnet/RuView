//! Optional ACTUAL aggregate fit diagnostics in the SAME owner ArtifactStore.
//! Reuses dca8's report projection and finite/config/count/delta validation.
//! Never reconstructs historical outcomes or establishes qualification authority.
use super::temporal_artifacts::{bounded_read, immutable};
use super::*;
use crate::room_learner::{
    observation::CaptureContext,
    temporal_model_input::{TemporalFitBinding, TemporalModel},
    training::{FitConfig, ObjectiveValues, TrainingReport},
};
const SCHEMA: &str = "ruview.room-learner.temporal-fit-summary.v1";
const MAX_BYTES: usize = 16_384;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FitObjective {
    pub total: f32,
    pub contrastive: f32,
    pub supervised: f32,
}
impl From<ObjectiveValues> for FitObjective {
    fn from(v: ObjectiveValues) -> Self {
        Self {
            total: v.total,
            contrastive: v.contrastive,
            supervised: v.supervised,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FitMetrics {
    pub initial: FitObjective,
    pub final_objective: FitObjective,
    pub completed_epochs: usize,
    pub parameter_count: usize,
    pub parameter_delta_l2: f32,
    pub encoder_delta_l2: f32,
    pub backbone_delta_l2: f32,
    pub projection_delta_l2: f32,
    pub head_delta_l2: f32,
    pub objective_evaluations: usize,
    pub training_cohorts: usize,
    pub contrastive_trials: usize,
    pub configured: FitConfig,
    /// Actual TrainingReport has no elapsed measurement. Never copy its limit here.
    pub elapsed_ms: Option<u64>,
}
impl FitMetrics {
    fn from_report(r: &TrainingReport, c: &FitConfig) -> Self {
        Self {
            initial: r.initial.into(),
            final_objective: r.final_objective.into(),
            completed_epochs: r.completed_epochs,
            parameter_count: r.parameter_count,
            parameter_delta_l2: r.parameter_delta_l2,
            encoder_delta_l2: r.encoder_delta_l2,
            backbone_delta_l2: r.backbone_delta_l2,
            projection_delta_l2: r.projection_delta_l2,
            head_delta_l2: r.head_delta_l2,
            objective_evaluations: r.objective_evaluations,
            training_cohorts: r.training_cohorts,
            contrastive_trials: r.contrastive_trials,
            configured: c.clone(),
            elapsed_ms: None,
        }
    }
    fn validate(&self, b: &TemporalFitBinding, context: &CaptureContext) -> Result<(), String> {
        let c = &self.configured;
        c.validate_context(context)?;
        let finite = |o: &FitObjective| {
            [o.total, o.supervised, o.contrastive]
                .iter()
                .all(|v| v.is_finite() && *v >= 0.)
                && (f64::from(o.total)
                    - f64::from(c.supervised_weight) * f64::from(o.supervised)
                    - f64::from(c.contrastive_weight) * f64::from(o.contrastive))
                .abs()
                    <= 1e-5 * (1. + f64::from(o.total).abs())
        };
        if self.training_cohorts != b.training_cohorts
            || !(4..=64).contains(&self.training_cohorts)
            || !(2..=self.training_cohorts).contains(&self.contrastive_trials)
            || self.parameter_count != b.parameter_count
            || self.parameter_count > c.max_parameters
            || self.completed_epochs == 0
            || self.completed_epochs > c.epochs
            || self.objective_evaluations < 2
            || self.objective_evaluations >= c.max_objective_evaluations
            || c.hidden_dim != b.encoder.hidden_dim
            || c.embedding_dim != b.encoder.embedding_dim
            || c.temporal_tokens != b.encoder.temporal_tokens
            || c.temperature != b.encoder.temperature
            || self.elapsed_ms.is_some()
            || !finite(&self.initial)
            || !finite(&self.final_objective)
            || self.final_objective.total >= self.initial.total
            || ![
                self.parameter_delta_l2,
                self.encoder_delta_l2,
                self.backbone_delta_l2,
                self.projection_delta_l2,
                self.head_delta_l2,
            ]
            .iter()
            .all(|v| v.is_finite() && *v >= 0.)
            || self.parameter_delta_l2 <= 0.
            || self.encoder_delta_l2 <= 0.
            || self.backbone_delta_l2 <= 0.
            || self.projection_delta_l2 <= 0.
        {
            return Err("temporal_fit_report_invalid".into());
        }
        if let Some(bound) = &b.bound_fit_config {
            if serde_json::to_vec(bound).map_err(|_| "fit_config_encoding")?
                != serde_json::to_vec(c).map_err(|_| "fit_config_encoding")?
            {
                return Err("temporal_fit_configuration_changed".into());
            }
        }
        let norm = |whole: f32, a: f32, b: f32| {
            let expected = f64::from(a).hypot(f64::from(b));
            (f64::from(whole) - expected).abs() <= 1e-4 * (1. + expected)
        };
        if !norm(
            self.parameter_delta_l2,
            self.encoder_delta_l2,
            self.head_delta_l2,
        ) || !norm(
            self.encoder_delta_l2,
            self.backbone_delta_l2,
            self.projection_delta_l2,
        ) || f64::from(self.parameter_delta_l2) > 20_000. * (self.parameter_count as f64).sqrt()
        {
            return Err("temporal_fit_deltas_inconsistent".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemporalFitSummary {
    pub schema: String,
    pub binding: TemporalFitBinding,
    pub metrics: FitMetrics,
    pub diagnostic_only: bool,
    pub physically_validated: bool,
    pub qualification_granted: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    summary: TemporalFitSummary,
    summary_sha256: String,
}
fn small<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let b = serde_json::to_vec(value).map_err(|_| "temporal_fit_encoding")?;
    if b.len() > MAX_BYTES {
        return Err("temporal_fit_summary_budget".into());
    }
    Ok(b)
}
impl TemporalFitSummary {
    fn actual(
        model: &TemporalModel,
        report: &TrainingReport,
        configured: &FitConfig,
    ) -> Result<Self, String> {
        let v = Self {
            schema: SCHEMA.into(),
            binding: model.fit_diagnostic_binding()?,
            metrics: FitMetrics::from_report(report, configured),
            diagnostic_only: true,
            physically_validated: false,
            qualification_granted: false,
        };
        v.validate(model)?;
        Ok(v)
    }
    fn validate(&self, model: &TemporalModel) -> Result<(), String> {
        let expected = model.fit_diagnostic_binding()?;
        if self.schema != SCHEMA
            || !self.diagnostic_only
            || self.physically_validated
            || self.qualification_granted
            || small(&self.binding)? != small(&expected)?
        {
            return Err("temporal_fit_summary_binding_changed".into());
        }
        self.metrics.validate(&expected, model.context())
    }
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FitDiagnosticState {
    Recorded,
    NotRecorded,
    Unavailable,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TemporalFitDiagnostics {
    pub state: FitDiagnosticState,
    pub summary: Option<TemporalFitSummary>,
    pub warning: Option<&'static str>,
    pub diagnostic_only: bool,
    pub physically_validated: bool,
    pub qualification_granted: bool,
}
impl TemporalFitDiagnostics {
    fn state(
        state: FitDiagnosticState,
        summary: Option<TemporalFitSummary>,
        warning: Option<&'static str>,
    ) -> Self {
        Self {
            state,
            summary,
            warning,
            diagnostic_only: true,
            physically_validated: false,
            qualification_granted: false,
        }
    }
}
#[derive(Debug, Serialize)]
pub(crate) struct TemporalFitSaveResult {
    pub candidate: TemporalCandidateReceipt,
    pub fit_diagnostics: TemporalFitDiagnostics,
}
impl ArtifactStore {
    fn temporal_fit_summary_path(&self, model: &TemporalModel) -> Result<PathBuf, String> {
        let binding = model.fit_diagnostic_binding()?;
        self.validate_context(model.context())?;
        // Model must already exist as the WHOLE verified codec. Never call the old
        // inner-v1 loader or create a loose summary from a supplied hash.
        let loaded =
            self.load_exact_temporal_candidate(model.context(), &binding.artifact_sha256)?;
        if small(&loaded.fit_diagnostic_binding()?)? != small(&binding)? {
            return Err("temporal_fit_artifact_changed".into());
        }
        let directory = self
            .temporal_directory(model.context(), false)?
            .ok_or("temporal_fit_model_directory_missing")?;
        Ok(directory.join(format!("{}.fit-summary-v1.json", binding.artifact_sha256)))
    }
    fn save_actual_temporal_fit_summary(
        &self,
        model: &TemporalModel,
        report: &TrainingReport,
        configured: &FitConfig,
    ) -> Result<TemporalFitSummary, String> {
        let summary = TemporalFitSummary::actual(model, report, configured)?;
        let envelope = Envelope {
            summary_sha256: digest(&small(&summary)?),
            summary,
        };
        let bytes = small(&envelope)?;
        let path = self.temporal_fit_summary_path(model)?;
        immutable(&path, &bytes)?;
        let loaded = self
            .load_temporal_fit_summary(model)?
            .ok_or("temporal_fit_readback_missing")?;
        if small(&loaded)? != small(&envelope.summary)? {
            return Err("temporal_fit_readback_changed".into());
        }
        Ok(loaded)
    }
    pub(crate) fn load_temporal_fit_summary(
        &self,
        model: &TemporalModel,
    ) -> Result<Option<TemporalFitSummary>, String> {
        let path = self.temporal_fit_summary_path(model)?;
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("temporal_fit_summary_unavailable".into()),
            Ok(_) => {}
        }
        let bytes = bounded_read(&path, MAX_BYTES)?;
        let envelope: Envelope =
            serde_json::from_slice(&bytes).map_err(|_| "temporal_fit_summary_malformed")?;
        if digest(&small(&envelope.summary)?) != envelope.summary_sha256 {
            return Err("temporal_fit_summary_integrity".into());
        }
        envelope.summary.validate(model)?;
        Ok(Some(envelope.summary))
    }
    /// Coherent real-caller seam: a valid whole model is durable FIRST. A metrics
    /// failure is explicit optional diagnostics failure, never failed model save.
    pub(crate) fn save_temporal_candidate_with_fit_summary(
        &self,
        model: &TemporalModel,
        report: &TrainingReport,
        configured: &FitConfig,
    ) -> Result<TemporalFitSaveResult, String> {
        self.save_temporal_candidate_with_fit_summary_using(model,report,configured, |_|{})
    }
    #[cfg(test)]
    pub(crate) fn save_temporal_candidate_with_fit_summary_probe(
        &self,model:&TemporalModel,report:&TrainingReport,configured:&FitConfig,
        model_saved:impl FnOnce(&TemporalCandidateReceipt),
    )->Result<TemporalFitSaveResult,String>{
        self.save_temporal_candidate_with_fit_summary_using(model,report,configured,model_saved)
    }
    /// One shared real store body. The production wrapper supplies a no-op;
    /// the test seam can hold the genuine model-save/report-save boundary.
    fn save_temporal_candidate_with_fit_summary_using(
        &self,model:&TemporalModel,report:&TrainingReport,configured:&FitConfig,
        model_saved:impl FnOnce(&TemporalCandidateReceipt),
    )->Result<TemporalFitSaveResult,String>{
        let candidate = self.save_temporal_candidate(model)?;
        model_saved(&candidate);
        let fit_diagnostics=match self.save_actual_temporal_fit_summary(model,report,configured){Ok(summary)=>TemporalFitDiagnostics::state(FitDiagnosticState::Recorded,Some(summary),None),Err(_)=>TemporalFitDiagnostics::state(FitDiagnosticState::NotRecorded,None,Some("The model was saved, but actual aggregate fit diagnostics were not recorded; existing files were retained."))};
        Ok(TemporalFitSaveResult {
            candidate,
            fit_diagnostics,
        })
    }
    /// Future OWNER-side status projection only. Missing history is unknown, never
    /// zeros/defaults or an inferred TrainingReport; paired progress stays unchanged.
    pub(crate) fn temporal_fit_diagnostics(&self, model: &TemporalModel) -> TemporalFitDiagnostics {
        match self.load_temporal_fit_summary(model){Ok(Some(summary))=>TemporalFitDiagnostics::state(FitDiagnosticState::Recorded,Some(summary),None),Ok(None)=>TemporalFitDiagnostics::state(FitDiagnosticState::NotRecorded,None,None),Err(_)=>TemporalFitDiagnostics::state(FitDiagnosticState::Unavailable,None,Some("Saved fit diagnostics could not be verified; the model and existing files were retained."))}
    }
}

/// Typed rehash for tampering tests: exercise semantic validation AFTER integrity,
/// never merely hash a serde_json::Value with different key/float encoding.
#[cfg(test)]
pub(crate) fn rehash_fixture(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| "fixture_schema")?;
    envelope.summary_sha256 = digest(&small(&envelope.summary)?);
    small(&envelope)
}
