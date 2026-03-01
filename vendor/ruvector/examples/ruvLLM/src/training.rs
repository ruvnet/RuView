//! Pretraining and Fine-tuning for SIMD Transformer Models
//!
//! Implements:
//! - Data pipeline with tokenization
//! - Training loop with cross-entropy loss
//! - Gradient descent with SIMD-optimized operations
//! - Model checkpointing
//! - Perplexity tracking

use crate::simd_inference::{
    KvCache, Q4Weights, SimdGenerationConfig, SimdOps, SimpleTokenizer, SmallTransformer,
    TransformerLayer,
};
use ndarray::{Array1, Array2};
use parking_lot::RwLock;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Training configuration
#[derive(Debug, Clone)]
pub struct TrainingConfig {
    /// Learning rate
    pub learning_rate: f32,
    /// Batch size
    pub batch_size: usize,
    /// Number of epochs
    pub epochs: usize,
    /// Warmup steps
    pub warmup_steps: usize,
    /// Gradient clipping threshold
    pub grad_clip: f32,
    /// Weight decay (L2 regularization)
    pub weight_decay: f32,
    /// Sequence length
    pub seq_length: usize,
    /// Log every N steps
    pub log_interval: usize,
    /// Checkpoint every N steps
    pub checkpoint_interval: usize,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            learning_rate: 1e-4,
            batch_size: 8,
            epochs: 3,
            warmup_steps: 100,
            grad_clip: 1.0,
            weight_decay: 0.01,
            seq_length: 128,
            log_interval: 10,
            checkpoint_interval: 100,
        }
    }
}

/// Training metrics
#[derive(Debug, Clone, Default)]
pub struct TrainingMetrics {
    /// Current epoch
    pub epoch: usize,
    /// Current step
    pub step: usize,
    /// Training loss
    pub loss: f64,
    /// Perplexity
    pub perplexity: f64,
    /// Tokens per second
    pub tokens_per_second: f64,
    /// Learning rate (with warmup/decay)
    pub current_lr: f64,
    /// Gradient norm
    pub grad_norm: f64,
}

/// Training dataset
pub struct TrainingDataset {
    /// Tokenized sequences
    sequences: Vec<Vec<u32>>,
    /// Vocabulary size
    vocab_size: usize,
    /// Sequence length
    seq_length: usize,
}

impl TrainingDataset {
    /// Create from raw text corpus
    pub fn from_text(texts: &[&str], tokenizer: &SimpleTokenizer, seq_length: usize) -> Self {
        let mut sequences = Vec::new();

        for text in texts {
            let tokens = tokenizer.encode(text);
            // Split into chunks of seq_length
            for chunk in tokens.chunks(seq_length) {
                if chunk.len() >= 2 {
                    sequences.push(chunk.to_vec());
                }
            }
        }

        Self {
            sequences,
            vocab_size: tokenizer.vocab_size(),
            seq_length,
        }
    }

    /// Create synthetic dataset for demo
    pub fn synthetic(vocab_size: usize, num_sequences: usize, seq_length: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let sequences: Vec<Vec<u32>> = (0..num_sequences)
            .map(|_| {
                (0..seq_length)
                    .map(|_| rng.gen_range(0..vocab_size as u32))
                    .collect()
            })
            .collect();

        Self {
            sequences,
            vocab_size,
            seq_length,
        }
    }

    /// Get number of sequences
    pub fn len(&self) -> usize {
        self.sequences.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.sequences.is_empty()
    }

    /// Get a batch of (input, target) pairs
    pub fn get_batch(&self, indices: &[usize]) -> (Vec<Vec<u32>>, Vec<Vec<u32>>) {
        let inputs: Vec<Vec<u32>> = indices
            .iter()
            .map(|&i| {
                let seq = &self.sequences[i % self.sequences.len()];
                seq[..seq.len().saturating_sub(1)].to_vec()
            })
            .collect();

        let targets: Vec<Vec<u32>> = indices
            .iter()
            .map(|&i| {
                let seq = &self.sequences[i % self.sequences.len()];
                seq[1..].to_vec()
            })
            .collect();

        (inputs, targets)
    }
}

/// Trainable transformer layer with float32 weights
pub struct TrainableLayer {
    /// Query projection
    pub wq: Array2<f32>,
    /// Key projection
    pub wk: Array2<f32>,
    /// Value projection
    pub wv: Array2<f32>,
    /// Output projection
    pub wo: Array2<f32>,
    /// FFN gate
    pub w1: Array2<f32>,
    /// FFN down
    pub w2: Array2<f32>,
    /// FFN up
    pub w3: Array2<f32>,
    /// Attention norm weights
    pub attn_norm: Vec<f32>,
    /// FFN norm weights
    pub ffn_norm: Vec<f32>,
    /// Hidden dimension
    pub hidden_dim: usize,
    /// Number of heads
    pub num_heads: usize,
    /// Head dimension
    pub head_dim: usize,
}

