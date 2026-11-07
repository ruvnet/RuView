//! Existing AETHER projection heads routed by frozen source identity.
//! Opposite RF responses can swap across room states. Source-routed coordinates
//! preserve receiver identity during alignment while the transformer is shared.
use super::{EncoderConfig, RoomModel};
use crate::{
    embedding::{EmbeddingConfig, ProjectionHead},
    graph_transformer::Linear,
};
pub(crate) fn config(c: &EncoderConfig) -> EmbeddingConfig {
    EmbeddingConfig {
        d_model: c.hidden_dim,
        d_proj: c.embedding_dim,
        temperature: c.temperature,
        normalize: false,
    }
}
pub(crate) fn initial_weights(c: &EncoderConfig) -> Vec<Vec<f32>> {
    (0..c.source_count)
        .map(|source| {
            let mut head = ProjectionHead::new(config(c));
            // Source-specific deterministic Xavier seeds break routing symmetry;
            // no label, amplitude polarity or fixture value enters initialization.
            head.proj_1 =
                Linear::with_seed(c.hidden_dim, c.embedding_dim, 20_240 + source as u64 * 2);
            head.proj_1.set_bias(vec![0.1; c.embedding_dim]);
            head.proj_2 =
                Linear::with_seed(c.embedding_dim, c.embedding_dim, 20_241 + source as u64 * 2);
            let mut weights = Vec::new();
            head.flatten_into(&mut weights);
            weights
        })
        .collect()
}
impl RoomModel {
    pub(crate) fn source_projectors(&self, params: &[f32]) -> Result<Vec<ProjectionHead>, String> {
        if params.len() != self.parameter_count() {
            return Err("invalid conditional parameter count".into());
        }
        let mut offset = self.encoder_weights.len();
        let mut heads = Vec::new();
        for weights in &self.source_projection_weights {
            let (head, n) = ProjectionHead::unflatten_from(
                &params[offset..offset + weights.len()],
                &config(&self.config),
            );
            offset += n;
            heads.push(head);
        }
        Ok(heads)
    }
}

/// Center each initial first-layer activation on Train Empty only. This keeps
/// every small ReLU head trainable without assuming any source's signal polarity.
pub(crate) fn center_empty_heads(
    model: &mut RoomModel,
    rows: &[crate::room_learner::observation::CaptureCohort],
) -> Result<(), String> {
    use crate::room_learner::observation::RoomLabel;
    if rows
        .iter()
        .any(|r| r.role != crate::room_learner::observation::DataRole::Train)
    {
        return Err("projection centering only accepts Train".into());
    }
    let mut encoder = super::extractor(&model.config);
    encoder
        .transformer
        .unflatten_weights(&model.encoder_weights)?;
    let mut baseline = vec![vec![0.0; model.config.hidden_dim]; model.config.source_count];
    let mut count = 0usize;
    for row in rows.iter().filter(|r| r.label == RoomLabel::Empty) {
        count += 1;
        for (source, mut tokens) in model.tokens(row, false)?.into_iter().enumerate() {
            for frame in &mut tokens {
                for x in &mut frame[..model.config.bins] {
                    *x /= model.profiles[source].max_normalized;
                }
            }
            let features = encoder.transformer.embed(&tokens);
            for vector in &features {
                for (mean, x) in baseline[source].iter_mut().zip(vector) {
                    *mean += x / features.len() as f32;
                }
            }
        }
    }
    if count == 0 {
        return Err("projection centering requires Train Empty".into());
    }
    for (source, mean) in baseline.iter_mut().enumerate() {
        mean.iter_mut().for_each(|x| *x /= count as f32);
        let (mut head, _) = ProjectionHead::unflatten_from(
            &model.source_projection_weights[source],
            &config(&model.config),
        );
        // Initial bias is 0.1; retain a 0.1 positive activation at the baseline.
        let bias = head
            .proj_1
            .forward(mean)
            .iter()
            .map(|value| 0.2 - value)
            .collect();
        head.proj_1.set_bias(bias);
        let mut weights = Vec::new();
        head.flatten_into(&mut weights);
        model.source_projection_weights[source] = weights;
    }
    Ok(())
}
