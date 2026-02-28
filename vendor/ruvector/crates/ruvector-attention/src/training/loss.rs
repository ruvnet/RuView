//! Loss functions for attention-based learning
//!
//! Includes contrastive losses optimized for representation learning.

/// Reduction method for loss computation
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Reduction {
    #[default]
    Mean,
    Sum,
    None,
}

/// Loss trait for attention training
pub trait Loss: Send + Sync {
    /// Compute loss value
    fn compute(&self, anchor: &[f32], positive: &[f32], negatives: &[&[f32]]) -> f32;

    /// Compute loss with gradients for anchor
    fn compute_with_gradients(
        &self,
        anchor: &[f32],
        positive: &[f32],
        negatives: &[&[f32]],
    ) -> (f32, Vec<f32>);
}

/// InfoNCE contrastive loss
///
/// L = -log(exp(sim(a,p)/τ) / Σexp(sim(a,n)/τ))
pub struct InfoNCELoss {
    temperature: f32,
}

impl InfoNCELoss {
    pub fn new(temperature: f32) -> Self {
        Self {
            temperature: temperature.max(0.01),
        }
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        dot / (norm_a * norm_b)
    }
}

impl Loss for InfoNCELoss {
    fn compute(&self, anchor: &[f32], positive: &[f32], negatives: &[&[f32]]) -> f32 {
        let pos_sim = Self::cosine_similarity(anchor, positive) / self.temperature;

        let neg_sims: Vec<f32> = negatives
            .iter()
            .map(|n| Self::cosine_similarity(anchor, n) / self.temperature)
            .collect();

        // Stable log-sum-exp
        let max_sim = neg_sims
            .iter()
            .copied()
            .chain(std::iter::once(pos_sim))
            .fold(f32::NEG_INFINITY, f32::max);

        let sum_exp: f32 =
            neg_sims.iter().map(|s| (s - max_sim).exp()).sum::<f32>() + (pos_sim - max_sim).exp();

        let log_sum_exp = max_sim + sum_exp.ln();

        log_sum_exp - pos_sim
    }

    fn compute_with_gradients(
        &self,
        anchor: &[f32],
        positive: &[f32],
        negatives: &[&[f32]],
    ) -> (f32, Vec<f32>) {
        let dim = anchor.len();
        let pos_sim = Self::cosine_similarity(anchor, positive) / self.temperature;

        let neg_sims: Vec<f32> = negatives
            .iter()
            .map(|n| Self::cosine_similarity(anchor, n) / self.temperature)
            .collect();

        // Compute softmax weights
        let max_sim = neg_sims
            .iter()
            .copied()
            .chain(std::iter::once(pos_sim))
            .fold(f32::NEG_INFINITY, f32::max);

        let pos_exp = (pos_sim - max_sim).exp();
        let neg_exps: Vec<f32> = neg_sims.iter().map(|s| (s - max_sim).exp()).collect();
        let total_exp: f32 = pos_exp + neg_exps.iter().sum::<f32>();

        let pos_weight = pos_exp / total_exp;
        let neg_weights: Vec<f32> = neg_exps.iter().map(|e| e / total_exp).collect();

        // Loss value
        let loss = -(pos_weight.ln());

        // Gradient with respect to anchor
        // ∂L/∂anchor = (p_pos - 1) * ∂sim(a,p)/∂a + Σ p_neg_i * ∂sim(a,n_i)/∂a
        let norm_a: f32 = anchor.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        let norm_p: f32 = positive.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);

        let mut gradients = vec![0.0f32; dim];

        // Gradient from positive
        let dot_ap: f32 = anchor.iter().zip(positive.iter()).map(|(a, p)| a * p).sum();
        for i in 0..dim {
            let d_sim = (positive[i] / (norm_a * norm_p))
                - (anchor[i] * dot_ap / (norm_a.powi(3) * norm_p));
            gradients[i] += (pos_weight - 1.0) * d_sim / self.temperature;
        }

        // Gradient from negatives
        for (neg, &weight) in negatives.iter().zip(neg_weights.iter()) {
            let norm_n: f32 = neg.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
            let dot_an: f32 = anchor.iter().zip(neg.iter()).map(|(a, n)| a * n).sum();

            for i in 0..dim {
                let d_sim =
                    (neg[i] / (norm_a * norm_n)) - (anchor[i] * dot_an / (norm_a.powi(3) * norm_n));
                gradients[i] += weight * d_sim / self.temperature;
            }
        }

        (loss, gradients)
    }
}

/// Local contrastive loss for neighborhood preservation
pub struct LocalContrastiveLoss {
    margin: f32,
    reduction: Reduction,
}

impl LocalContrastiveLoss {
    pub fn new(margin: f32) -> Self {
        Self {
            margin,
            reduction: Reduction::Mean,
        }
    }

    pub fn with_reduction(mut self, reduction: Reduction) -> Self {
        self.reduction = reduction;
        self
    }

    fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

impl Loss for LocalContrastiveLoss {
    fn compute(&self, anchor: &[f32], positive: &[f32], negatives: &[&[f32]]) -> f32 {
        let d_pos = Self::euclidean_distance(anchor, positive);

        let losses: Vec<f32> = negatives
            .iter()
            .map(|neg| {
                let d_neg = Self::euclidean_distance(anchor, neg);
                (d_pos - d_neg + self.margin).max(0.0)
            })
            .collect();

        match self.reduction {
            Reduction::Mean => losses.iter().sum::<f32>() / losses.len().max(1) as f32,
            Reduction::Sum => losses.iter().sum(),
            Reduction::None => losses.first().copied().unwrap_or(0.0),
        }
    }