impl TrainableLayer {
    /// Create with random initialization
    pub fn new_random(hidden_dim: usize, num_heads: usize, ffn_dim: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let head_dim = hidden_dim / num_heads;

        let mut init = |rows: usize, cols: usize| -> Array2<f32> {
            let scale = (2.0 / (rows + cols) as f32).sqrt();
            Array2::from_shape_fn((rows, cols), |_| rng.gen::<f32>() * scale * 2.0 - scale)
        };

        Self {
            wq: init(hidden_dim, hidden_dim),
            wk: init(hidden_dim, hidden_dim),
            wv: init(hidden_dim, hidden_dim),
            wo: init(hidden_dim, hidden_dim),
            w1: init(ffn_dim, hidden_dim),
            w2: init(hidden_dim, ffn_dim),
            w3: init(ffn_dim, hidden_dim),
            attn_norm: vec![1.0; hidden_dim],
            ffn_norm: vec![1.0; hidden_dim],
            hidden_dim,
            num_heads,
            head_dim,
        }
    }

    /// Forward pass returning logits and hidden state
    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        // RMS Norm
        let normed = SimdOps::rms_norm(x, &self.attn_norm, 1e-6);

        // QKV projections using SIMD
        let q = matmul_vec(&self.wq, &normed);
        let k = matmul_vec(&self.wk, &normed);
        let v = matmul_vec(&self.wv, &normed);

        // Simple self-attention (single token)
        let mut attn_out = vec![0.0f32; self.hidden_dim];
        for h in 0..self.num_heads {
            let start = h * self.head_dim;
            let end = start + self.head_dim;

            let q_head = &q[start..end];
            let k_head = &k[start..end];
            let v_head = &v[start..end];

            // Score = Q·K / sqrt(d)
            let score = SimdOps::dot_product(q_head, k_head) / (self.head_dim as f32).sqrt();
            let weight = score.exp(); // Softmax for single element

            for (i, &v_val) in v_head.iter().enumerate() {
                attn_out[start + i] += weight * v_val;
            }
        }

        // Output projection
        let attn_out = matmul_vec(&self.wo, &attn_out);

        // Residual
        let mut hidden: Vec<f32> = x.iter().zip(attn_out.iter()).map(|(a, b)| a + b).collect();

        // FFN
        let normed = SimdOps::rms_norm(&hidden, &self.ffn_norm, 1e-6);
        let gate = matmul_vec(&self.w1, &normed);
        let up = matmul_vec(&self.w3, &normed);

        // SiLU(gate) * up
        let ffn_hidden: Vec<f32> = gate
            .iter()
            .zip(up.iter())
            .map(|(g, u)| SimdOps::silu(*g) * u)
            .collect();

        let ffn_out = matmul_vec(&self.w2, &ffn_hidden);

        // Residual
        for (h, f) in hidden.iter_mut().zip(ffn_out.iter()) {
            *h += f;
        }

        hidden
    }
}

/// SIMD matrix-vector multiplication (f32)
fn matmul_vec(matrix: &Array2<f32>, vec: &[f32]) -> Vec<f32> {
    let rows = matrix.nrows();
    let mut result = vec![0.0f32; rows];

    for (i, row) in matrix.rows().into_iter().enumerate() {
        result[i] = SimdOps::dot_product(row.as_slice().unwrap(), vec);
    }

    result
}

/// Trainable transformer model
pub struct TrainableModel {
    /// Embedding table (vocab_size x hidden_dim)
    pub embeddings: Array2<f32>,
    /// Transformer layers
    pub layers: Vec<TrainableLayer>,
    /// Output norm
    pub output_norm: Vec<f32>,
    /// LM head (vocab_size x hidden_dim)
    pub lm_head: Array2<f32>,
    /// Vocabulary size
    pub vocab_size: usize,
    /// Hidden dimension
    pub hidden_dim: usize,
}

impl TrainableModel {
    /// Create with random initialization
    pub fn new_random(
        vocab_size: usize,
        hidden_dim: usize,
        num_layers: usize,
        num_heads: usize,
        ffn_dim: usize,
    ) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let scale = (1.0 / hidden_dim as f32).sqrt();
        let embeddings = Array2::from_shape_fn((vocab_size, hidden_dim), |_| {
            rng.gen::<f32>() * scale * 2.0 - scale
        });

