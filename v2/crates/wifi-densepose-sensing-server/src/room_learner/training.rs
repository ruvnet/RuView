//! Bounded numerical optimization using the production trainer machinery.
use super::model::{
    center_empty_heads, extractor, initial_encoder, initial_projection_weights, EncoderConfig,
    RoomModel, SourceProfile, TrainingManifest,
};
use super::observation::{CaptureCohort, CaptureContext, RoomLabel, ValidatedDataset};
use crate::embedding::info_nce_loss;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[path = "optimization.rs"]
mod optimization;

#[path = "fit_config.rs"]
mod fit_config;
pub use fit_config::FitConfig;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ObjectiveValues {
    pub total: f32,
    pub contrastive: f32,
    pub supervised: f32,
}
#[path = "evaluation_report.rs"]
mod evaluation_report;
pub use evaluation_report::{
    BinaryConfusion, ConditionEvaluation, EvaluationRates, EvaluationReport,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingReport {
    pub initial: ObjectiveValues,
    pub final_objective: ObjectiveValues,
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
    pub validation: Option<EvaluationReport>,
    pub evidence: String,
}
struct Row {
    tokens: Vec<Vec<Vec<f32>>>,
    occupied: f32,
    contrastive: bool,
}
fn occupied(label: RoomLabel) -> f32 {
    if matches!(label, RoomLabel::OccupiedStatic | RoomLabel::OccupiedMoving) {
        1.0
    } else {
        0.0
    }
}
fn bce(logit: f32, target: f32) -> f32 {
    logit.max(0.0) - logit * target + (-logit.abs()).exp().ln_1p()
}
fn normalized(mut embedding: Vec<f32>) -> Vec<f32> {
    let norm = embedding
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt()
        .max(1.0e-8);
    embedding.iter_mut().for_each(|x| *x /= norm);
    embedding
}
fn objective(
    model: &RoomModel,
    rows: &[Row],
    params: &[f32],
    config: &FitConfig,
) -> Result<ObjectiveValues, String> {
    let mut supervised = 0.0;
    let mut by_source = vec![Vec::new(); model.profiles.len()];
    let head = &params[model.representation_parameter_count()..];
    let projectors = model.source_projectors(params)?;
    let mut encoder = extractor(&model.config);
    encoder
        .transformer
        .unflatten_weights(&params[..model.encoder_weights.len()])?;
    for row in rows {
        let (features, embeddings) =
            model.represent_with_encoder(&row.tokens, &mut encoder, &projectors)?;
        supervised += bce(model.logit(&features, head), row.occupied);
        // One event per independent trial per batch. Sibling/overlapping windows
        // in the same trial cannot accidentally become contrastive negatives.
        if row.contrastive {
            for (source, embedding) in by_source.iter_mut().zip(embeddings) {
                source.push(normalized(embedding));
            }
        }
    }
    supervised /= rows.len() as f32;
    let mut contrastive = 0.0;
    // A source-zero star aligns every source without O(source_count^2) pairs.
    // Both directions are optimized; each row's other source is its positive.
    for other in by_source.iter().skip(1) {
        contrastive += (info_nce_loss(&by_source[0], other, config.temperature)
            + info_nce_loss(other, &by_source[0], config.temperature))
            * 0.5;
    }
    contrastive /= (by_source.len() - 1) as f32;
    let total = supervised * config.supervised_weight + contrastive * config.contrastive_weight;
    if !total.is_finite() {
        return Err("non-finite training objective".into());
    }
    Ok(ObjectiveValues {
        total,
        supervised,
        contrastive,
    })
}
fn profiles(dataset: &ValidatedDataset) -> Result<Vec<SourceProfile>, String> {
    let mut profiles = Vec::new();
    for source in &dataset.context().sources {
        let width = source.feature_count;
        let mut mean = vec![0.0f64; width];
        let mut square = vec![0.0f64; width];
        let mut count = 0usize;
        for event in dataset
            .train()
            .iter()
            .filter(|c| c.label == RoomLabel::Empty)
        {
            let view = event
                .views
                .iter()
                .find(|v| v.source_id == source.source_id)
                .ok_or("missing baseline source")?;
            for frame in &view.samples {
                count += 1;
                for (k, &v) in frame.iter().enumerate() {
                    mean[k] += v as f64;
                    square[k] += (v as f64).powi(2);
                }
            }
        }
        if count < 2 {
            return Err("each source needs Train Empty baseline frames".into());
        }
        let scale: Vec<f32> = mean
            .iter()
            .zip(&square)
            .map(|(m, s)| {
                (s / count as f64 - (m / count as f64).powi(2))
                    .max(0.0)
                    .sqrt()
                    .max(0.1) as f32
            })
            .collect();
        let mean: Vec<f32> = mean.iter().map(|m| (m / count as f64) as f32).collect();
        let mut max_normalized = 1.0f32;
        for event in dataset.train() {
            let view = event
                .views
                .iter()
                .find(|v| v.source_id == source.source_id)
                .ok_or("missing training source")?;
            for frame in &view.samples {
                for (k, v) in frame.iter().enumerate() {
                    max_normalized = max_normalized.max(((v - mean[k]) / scale[k]).abs());
                }
            }
        }
        profiles.push(SourceProfile {
            source: source.clone(),
            mean,
            scale,
            max_normalized,
        });
    }
    Ok(profiles)
}
impl RoomModel {
    pub fn fit(
        dataset: &ValidatedDataset,
        config: FitConfig,
    ) -> Result<(Self, TrainingReport), String> {
        Self::fit_with_cancel(dataset, config, &AtomicBool::new(false))
    }
    pub fn fit_with_cancel(
        dataset: &ValidatedDataset,
        config: FitConfig,
        cancel: &AtomicBool,
    ) -> Result<(Self, TrainingReport), String> {
        let started = Instant::now();
        config.validate()?;
        if cancel.load(Ordering::Relaxed) {
            return Err("training cancelled".into());
        }
        if !(4..=64).contains(&dataset.train().len()) {
            return Err("training requires 4..64 cohorts".into());
        }
        let trial_count = dataset
            .train()
            .iter()
            .map(|c| &c.trial_id)
            .collect::<HashSet<_>>()
            .len();
        if trial_count < 2
            || !dataset.train().iter().any(|c| occupied(c.label) == 1.0)
            || !dataset
                .train()
                .iter()
                .any(|c| c.label == RoomLabel::AdjacentOnly)
        {
            return Err(
                "training needs independent trials, occupied and adjacent-only controls".into(),
            );
        }
        let encoder_config = EncoderConfig {
            source_count: dataset.context().sources.len(),
            bins: dataset
                .context()
                .sources
                .iter()
                .map(|s| s.feature_count)
                .max()
                .unwrap_or(0),
            hidden_dim: config.hidden_dim,
            embedding_dim: config.embedding_dim,
            temporal_tokens: config.temporal_tokens,
            temperature: config.temperature,
        };
        if !(1..=128).contains(&encoder_config.bins) {
            return Err("source width exceeds bounded encoder".into());
        }
        let encoder = initial_encoder(&encoder_config)?;
        let mut model = Self {
            version: 1,
            algorithm: "aether_multiview_infonce_room_bce_v1".into(),
            context: dataset.context().clone(),
            training_manifest: TrainingManifest::from_training(dataset.train())?,
            source_projection_weights: initial_projection_weights(&encoder_config),
            config: encoder_config,
            profiles: profiles(dataset)?,
            encoder_weights: encoder.transformer.flatten_weights(),
            head_weights: vec![
                0.0;
                dataset.context().sources.len() * (config.embedding_dim + 2) + 1
            ],
        };
        model.validate()?;
        if model.parameter_count() > config.max_parameters {
            return Err("model exceeds parameter budget".into());
        }
        center_empty_heads(&mut model, dataset.train())?;
        model.validate()?;
        let mut independent: Vec<&CaptureCohort> = Vec::new();
        let mut rows = Vec::new();
        for c in dataset.train() {
            let contrastive = !independent.iter().any(|other| {
                other.trial_id == c.trial_id
                    || c.views.iter().any(|v| {
                        other.views.iter().any(|w| {
                            v.source_id == w.source_id
                                && (v.frame_ids.iter().any(|id| w.frame_ids.contains(id))
                                    || (v.first_time_ns <= w.last_time_ns
                                        && v.last_time_ns >= w.first_time_ns))
                        })
                    })
            });
            if contrastive {
                independent.push(c);
            }
            rows.push(Row {
                tokens: model.tokens(c, false)?,
                occupied: occupied(c.label),
                contrastive,
            });
        }
        let contrastive_trials = independent.len();
        if contrastive_trials < 2 {
            return Err("contrastive objective needs non-overlapping independent trials".into());
        }
        let initial_params = model.parameters();
        let mut params = initial_params.clone();
        let initial = objective(&model, &rows, &params, &config)?;
        let mut current = initial;
        let evaluations = Cell::new(1usize);
        let check = || -> Result<(), String> {
            if cancel.load(Ordering::Relaxed) {
                Err("training cancelled".into())
            } else if started.elapsed().as_millis() >= config.max_duration_ms as u128 {
                Err("training time budget exhausted".into())
            } else if evaluations.get() >= config.max_objective_evaluations {
                Err("objective evaluation budget exhausted".into())
            } else {
                Ok(())
            }
        };
        let mut epochs = 0;
        for _ in 0..config.epochs {
            check()?;
            let next = optimization::epoch_step(
                &model,
                &rows,
                &params,
                &config,
                current,
                &check,
                &evaluations,
            )?;
            if let Some((next, loss)) = next {
                params = next;
                current = loss;
            } else {
                break;
            }
            epochs += 1;
        }
        check()?;
        if epochs == 0 || params == initial_params {
            return Err("optimizer made no improving parameter update".into());
        }
        optimization::require_rf_update(&model, &params, &initial_params)?;
        model.set_parameters(&params)?;
        model.validate()?;
        let delta = |a: &[f32], b: &[f32]| {
            a.iter()
                .zip(b)
                .map(|(x, y)| (x - y).powi(2))
                .sum::<f32>()
                .sqrt()
        };
        let split = model.representation_parameter_count();
        let report = TrainingReport {
            initial,
            final_objective: current,
            completed_epochs: epochs,
            parameter_count: params.len(),
            parameter_delta_l2: delta(&params, &initial_params),
            encoder_delta_l2: delta(&params[..split], &initial_params[..split]),
            backbone_delta_l2: delta(
                &params[..model.encoder_weights.len()],
                &initial_params[..model.encoder_weights.len()],
            ),
            projection_delta_l2: delta(
                &params[model.encoder_weights.len()..split],
                &initial_params[model.encoder_weights.len()..split],
            ),
            head_delta_l2: delta(&params[split..], &initial_params[split..]),
            objective_evaluations: evaluations.get(),
            training_cohorts: rows.len(),
            contrastive_trials,
            validation: if dataset.validation().is_empty() {
                None
            } else {
                Some(model.evaluate(dataset.validation())?)
            },
            evidence: "EXPERIMENTAL_RF_ONLY; physical accuracy and promotion are unqualified"
                .into(),
        };
        check()?;
        Ok((model, report))
    }
    /// Explicit evaluation only; fit never reads HeldOut observations.
    pub fn evaluate(&self, cohorts: &[CaptureCohort]) -> Result<EvaluationReport, String> {
        EvaluationReport::compute(self, cohorts)
    }
}

#[cfg(test)]
#[path = "learning_tests.rs"]
pub(crate) mod learning_tests;

#[cfg(test)]
#[path = "source_conditioning_tests.rs"]
mod source_conditioning_tests;
