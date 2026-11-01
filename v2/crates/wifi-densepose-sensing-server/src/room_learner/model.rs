//! Portable RF-only room presence learner. No raw observations are persisted.
use super::observation::{CaptureCohort, CaptureContext, SourceSpec};
use crate::embedding::{EmbeddingConfig, EmbeddingExtractor};
use crate::graph_transformer::{CrossAttention, Linear, TransformerConfig};
use serde::{Deserialize, Serialize};

#[path = "manifest.rs"]
mod manifest;
pub use manifest::{ConsumedChannel, TrainingManifest};
#[path = "projection.rs"]
mod projection;
pub(crate) use projection::{center_empty_heads, initial_weights as initial_projection_weights};

const MAX_ARTIFACT_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SourceProfile {
    pub source: SourceSpec,
    pub mean: Vec<f32>,
    pub scale: Vec<f32>,
    pub max_normalized: f32,
}

/// Versioned, bounded architecture. Values are checked before allocating a network.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EncoderConfig {
    pub bins: usize,
    /// Stable ordered context sources; metadata tags are not measured RF bins.
    pub source_count: usize,
    pub temporal_tokens: usize,
    pub hidden_dim: usize,
    pub embedding_dim: usize,
    pub temperature: f32,
}
impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            bins: 56,
            source_count: 2,
            temporal_tokens: 4,
            hidden_dim: 3,
            embedding_dim: 3,
            temperature: 0.3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomModel {
    pub(crate) version: u32,
    pub(crate) algorithm: String,
    pub(crate) context: CaptureContext,
    pub(crate) training_manifest: TrainingManifest,
    pub(crate) config: EncoderConfig,
    pub(crate) profiles: Vec<SourceProfile>,
    pub(crate) encoder_weights: Vec<f32>,
    pub(crate) source_projection_weights: Vec<Vec<f32>>,
    pub(crate) head_weights: Vec<f32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresenceDecision {
    Empty,
    Occupied,
    Abstain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomPrediction {
    pub decision: PresenceDecision,
    pub occupied_probability: Option<f32>,
    pub reason: Option<String>,
    /// Always false: a trained candidate is not physical qualification evidence.
    pub physically_validated: bool,
}
impl RoomPrediction {
    fn abstain(reason: String) -> Self {
        Self {
            decision: PresenceDecision::Abstain,
            occupied_probability: None,
            reason: Some(reason),
            physically_validated: false,
        }
    }
}

pub(crate) fn extractor(config: &EncoderConfig) -> EmbeddingExtractor {
    EmbeddingExtractor::new(
        TransformerConfig {
            n_subcarriers: config.bins + config.source_count,
            n_keypoints: 17,
            d_model: config.hidden_dim,
            n_heads: 1,
            n_gnn_layers: 1,
        },
        EmbeddingConfig {
            d_model: config.hidden_dim,
            d_proj: config.embedding_dim,
            temperature: config.temperature,
            normalize: false,
        },
    )
}

/// Keep the tiny reused ReLU graph active around the calibrated empty baseline.
/// Layout follows CsiToPoseTransformer::flatten_weights' documented ordering.
pub(crate) fn initial_encoder(config: &EncoderConfig) -> Result<EmbeddingExtractor, String> {
    let mut encoder = extractor(config);
    let d = config.hidden_dim;
    let mut weights = encoder.transformer.flatten_weights();
    // A separate fan-in for source metadata keeps its scale independent of
    // how many canonical RF bins are present. This assumes no response polarity.
    let input_width = config.bins + config.source_count;
    let tags = Linear::with_seed(config.source_count, d, 501);
    for row in 0..d {
        for source in 0..config.source_count {
            weights[row * input_width + config.bins + source] = tags.weights()[row][source];
        }
    }
    let gnn = (config.bins + config.source_count + 1) * d
        + 17 * d
        + CrossAttention::new(d, 1).param_count();
    for r in 0..d {
        for c in 0..d {
            weights[gnn + r * d + c] = if r == c { 1.0 } else { 0.0 };
        }
    }
    weights[gnn + d * d..gnn + d * d + d].fill(1.0);
    encoder.transformer.unflatten_weights(&weights)?;
    Ok(encoder)
}

impl RoomModel {
    pub fn context(&self) -> &CaptureContext {
        &self.context
    }
    pub fn training_manifest(&self) -> &TrainingManifest {
        &self.training_manifest
    }
    pub fn profiles(&self) -> &[SourceProfile] {
        &self.profiles
    }
    pub fn encoder_config(&self) -> &EncoderConfig {
        &self.config
    }
    pub fn parameter_count(&self) -> usize {
        self.representation_parameter_count() + self.head_weights.len()
    }
    pub fn representation_parameter_count(&self) -> usize {
        self.encoder_weights.len()
            + self
                .source_projection_weights
                .iter()
                .map(Vec::len)
                .sum::<usize>()
    }
    /// Read-only learned parameters for independent audits and regression oracles.
    pub fn parameters(&self) -> Vec<f32> {
        self.encoder_weights
            .iter()
            .chain(self.source_projection_weights.iter().flatten())
            .chain(&self.head_weights)
            .copied()
            .collect()
    }
    pub(crate) fn set_parameters(&mut self, values: &[f32]) -> Result<(), String> {
        if values.len() != self.parameter_count() || !values.iter().all(|x| x.is_finite()) {
            return Err("invalid model parameter vector".into());
        }
        let mut offset = self.encoder_weights.len();
        self.encoder_weights.copy_from_slice(&values[..offset]);
        for weights in &mut self.source_projection_weights {
            let end = offset + weights.len();
            weights.copy_from_slice(&values[offset..end]);
            offset = end;
        }
        self.head_weights.copy_from_slice(&values[offset..]);
        Ok(())
    }
    pub fn export(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_ARTIFACT_BYTES {
            return Err("model artifact too large".into());
        }
        Ok(bytes)
    }
    pub fn import(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_ARTIFACT_BYTES {
            return Err("model artifact too large".into());
        }
        let model: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        model.validate()?;
        Ok(model)
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.training_manifest.validate()?;
        if self
            .context
            .sources
            .iter()
            .any(|s| s.signal_kind == ruview_unified::RfModality::Synthetic)
            && self.training_manifest.training_provenance
                != [super::observation::CaptureProvenance::Synthetic]
        {
            return Err("synthetic modality cannot have live training provenance".into());
        }
        if self.version != 1
            || self.algorithm != "aether_multiview_infonce_room_bce_v1"
            || !(1..=128).contains(&self.config.bins)
            || !(2..=8).contains(&self.config.source_count)
            || self.config.source_count != self.context.sources.len()
            || !(2..=16).contains(&self.config.temporal_tokens)
            || !(2..=4).contains(&self.config.hidden_dim)
            || !(2..=4).contains(&self.config.embedding_dim)
            || !self.config.temperature.is_finite()
            || !(0.1..=2.0).contains(&self.config.temperature)
        {
            return Err("unsupported room model version or architecture".into());
        }
        if !(2..=8).contains(&self.context.sources.len())
            || self.profiles.len() != self.context.sources.len()
        {
            return Err("invalid source profile count".into());
        }
        // Validate the context through the same trusted contract as capture.
        self.context.validate()?;
        if self
            .context
            .sources
            .iter()
            .any(|s| s.feature_count != self.config.bins)
        {
            return Err("encoder width does not match source grids".into());
        }
        for (profile, source) in self.profiles.iter().zip(&self.context.sources) {
            if &profile.source != source
                || profile.mean.len() != source.feature_count
                || profile.scale.len() != source.feature_count
                || !profile.mean.iter().all(|x| x.is_finite())
                || !profile.scale.iter().all(|x| x.is_finite() && *x >= 0.01)
                || !profile.max_normalized.is_finite()
                || !(1.0..=1.0e6).contains(&profile.max_normalized)
            {
                return Err("invalid normalization profile".into());
            }
        }
        if self.encoder_weights.len() != extractor(&self.config).transformer.param_count()
            || self.source_projection_weights.len() != self.config.source_count
            || self.source_projection_weights.iter().any(|w| {
                w.len()
                    != crate::embedding::ProjectionHead::new(projection::config(&self.config))
                        .param_count()
            })
            || self.head_weights.len() != self.profiles.len() * (self.config.embedding_dim + 2) + 1
            || self.parameter_count() > 512
            || !self
                .parameters()
                .iter()
                .all(|x| x.is_finite() && x.abs() <= 1.0e4)
        {
            return Err("invalid learned weights".into());
        }
        Ok(())
    }
    pub fn infer(&self, cohort: &CaptureCohort) -> RoomPrediction {
        let result = (|| {
            self.validate()?;
            let tokens = self.tokens(cohort, true)?;
            let (features, _) = self.represent(&tokens, &self.parameters())?;
            let probability = sigmoid(self.logit(&features, &self.head_weights));
            if !probability.is_finite() {
                return Err("non-finite prediction".into());
            }
            Ok(probability)
        })();
        match result {
            Err(e) => RoomPrediction::abstain(e),
            Ok(p) => RoomPrediction {
                occupied_probability: Some(p),
                decision: if (0.45..=0.55).contains(&p) {
                    PresenceDecision::Abstain
                } else if p > 0.5 {
                    PresenceDecision::Occupied
                } else {
                    PresenceDecision::Empty
                },
                reason: if (0.45..=0.55).contains(&p) {
                    Some("uncertain task probability".into())
                } else {
                    None
                },
                physically_validated: false,
            },
        }
    }
    pub(crate) fn tokens(
        &self,
        cohort: &CaptureCohort,
        check_range: bool,
    ) -> Result<Vec<Vec<Vec<f32>>>, String> {
        super::observation::validate_cohort(&self.context, cohort)?;
        let mut result = Vec::with_capacity(self.profiles.len());
        for (source_index, profile) in self.profiles.iter().enumerate() {
            let view = cohort
                .views
                .iter()
                .find(|v| v.source_id == profile.source.source_id)
                .ok_or("missing trained source")?;
            if view.samples.len() < self.config.temporal_tokens {
                return Err("insufficient measured temporal rows".into());
            }
            let bins = self.config.bins;
            let time = self.config.temporal_tokens;
            let mut tokens = vec![vec![0.0; bins]; time];
            let mut counts = vec![vec![0usize; bins]; time];
            for (t, frame) in view.samples.iter().enumerate() {
                let ti = t * time / view.samples.len();
                for (k, value) in frame.iter().enumerate() {
                    let value = (*value - profile.mean[k]) / profile.scale[k];
                    if !value.is_finite()
                        || (check_range && value.abs() > profile.max_normalized * 2.0 + 4.0)
                    {
                        return Err("RF input outside trained support".into());
                    }
                    let bi = k;
                    tokens[ti][bi] += value;
                    counts[ti][bi] += 1;
                }
            }
            for t in 0..time {
                for b in 0..bins {
                    if counts[t][b] > 0 {
                        tokens[t][b] /= counts[t][b] as f32;
                    }
                }
            }
            if !tokens.iter().flatten().all(|x| x.is_finite()) {
                return Err("invalid RF tokens".into());
            }
            // Source identity is a conditioning channel, appended only after
            // RF baseline normalization; metadata never enters RF statistics.
            for token in &mut tokens {
                token.extend((0..self.config.source_count).map(|i| {
                    if i == source_index {
                        1.0
                    } else {
                        0.0
                    }
                }));
            }
            result.push(tokens);
        }
        Ok(result)
    }
    pub(crate) fn represent(
        &self,
        tokens: &[Vec<Vec<f32>>],
        params: &[f32],
    ) -> Result<(Vec<f32>, Vec<Vec<f32>>), String> {
        let mut encoder = extractor(&self.config);
        encoder
            .transformer
            .unflatten_weights(&params[..self.encoder_weights.len()])?;
        let projectors = self.source_projectors(params)?;
        self.represent_with_encoder(tokens, &mut encoder, &projectors)
    }
    pub(crate) fn represent_with_encoder(
        &self,
        tokens: &[Vec<Vec<f32>>],
        encoder: &mut EmbeddingExtractor,
        projectors: &[crate::embedding::ProjectionHead],
    ) -> Result<(Vec<f32>, Vec<Vec<f32>>), String> {
        let mut features =
            Vec::with_capacity(self.profiles.len() * (self.config.embedding_dim + 2));
        let mut embeddings = Vec::with_capacity(tokens.len());
        for (index, source) in tokens.iter().enumerate() {
            // A frozen Train-only support scale bounds encoder RF values while
            // retaining their signed baseline-relative DC. The task skip below
            // uses the original calibrated values; metadata is never rescaled.
            let mut conditioned = source.clone();
            for row in &mut conditioned {
                for x in &mut row[..self.config.bins] {
                    *x /= self.profiles[index].max_normalized;
                }
            }
            encoder.projection = projectors[index].clone();
            let embedding = encoder.extract(&conditioned);
            features.extend_from_slice(&embedding);
            // Calibrated DC and temporal energy skip preserve static occupancy
            // evidence that per-window centering or L2 projection would erase.
            let mean = source
                .iter()
                .flat_map(|row| row[..self.config.bins].iter())
                .sum::<f32>()
                / (self.config.temporal_tokens * self.config.bins) as f32;
            let temporal = source
                .windows(2)
                .flat_map(|w| {
                    w[0][..self.config.bins]
                        .iter()
                        .zip(&w[1][..self.config.bins])
                })
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>()
                / ((self.config.temporal_tokens - 1) * self.config.bins) as f32;
            features.extend_from_slice(&[mean, temporal]);
            embeddings.push(embedding);
        }
        if !features.iter().all(|x| x.is_finite()) {
            return Err("non-finite representation".into());
        }
        Ok((features, embeddings))
    }
    pub(crate) fn logit(&self, features: &[f32], head: &[f32]) -> f32 {
        features.iter().zip(head).map(|(x, w)| x * w).sum::<f32>() + head[head.len() - 1]
    }
}

pub(crate) fn sigmoid(x: f32) -> f32 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}
