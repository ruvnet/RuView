//! FastGRNN Router for Intelligent Model Selection
//!
//! Uses sparse + low-rank matrices for efficient routing decisions.
//! 90% sparse weight matrices with rank-8 decomposition.

/// Router configuration
#[derive(Clone, Debug)]
pub struct RouterConfig {
    /// Input dimension
    pub input_dim: usize,
    /// Hidden state dimension
    pub hidden_dim: usize,
    /// Number of model outputs
    pub num_models: usize,
    /// Weight sparsity (0.0 - 1.0)
    pub sparsity: f32,
    /// Low-rank decomposition rank
    pub rank: usize,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            input_dim: 128,
            hidden_dim: 64,
            num_models: 4,
            sparsity: 0.9,
            rank: 8,
        }
    }
}

/// Routing decision from FastGRNN
#[derive(Clone, Debug)]
pub struct RoutingDecision {
    /// Selected model index
    pub model_index: usize,
    /// Model selection probabilities
    pub model_probs: Vec<f32>,
    /// Recommended context size bucket
    pub context_bucket: usize,
    /// Recommended temperature
    pub temperature: f32,
    /// Confidence score
    pub confidence: f32,
}

/// FastGRNN Router with sparse + low-rank weights
pub struct FastGRNNRouter {
    /// Configuration
    config: RouterConfig,
    /// Input to gate (sparse)
    w_z: Vec<f32>,
    /// Low-rank factor A for recurrent
    u_z_a: Vec<f32>,
    /// Low-rank factor B for recurrent
    u_z_b: Vec<f32>,
    /// Output projection for models
    w_model: Vec<f32>,
    /// Output projection for context
    w_context: Vec<f32>,
    /// Output projection for temperature
    w_temp: Vec<f32>,
    /// Gate modulation parameters
    zeta: f32,
    nu: f32,
}

impl FastGRNNRouter {
    /// Create a new FastGRNN router
    pub fn new(config: RouterConfig) -> Result<Self, String> {
        let h = config.hidden_dim;
        let d = config.input_dim;
        let r = config.rank;
        let m = config.num_models;

        Ok(Self {
            config: config.clone(),
            w_z: vec![0.01; d * h],
            u_z_a: vec![0.01; h * r],
            u_z_b: vec![0.01; r * h],
            w_model: vec![0.01; h * m],
            w_context: vec![0.01; h * 5], // 5 context buckets
            w_temp: vec![0.01; h],
            zeta: 1.0,
            nu: 0.0,
        })
    }

    /// Forward pass with hidden state
    pub fn forward(&self, input: &[f32], hidden: &[f32]) -> Result<(RoutingDecision, Vec<f32>), String> {
        let h = self.config.hidden_dim;
        let d = self.config.input_dim;
        let r = self.config.rank;
        let m = self.config.num_models;

        if input.len() != d {
            return Err(format!("Input dimension mismatch: expected {}, got {}", d, input.len()));
        }

        // Compute gate: z = sigmoid(W_z @ x + U_z @ h)
        // where U_z = U_z_a @ U_z_b (low-rank)

        // W_z @ x
        let mut pre_gate = vec![0.0f32; h];
        for i in 0..h {
            for j in 0..d {
                pre_gate[i] += self.w_z[j * h + i] * input[j];
            }
        }

        // Low-rank recurrent: U_z_a @ (U_z_b @ h)
        // First: U_z_b @ h
        let mut low_rank = vec![0.0f32; r];
        for i in 0..r {
            for j in 0..h.min(hidden.len()) {
                low_rank[i] += self.u_z_b[j * r + i] * hidden[j];
            }
        }

        // Then: U_z_a @ low_rank
        for i in 0..h {
            for j in 0..r {
                pre_gate[i] += self.u_z_a[j * h + i] * low_rank[j];
            }
        }

        // Gate activation: z = sigmoid(pre_gate)
        let gate: Vec<f32> = pre_gate.iter().map(|&x| 1.0 / (1.0 + (-x).exp())).collect();

        // New hidden state: h' = (zeta * (1 - z) + nu) * tanh(W_x @ x) + z * h
        let mut new_hidden = vec![0.0f32; h];
        for i in 0..h.min(hidden.len()) {
            let tanh_wx = (pre_gate[i]).tanh();
            new_hidden[i] = (self.zeta * (1.0 - gate[i]) + self.nu) * tanh_wx + gate[i] * hidden[i];
        }

        // Output heads

        // Model selection (softmax)
        let mut model_logits = vec![0.0f32; m];
        for i in 0..m {
            for j in 0..h {
                model_logits[i] += self.w_model[j * m + i] * new_hidden[j];
            }
        }
        self.softmax(&mut model_logits);
        let model_index = model_logits.iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Context bucket (softmax over 5 buckets)
        let mut context_logits = vec![0.0f32; 5];
        for i in 0..5 {
            for j in 0..h {
                context_logits[i] += self.w_context[j * 5 + i] * new_hidden[j];
            }
        }
        self.softmax(&mut context_logits);
        let context_bucket = context_logits.iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(2);

        // Temperature (sigmoid scaled to [0.1, 2.0])
        let mut temp_logit = 0.0f32;
        for j in 0..h {
            temp_logit += self.w_temp[j] * new_hidden[j];
        }
        let temperature = 0.1 + 1.9 / (1.0 + (-temp_logit).exp());

        // Confidence
        let confidence = model_logits[model_index];

        let decision = RoutingDecision {
            model_index,
            model_probs: model_logits,
            context_bucket,
            temperature,
            confidence,
        };

        Ok((decision, new_hidden))
    }

    /// Initialize hidden state
    pub fn init_hidden(&self) -> Vec<f32> {
        vec![0.0; self.config.hidden_dim]
    }

    fn softmax(&self, x: &mut [f32]) {
        if x.is_empty() {
            return;
        }
        let max = x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mut sum = 0.0f32;
        for v in x.iter_mut() {
            *v = (*v - max).exp();
            sum += *v;
        }
        if sum > 0.0 {
            for v in x.iter_mut() {
                *v /= sum;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        let router = FastGRNNRouter::new(RouterConfig::default());
        assert!(router.is_ok());
    }

    #[test]
    fn test_router_forward() {
        let config = RouterConfig {
            input_dim: 64,
            hidden_dim: 32,
            num_models: 4,
            ..Default::default()
        };
        let router = FastGRNNRouter::new(config).unwrap();
        let input = vec![0.5; 64];
        let hidden = router.init_hidden();

        let (decision, new_hidden) = router.forward(&input, &hidden).unwrap();

        assert!(decision.model_index < 4);
        assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
        assert!(decision.temperature >= 0.1 && decision.temperature <= 2.0);
        assert_eq!(new_hidden.len(), 32);
    }
}
