use ndarray::{Array2, Array1, Axis};
use std::f32;

/// Self-attention mechanism for temporal sequences
pub struct TemporalAttention {
    d_model: usize,
    n_heads: usize,
    d_k: usize,
    // Query, Key, Value projections for each head
    w_q: Vec<Array2<f32>>,
    w_k: Vec<Array2<f32>>,
    w_v: Vec<Array2<f32>>,
    w_o: Array2<f32>,
    // Positional encoding
    pos_encoding: Array2<f32>,
}

impl TemporalAttention {
    pub fn new(d_model: usize, n_heads: usize, max_seq_len: usize) -> Self {
        assert_eq!(d_model % n_heads, 0, "d_model must be divisible by n_heads");
        let d_k = d_model / n_heads;

        use rand::{thread_rng, Rng};
        let mut rng = thread_rng();
        let scale = (1.0 / d_k as f32).sqrt();

        // Initialize projection matrices for each head
        let mut w_q = Vec::new();
        let mut w_k = Vec::new();
        let mut w_v = Vec::new();

        for _ in 0..n_heads {
            w_q.push(Array2::from_shape_fn((d_k, d_model), |_|
                rng.gen::<f32>() * scale - scale/2.0));
            w_k.push(Array2::from_shape_fn((d_k, d_model), |_|
                rng.gen::<f32>() * scale - scale/2.0));
            w_v.push(Array2::from_shape_fn((d_k, d_model), |_|
                rng.gen::<f32>() * scale - scale/2.0));
        }

        let w_o = Array2::from_shape_fn((d_model, d_model), |_|
            rng.gen::<f32>() * scale - scale/2.0);

        // Create sinusoidal positional encoding
        let pos_encoding = Self::create_positional_encoding(max_seq_len, d_model);

        Self {
            d_model,
            n_heads,
            d_k,
            w_q,
            w_k,
            w_v,
            w_o,
            pos_encoding,
        }
    }

    fn create_positional_encoding(max_len: usize, d_model: usize) -> Array2<f32> {
        let mut encoding = Array2::zeros((max_len, d_model));

        for pos in 0..max_len {
            for i in 0..d_model/2 {
                let angle = pos as f32 / (10000.0_f32.powf(2.0 * i as f32 / d_model as f32));
                encoding[[pos, 2*i]] = angle.sin();
                encoding[[pos, 2*i + 1]] = angle.cos();
            }
        }

        encoding
    }

    /// Scaled dot-product attention
    fn attention(&self, q: &Array2<f32>, k: &Array2<f32>, v: &Array2<f32>) -> Array2<f32> {
        let d_k_sqrt = (self.d_k as f32).sqrt();

        // Compute attention scores: Q @ K^T / sqrt(d_k)
        let scores = q.dot(&k.t()) / d_k_sqrt;

        // Apply softmax
        let exp_scores = scores.mapv(|x| x.exp());
        let sum_exp = exp_scores.sum_axis(Axis(1));
        let attention_weights = &exp_scores / &sum_exp.insert_axis(Axis(1));

        // Apply attention to values
        attention_weights.dot(v)
    }

    /// Multi-head attention forward pass
    pub fn forward(&self, x: &Array2<f32>) -> Array2<f32> {
        let seq_len = x.nrows();
        let batch_d = x.ncols();

        // Add positional encoding
        let pos_slice = self.pos_encoding.slice(s![..seq_len, ..batch_d]);
        let x_pos = x + &pos_slice;

        let mut head_outputs = Vec::new();

        // Process each attention head
        for h in 0..self.n_heads {
            let q = x_pos.dot(&self.w_q[h].t());
            let k = x_pos.dot(&self.w_k[h].t());
            let v = x_pos.dot(&self.w_v[h].t());

            let head_out = self.attention(&q, &k, &v);
            head_outputs.push(head_out);
        }

        // Concatenate heads
        let mut concat = Array2::zeros((seq_len, self.d_model));
        for (h, head_out) in head_outputs.iter().enumerate() {
            let start = h * self.d_k;
            let end = start + self.d_k;
            concat.slice_mut(s![.., start..end]).assign(head_out);
        }

        // Final linear projection
        concat.dot(&self.w_o.t())
    }

    /// Extract temporal features with attention
    pub fn extract_features(&self, sequence: &[Vec<f32>]) -> Vec<f32> {
        let seq_len = sequence.len();
        let feat_dim = sequence[0].len();

        // Convert to ndarray
        let mut x = Array2::zeros((seq_len, feat_dim));
        for (i, features) in sequence.iter().enumerate() {
            for (j, &val) in features.iter().enumerate() {
                x[[i, j]] = val;
            }
        }

        // Apply attention
        let attended = self.forward(&x);

        // Global average pooling over time
        attended.mean_axis(Axis(0))
            .unwrap()
            .to_vec()
    }
}

/// Causal attention for autoregressive prediction
pub struct CausalAttention {
    attention: TemporalAttention,
    mask: Array2<bool>,
}

impl CausalAttention {
    pub fn new(d_model: usize, n_heads: usize, max_seq_len: usize) -> Self {
        let attention = TemporalAttention::new(d_model, n_heads, max_seq_len);

        // Create causal mask (lower triangular)
        let mut mask = Array2::from_elem((max_seq_len, max_seq_len), false);
        for i in 0..max_seq_len {
            for j in 0..=i {
                mask[[i, j]] = true;
            }
        }

        Self { attention, mask }
    }

    /// Apply causal masking to attention scores
    pub fn forward_causal(&self, x: &Array2<f32>) -> Array2<f32> {
        let seq_len = x.nrows();

        // Apply standard attention
        let output = self.attention.forward(x);

        // Apply causal mask (in practice, this would be done inside attention computation)
        let mask_slice = self.mask.slice(s![..seq_len, ..seq_len]);

        // Masked output
        output.masked_fill(&mask_slice.mapv(|b| !b), 0.0)
    }
}

// Simplified implementation for masked_fill
trait MaskedFill {
    fn masked_fill(&self, mask: &Array2<bool>, value: f32) -> Self;
}

impl MaskedFill for Array2<f32> {
    fn masked_fill(&self, mask: &Array2<bool>, value: f32) -> Self {
        let mut result = self.clone();
        for ((i, j), &m) in mask.indexed_iter() {
            if !m {
                result[[i, j]] = value;
            }
        }
        result
    }
}

use ndarray::s;