    fn compute_with_gradients(
        &self,
        anchor: &[f32],
        positive: &[f32],
        negatives: &[&[f32]],
    ) -> (f32, Vec<f32>) {
        let dim = anchor.len();
        let d_pos = Self::euclidean_distance(anchor, positive);

        let mut total_loss = 0.0f32;
        let mut gradients = vec![0.0f32; dim];
        let mut active_count = 0;

        for neg in negatives.iter() {
            let d_neg = Self::euclidean_distance(anchor, neg);
            let margin_loss = d_pos - d_neg + self.margin;

            if margin_loss > 0.0 {
                total_loss += margin_loss;
                active_count += 1;

                // Gradient: ∂L/∂a = (a - p)/d_pos - (a - n)/d_neg
                for i in 0..dim {
                    if d_pos > 1e-8 {
                        gradients[i] += (anchor[i] - positive[i]) / d_pos;
                    }
                    if d_neg > 1e-8 {
                        gradients[i] -= (anchor[i] - neg[i]) / d_neg;
                    }
                }
            }
        }

        let loss = match self.reduction {
            Reduction::Mean if active_count > 0 => {
                gradients.iter_mut().for_each(|g| *g /= active_count as f32);
                total_loss / active_count as f32
            }
            Reduction::Sum => total_loss,
            _ => total_loss / negatives.len().max(1) as f32,
        };

        (loss, gradients)
    }
}

/// Spectral regularization for smooth representations
pub struct SpectralRegularization {
    weight: f32,
}

impl SpectralRegularization {
    pub fn new(weight: f32) -> Self {
        Self { weight }
    }

    /// Compute spectral norm regularization for a batch of embeddings
    pub fn compute_batch(&self, embeddings: &[&[f32]]) -> f32 {
        if embeddings.is_empty() {
            return 0.0;
        }

        let dim = embeddings[0].len();
        let n = embeddings.len();

        // Compute covariance matrix diagonal approximation
        let mut var_sum = 0.0f32;

        for d in 0..dim {
            let mean: f32 = embeddings.iter().map(|e| e[d]).sum::<f32>() / n as f32;
            let var: f32 = embeddings
                .iter()
                .map(|e| (e[d] - mean).powi(2))
                .sum::<f32>()
                / n as f32;
            var_sum += var;
        }

        // Regularization: encourage uniform variance across dimensions
        let avg_var = var_sum / dim as f32;
        let var_of_var: f32 = {
            let mut sum = 0.0;
            for d in 0..dim {
                let mean: f32 = embeddings.iter().map(|e| e[d]).sum::<f32>() / n as f32;
                let var: f32 = embeddings
                    .iter()
                    .map(|e| (e[d] - mean).powi(2))
                    .sum::<f32>()
                    / n as f32;
                sum += (var - avg_var).powi(2);
            }
            sum / dim as f32
        };

        self.weight * var_of_var
    }
}

impl Loss for SpectralRegularization {
    fn compute(&self, anchor: &[f32], positive: &[f32], negatives: &[&[f32]]) -> f32 {
        let mut all_embeddings: Vec<&[f32]> = Vec::with_capacity(2 + negatives.len());
        all_embeddings.push(anchor);
        all_embeddings.push(positive);
        all_embeddings.extend(negatives.iter().copied());

        self.compute_batch(&all_embeddings)
    }

    fn compute_with_gradients(
        &self,
        anchor: &[f32],
        positive: &[f32],
        negatives: &[&[f32]],
    ) -> (f32, Vec<f32>) {
        let loss = self.compute(anchor, positive, negatives);
        // Simplified: no gradient for spectral reg (typically used as auxiliary)
        let gradients = vec![0.0f32; anchor.len()];
        (loss, gradients)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infonce_loss() {
        let loss = InfoNCELoss::new(0.07);

        let anchor = vec![1.0, 0.0, 0.0];
        let positive = vec![0.9, 0.1, 0.0];
        let negatives: Vec<Vec<f32>> = vec![vec![0.0, 1.0, 0.0], vec![0.0, 0.0, 1.0]];
        let neg_refs: Vec<&[f32]> = negatives.iter().map(|n| n.as_slice()).collect();

        let loss_val = loss.compute(&anchor, &positive, &neg_refs);
        assert!(loss_val >= 0.0);
    }

    #[test]
    fn test_infonce_gradients() {
        let loss = InfoNCELoss::new(0.1);

        let anchor = vec![0.5; 64];
        let positive = vec![0.6; 64];
        let negatives: Vec<Vec<f32>> = vec![vec![0.1; 64]; 5];
        let neg_refs: Vec<&[f32]> = negatives.iter().map(|n| n.as_slice()).collect();

        let (loss_val, grads) = loss.compute_with_gradients(&anchor, &positive, &neg_refs);

        assert!(loss_val >= 0.0);
        assert_eq!(grads.len(), 64);
    }

    #[test]
    fn test_local_contrastive() {
        let loss = LocalContrastiveLoss::new(1.0);

        let anchor = vec![0.0, 0.0];
        let positive = vec![0.1, 0.0]; // Close
        let negatives: Vec<Vec<f32>> = vec![vec![2.0, 0.0], vec![0.0, 2.0]]; // Far
        let neg_refs: Vec<&[f32]> = negatives.iter().map(|n| n.as_slice()).collect();

        let loss_val = loss.compute(&anchor, &positive, &neg_refs);
        assert!(loss_val >= 0.0);
    }

    #[test]
    fn test_spectral_regularization() {
        let reg = SpectralRegularization::new(0.01);

        let embeddings: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32 * 0.1; 32]).collect();
        let emb_refs: Vec<&[f32]> = embeddings.iter().map(|e| e.as_slice()).collect();

        let loss_val = reg.compute_batch(&emb_refs);
        assert!(loss_val >= 0.0);
    }
}