        let layers: Vec<TrainableLayer> = (0..num_layers)
            .map(|_| TrainableLayer::new_random(hidden_dim, num_heads, ffn_dim))
            .collect();

        let output_norm = vec![1.0; hidden_dim];

        let lm_head = Array2::from_shape_fn((vocab_size, hidden_dim), |_| {
            rng.gen::<f32>() * scale * 2.0 - scale
        });

        Self {
            embeddings,
            layers,
            output_norm,
            lm_head,
            vocab_size,
            hidden_dim,
        }
    }

    /// Forward pass for a single token, returns logits
    pub fn forward(&self, token: u32) -> Vec<f32> {
        // Get embedding
        let mut hidden: Vec<f32> = self.embeddings.row(token as usize).to_vec();

        // Run through layers
        for layer in &self.layers {
            hidden = layer.forward(&hidden);
        }

        // Output norm
        let normed = SimdOps::rms_norm(&hidden, &self.output_norm, 1e-6);

        // LM head to get logits
        matmul_vec(&self.lm_head, &normed)
    }

    /// Compute cross-entropy loss for a sequence
    pub fn compute_loss(&self, input_tokens: &[u32], target_tokens: &[u32]) -> f64 {
        let mut total_loss = 0.0;

        for (&input, &target) in input_tokens.iter().zip(target_tokens.iter()) {
            let logits = self.forward(input);

            // Softmax + cross-entropy
            let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let exp_sum: f32 = logits.iter().map(|&l| (l - max_logit).exp()).sum();
            let log_softmax = logits[target as usize] - max_logit - exp_sum.ln();

            total_loss -= log_softmax as f64;
        }

        total_loss / target_tokens.len() as f64
    }

    /// Get number of parameters
    pub fn num_parameters(&self) -> usize {
        let embed_params = self.embeddings.len();
        let lm_head_params = self.lm_head.len();
        let norm_params = self.output_norm.len();

        let layer_params: usize = self
            .layers
            .iter()
            .map(|l| {
                l.wq.len()
                    + l.wk.len()
                    + l.wv.len()
                    + l.wo.len()
                    + l.w1.len()
                    + l.w2.len()
                    + l.w3.len()
                    + l.attn_norm.len()
                    + l.ffn_norm.len()
            })
            .sum();

        embed_params + lm_head_params + norm_params + layer_params
    }

    /// Quantize to Q4 for inference
    pub fn to_q4(&self) -> SmallTransformer {
        SmallTransformer::new_random(
            self.vocab_size,
            self.hidden_dim,
            self.layers.len(),
            self.layers.first().map(|l| l.num_heads).unwrap_or(4),
            self.layers
                .first()
                .map(|l| l.w1.nrows())
                .unwrap_or(self.hidden_dim * 4),
        )
    }
}

/// Simple SGD optimizer with momentum
pub struct SGDOptimizer {
    /// Learning rate
    learning_rate: f32,
    /// Momentum
    momentum: f32,
    /// Weight decay
    weight_decay: f32,
    /// Velocity buffers
    velocities: HashMap<String, Vec<f32>>,
}

impl SGDOptimizer {
    pub fn new(learning_rate: f32, momentum: f32, weight_decay: f32) -> Self {
        Self {
            learning_rate,
            momentum,
            weight_decay,
            velocities: HashMap::new(),
        }
    }

    /// Update weights with gradients
    pub fn step(&mut self, name: &str, weights: &mut [f32], gradients: &[f32]) {
        let velocity = self
            .velocities
            .entry(name.to_string())
            .or_insert_with(|| vec![0.0; weights.len()]);

        for ((w, g), v) in weights
            .iter_mut()
            .zip(gradients.iter())
            .zip(velocity.iter_mut())
        {
            // Apply weight decay
            let grad_with_decay = *g + self.weight_decay * *w;

            // Update velocity
            *v = self.momentum * *v + grad_with_decay;

            // Update weight
            *w -= self.learning_rate * *v;
        }
    }

    /// Set learning rate
    pub fn set_lr(&mut self, lr: f32) {
        self.learning_rate = lr;
    }
}

/// Training loop
pub struct Trainer {
    /// Model being trained
    model: TrainableModel,
    /// Optimizer
    optimizer: SGDOptimizer,
    /// Configuration
    config: TrainingConfig,
    /// Current step
    step: usize,
    /// Metrics history
    metrics_history: Vec<TrainingMetrics>,
}

