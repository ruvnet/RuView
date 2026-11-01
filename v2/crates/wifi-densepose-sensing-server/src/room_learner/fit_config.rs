//! Bounded fit settings and actual architecture preflight.
use super::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FitConfig {
    pub epochs: usize,
    pub learning_rate: f32,
    pub contrastive_weight: f32,
    pub supervised_weight: f32,
    pub gradient_epsilon: f32,
    pub max_parameters: usize,
    pub max_duration_ms: u64,
    pub max_objective_evaluations: usize,
    pub hidden_dim: usize,
    pub embedding_dim: usize,
    pub temporal_tokens: usize,
    pub temperature: f32,
}
impl Default for FitConfig {
    fn default() -> Self {
        Self {
            epochs: 48,
            learning_rate: 0.2,
            contrastive_weight: 0.1,
            supervised_weight: 1.0,
            gradient_epsilon: 0.002,
            max_parameters: 512,
            max_duration_ms: 60_000,
            max_objective_evaluations: 100_000,
            hidden_dim: 3,
            embedding_dim: 3,
            temporal_tokens: 4,
            temperature: 0.3,
        }
    }
}
impl FitConfig {
    /// Keep every advertised source count within the bounded optimizer budget.
    pub fn for_source_count(source_count: usize) -> Self {
        let mut config = Self::default();
        if source_count >= 7 {
            config.hidden_dim = 2;
            config.embedding_dim = 2;
        }
        config
    }
    pub fn validate_context(&self, context: &CaptureContext) -> Result<usize, String> {
        self.validate()?;
        context.validate()?;
        let config = EncoderConfig {
            source_count: context.sources.len(),
            bins: context
                .sources
                .iter()
                .map(|s| s.feature_count)
                .max()
                .unwrap_or(0),
            hidden_dim: self.hidden_dim,
            embedding_dim: self.embedding_dim,
            temporal_tokens: self.temporal_tokens,
            temperature: self.temperature,
        };
        let count = initial_encoder(&config)?
            .transformer
            .flatten_weights()
            .len()
            + initial_projection_weights(&config)
                .iter()
                .map(Vec::len)
                .sum::<usize>()
            + context.sources.len() * (self.embedding_dim + 2)
            + 1;
        if count > self.max_parameters {
            return Err("model exceeds parameter budget".into());
        }
        Ok(count)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=128).contains(&self.epochs)
            || !(1..=512).contains(&self.max_parameters)
            || !(1..=120_000).contains(&self.max_duration_ms)
            || !(1..=200_000).contains(&self.max_objective_evaluations)
            || !(2..=4).contains(&self.hidden_dim)
            || !(2..=4).contains(&self.embedding_dim)
            || !(2..=16).contains(&self.temporal_tokens)
            || !self.learning_rate.is_finite()
            || !(0.0001..=1.0).contains(&self.learning_rate)
            || !self.contrastive_weight.is_finite()
            || !(0.001..=10.0).contains(&self.contrastive_weight)
            || !self.supervised_weight.is_finite()
            || !(0.001..=10.0).contains(&self.supervised_weight)
            || !self.gradient_epsilon.is_finite()
            || !(0.0001..=0.05).contains(&self.gradient_epsilon)
            || !self.temperature.is_finite()
            || !(0.1..=2.0).contains(&self.temperature)
        {
            return Err("invalid bounded training configuration".into());
        }
        Ok(())
    }
}