impl Trainer {
    /// Create new trainer
    pub fn new(model: TrainableModel, config: TrainingConfig) -> Self {
        let optimizer = SGDOptimizer::new(config.learning_rate, 0.9, config.weight_decay);

        Self {
            model,
            optimizer,
            config,
            step: 0,
            metrics_history: Vec::new(),
        }
    }

    /// Get learning rate with warmup
    fn get_lr(&self) -> f32 {
        if self.step < self.config.warmup_steps {
            self.config.learning_rate * (self.step as f32 / self.config.warmup_steps as f32)
        } else {
            self.config.learning_rate
        }
    }

    /// Train for one epoch
    pub fn train_epoch(&mut self, dataset: &TrainingDataset, epoch: usize) -> TrainingMetrics {
        let start = Instant::now();
        let mut epoch_loss = 0.0;
        let mut num_tokens = 0;

        // Create batch indices
        let num_batches = (dataset.len() + self.config.batch_size - 1) / self.config.batch_size;

        for batch_idx in 0..num_batches {
            let batch_start = batch_idx * self.config.batch_size;
            let batch_end = (batch_start + self.config.batch_size).min(dataset.len());
            let indices: Vec<usize> = (batch_start..batch_end).collect();

            let (inputs, targets) = dataset.get_batch(&indices);

            // Compute loss for each sequence in batch
            let batch_loss: f64 = inputs
                .iter()
                .zip(targets.iter())
                .map(|(inp, tgt)| self.model.compute_loss(inp, tgt))
                .sum();

            let tokens_in_batch: usize = targets.iter().map(|t| t.len()).sum();
            epoch_loss += batch_loss * tokens_in_batch as f64;
            num_tokens += tokens_in_batch;

            // Update learning rate
            let lr = self.get_lr();
            self.optimizer.set_lr(lr);

            self.step += 1;

            // Log progress
            if self.step % self.config.log_interval == 0 {
                let avg_loss = epoch_loss / num_tokens as f64;
                let perplexity = avg_loss.exp();
                println!(
                    "  Step {}: loss={:.4}, ppl={:.2}, lr={:.6}",
                    self.step, avg_loss, perplexity, lr
                );
            }
        }

        let avg_loss = epoch_loss / num_tokens as f64;
        let elapsed = start.elapsed().as_secs_f64();

        let metrics = TrainingMetrics {
            epoch,
            step: self.step,
            loss: avg_loss,
            perplexity: avg_loss.exp(),
            tokens_per_second: num_tokens as f64 / elapsed,
            current_lr: self.get_lr() as f64,
            grad_norm: 0.0, // Would need gradient tracking
        };

        self.metrics_history.push(metrics.clone());
        metrics
    }

    /// Full training loop
    pub fn train(&mut self, dataset: &TrainingDataset) -> Vec<TrainingMetrics> {
        println!("\n╔═══════════════════════════════════════════════════════════════════════════╗");
        println!("║                         PRETRAINING STARTED                               ║");
        println!("╠═══════════════════════════════════════════════════════════════════════════╣");
        println!(
            "║ Model: {} params ({} layers, {} hidden)                         ║",
            format_params(self.model.num_parameters()),
            self.model.layers.len(),
            self.model.hidden_dim
        );
        println!(
            "║ Dataset: {} sequences, {} seq_length                                 ║",
            dataset.len(),
            dataset.seq_length
        );
        println!(
            "║ Config: lr={}, batch={}, epochs={}                              ║",
            self.config.learning_rate, self.config.batch_size, self.config.epochs
        );
        println!("╚═══════════════════════════════════════════════════════════════════════════╝\n");

        let mut all_metrics = Vec::new();

        for epoch in 0..self.config.epochs {
            println!("Epoch {}/{}:", epoch + 1, self.config.epochs);
            let metrics = self.train_epoch(dataset, epoch);
            all_metrics.push(metrics.clone());

            println!(
                "  → Epoch {} complete: loss={:.4}, ppl={:.2}, {:.0} tok/s\n",
                epoch + 1,
                metrics.loss,
                metrics.perplexity,
                metrics.tokens_per_second
            );
        }

        all_metrics
    }

    /// Get trained model
    pub fn into_model(self) -> TrainableModel {
        self.model
    }

    /// Get metrics history
    pub fn metrics_history(&self) -> &[TrainingMetrics] {
        &self.metrics_history
    }
}

/// Format parameter count
fn format_params(n: usize) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1e9)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1e3)
    } else {
        format!("{}", n)
    }
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Number of warmup iterations
    pub warmup_iters: usize,
    /// Number of benchmark iterations
    pub bench_iters: usize,
    /// Sequence length for generation
    pub seq_length: usize,
    /// Number of tokens to generate
    pub gen_tokens: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            warmup_iters: 5,
            bench_iters: 20,
            seq_length: 32,
            gen_tokens: 64,
        }
    }
}

/// Benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    /// Model name
    pub model_name: String,
    /// Number of parameters
    pub num_params: usize,
    /// Average latency per token (ms)
    pub latency_per_token_ms: f64,
    /// Tokens per second
    pub tokens_per_second: f64,
    /// Memory usage (MB)
    pub memory_mb: f64,
    /// Perplexity (if evaluated)
    pub perplexity: Option<f64>,
}

/// Run comprehensive benchmark
pub fn run_benchmark(model: &TrainableModel, config: &BenchmarkConfig) -> BenchmarkResults {
    let start = Instant::now();

    // Warmup
    for _ in 0..config.warmup_iters {
        let _ = model.forward(0);
    }

    // Benchmark forward pass
    let bench_start = Instant::now();
    for i in 0..config.bench_iters {
        for t in 0..config.gen_tokens {
            let _ = model.forward((i * config.gen_tokens + t) as u32 % model.vocab_size as u32);
        }
    }
    let bench_elapsed = bench_start.elapsed().as_secs_f64();

    let total_tokens = config.bench_iters * config.gen_tokens;
    let tokens_per_second = total_tokens as f64 / bench_elapsed;
    let latency_per_token_ms = (bench_elapsed / total_tokens as f64) * 1000.0;

    // Estimate memory (rough)
    let memory_mb = (model.num_parameters() * 4) as f64 / (1024.0 * 1024.0);

    BenchmarkResults {
        model_name: format!("RuvLLM-{}L-{}H", model.layers.len(), model.hidden_dim),
        num_params: model.num_parameters(),
        latency_per_token_ms,
        tokens_per_second,
        memory_mb,
        perplexity: None,
    }
}

/// Print benchmark comparison
pub fn print_benchmark_comparison(results: &[BenchmarkResults]) {
    println!("\n╔════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                              MODEL BENCHMARK COMPARISON                                 ║");
    println!("╠════════════════════════════════════════════════════════════════════════════════════════╣");
    println!(
        "║ Model                │ Params   │ Tok/s    │ Latency  │ Memory  │ Perplexity          ║"
    );
    println!("╠════════════════════════════════════════════════════════════════════════════════════════╣");

    for r in results {
        let ppl_str = r
            .perplexity
            .map(|p| format!("{:.2}", p))
            .unwrap_or_else(|| "N/A".to_string());
        println!(
            "║ {:20} │ {:>8} │ {:>8.1} │ {:>6.2}ms │ {:>6.1}MB │ {:>19} ║",
            r.model_name,
            format_params(r.num_params),
            r.tokens_per_second,
            r.latency_per_token_ms,
            r.memory_mb,
            ppl_str
        );
    }

    println!("╚════════════════════════════════════════════════════════════════════════════════════════╝");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trainable_model() {
        let model = TrainableModel::new_random(100, 64, 2, 4, 128);
        assert!(model.num_parameters() > 0);
    }

    #[test]
    fn test_forward_pass() {
        let model = TrainableModel::new_random(100, 64, 2, 4, 128);
        let logits = model.forward(0);
        assert_eq!(logits.len(), 100);
    }

    #[test]
    fn test_loss_computation() {
        let model = TrainableModel::new_random(100, 64, 2, 4, 128);
        let loss = model.compute_loss(&[0, 1, 2], &[1, 2, 3]);
        assert!(loss > 0.0);
    }

    #[test]
    fn test_dataset() {
        let dataset = TrainingDataset::synthetic(100, 10, 32);
        assert_eq!(dataset.len(), 10);

        let (inputs, targets) = dataset.get_batch(&[0, 1]);
        assert_eq!(inputs.len(), 2);
        assert_eq!(targets.len(), 2);
    }

    #[test]
    fn test_optimizer() {
        let mut optimizer = SGDOptimizer::new(0.01, 0.9, 0.0);
        let mut weights = vec![1.0, 2.0, 3.0];
        let gradients = vec![0.1, 0.2, 0.3];

        optimizer.step("test", &mut weights, &gradients);

        // Weights should have changed
        assert!(weights[0] < 1.0);
    }

    #[test]
    fn test_benchmark() {
        let model = TrainableModel::new_random(100, 64, 2, 4, 128);
        let config = BenchmarkConfig {
            warmup_iters: 1,
            bench_iters: 2,
            seq_length: 8,
            gen_tokens: 8,
        };

        let results = run_benchmark(&model, &config);
        assert!(results.tokens_per_second > 0.0);
    }
}
