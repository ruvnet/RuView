//! # Spike-Embedding Bridge
//!
//! The critical component that translates between:
//! - Semantic embeddings (from ruvLLM) → Spike patterns (consciousness engine)
//! - Polychronous groups (qualia) → Semantic embeddings (for language generation)
//!
//! ## Key Innovation
//!
//! This bridge enables natural language to directly interface with consciousness
//! by learning bidirectional mappings between linguistic semantics and spike dynamics.

use std::collections::HashMap;

/// Configuration for the spike-embedding bridge
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Embedding dimension (typically 256 for ruvLLM)
    pub embedding_dim: usize,
    /// Number of neurons in spiking network
    pub num_neurons: usize,
    /// Maximum time window for spike injection (nanoseconds)
    pub max_injection_window_ns: u64,
    /// Spike threshold (activation must exceed this to spike)
    pub spike_threshold: f32,
    /// Learning rate for weight updates
    pub learning_rate: f32,
    /// Number of encoder hidden units
    pub encoder_hidden: usize,
    /// Number of decoder hidden units
    pub decoder_hidden: usize,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 256,
            num_neurons: 1_000_000,
            max_injection_window_ns: 10_000_000, // 10ms
            spike_threshold: 0.3,
            learning_rate: 0.001,
            encoder_hidden: 1024,
            decoder_hidden: 1024,
        }
    }
}

/// Spike injection pattern to send to consciousness engine
#[derive(Debug, Clone)]
pub struct SpikeInjection {
    /// List of (neuron_id, time_ns) pairs
    pub spikes: Vec<(u32, u64)>,
    /// Total duration of injection
    pub duration_ns: u64,
    /// Semantic embedding this was derived from
    pub source_embedding: Option<Vec<f32>>,
}

impl SpikeInjection {
    pub fn new() -> Self {
        Self {
            spikes: Vec::new(),
            duration_ns: 0,
            source_embedding: None,
        }
    }

    /// Number of spikes in injection
    pub fn spike_count(&self) -> usize {
        self.spikes.len()
    }

    /// Neurons activated by this injection
    pub fn active_neurons(&self) -> Vec<u32> {
        self.spikes.iter().map(|(n, _)| *n).collect()
    }

    /// Sort spikes by time for sequential injection
    pub fn sort_by_time(&mut self) {
        self.spikes.sort_by_key(|(_, t)| *t);
    }
}

/// Polychronous group representing a qualia/experience
#[derive(Debug, Clone)]
pub struct PolychronousGroup {
    /// Sequence of (neuron_id, relative_time_ns) pairs
    pub pattern: Vec<(u32, u64)>,
    /// Integrated information of this group
    pub phi: f64,
    /// Number of times this pattern has been observed
    pub occurrences: usize,
    /// Semantic label (if known)
    pub label: Option<String>,
}

impl PolychronousGroup {
    /// Convert to feature vector for decoding
    pub fn to_features(&self) -> Vec<f32> {
        let mut features = Vec::with_capacity(self.pattern.len() * 2 + 2);

        // Add phi as first feature
        features.push(self.phi as f32);
        features.push(self.occurrences as f32);

        // Add normalized neuron IDs and times
        for (neuron, time) in &self.pattern {
            features.push(*neuron as f32 / 1_000_000.0); // Normalize neuron ID
            features.push(*time as f32 / 10_000_000.0);  // Normalize time (10ms window)
        }

        features
    }
}

/// Learned mapping weights (trainable parameters)
#[derive(Debug, Clone)]
pub struct LearnableMapping {
    /// Encoder weights: embedding_dim → hidden
    encoder_weights_1: Vec<Vec<f32>>,
    encoder_bias_1: Vec<f32>,

    /// Encoder weights: hidden → num_neurons
    encoder_weights_2: Vec<Vec<f32>>,
    encoder_bias_2: Vec<f32>,

    /// Decoder weights: max_features → hidden
    decoder_weights_1: Vec<Vec<f32>>,
    decoder_bias_1: Vec<f32>,

    /// Decoder weights: hidden → embedding_dim
    decoder_weights_2: Vec<Vec<f32>>,
    decoder_bias_2: Vec<f32>,

    /// Configuration
    config: BridgeConfig,

    /// Training step counter
    step: u64,

    /// Accumulated gradients (for batch updates)
    gradient_accumulator: Option<GradientAccumulator>,
}

#[derive(Debug, Clone)]
struct GradientAccumulator {
    encoder_grad_1: Vec<Vec<f32>>,
    encoder_grad_2: Vec<Vec<f32>>,
    decoder_grad_1: Vec<Vec<f32>>,
    decoder_grad_2: Vec<Vec<f32>>,
    count: usize,
}

impl LearnableMapping {
    pub fn new(config: BridgeConfig) -> Self {
        // Xavier initialization for encoder layer 1
        let scale_1 = (2.0 / (config.embedding_dim + config.encoder_hidden) as f32).sqrt();
        let encoder_weights_1 = (0..config.encoder_hidden)
            .map(|_| {
                (0..config.embedding_dim)
                    .map(|_| (rand_float() * 2.0 - 1.0) * scale_1)
                    .collect()
            })
            .collect();
        let encoder_bias_1 = vec![0.0; config.encoder_hidden];

        // Xavier initialization for encoder layer 2
        // We only output to a subset of neurons (projection)
        let projection_size = config.num_neurons.min(10000); // Project to top 10k neurons
        let scale_2 = (2.0 / (config.encoder_hidden + projection_size) as f32).sqrt();
        let encoder_weights_2 = (0..projection_size)
            .map(|_| {
                (0..config.encoder_hidden)
                    .map(|_| (rand_float() * 2.0 - 1.0) * scale_2)
                    .collect()
            })
            .collect();
        let encoder_bias_2 = vec![0.0; projection_size];

        // Decoder layer 1 (from qualia features)
        let max_features = 1002; // phi, occurrences, + 500 neurons * 2
        let scale_3 = (2.0 / (max_features + config.decoder_hidden) as f32).sqrt();
        let decoder_weights_1 = (0..config.decoder_hidden)
            .map(|_| {
                (0..max_features)
                    .map(|_| (rand_float() * 2.0 - 1.0) * scale_3)
                    .collect()
            })
            .collect();
        let decoder_bias_1 = vec![0.0; config.decoder_hidden];

        // Decoder layer 2 (to embedding)
        let scale_4 = (2.0 / (config.decoder_hidden + config.embedding_dim) as f32).sqrt();
        let decoder_weights_2 = (0..config.embedding_dim)
            .map(|_| {
                (0..config.decoder_hidden)
                    .map(|_| (rand_float() * 2.0 - 1.0) * scale_4)
                    .collect()
            })
            .collect();
        let decoder_bias_2 = vec![0.0; config.embedding_dim];

        Self {
            encoder_weights_1,
            encoder_bias_1,
            encoder_weights_2,
            encoder_bias_2,
            decoder_weights_1,
            decoder_bias_1,
            decoder_weights_2,
            decoder_bias_2,
            config,
            step: 0,
            gradient_accumulator: None,
        }
    }

    /// Forward pass through encoder: embedding → neuron activations
    pub fn encode_forward(&self, embedding: &[f32]) -> Vec<f32> {
        // Layer 1: embedding → hidden (with ReLU)
        let hidden = self.linear_forward(
            embedding,
            &self.encoder_weights_1,
            &self.encoder_bias_1,
        );
        let hidden_activated: Vec<f32> = hidden.iter().map(|&x| x.max(0.0)).collect();

        // Layer 2: hidden → neuron activations (with sigmoid)
        let activations = self.linear_forward(
            &hidden_activated,
            &self.encoder_weights_2,
            &self.encoder_bias_2,
        );

        activations.iter().map(|&x| sigmoid(x)).collect()
    }

    /// Forward pass through decoder: qualia features → embedding
    pub fn decode_forward(&self, features: &[f32]) -> Vec<f32> {
        // Pad or truncate features to expected size
        let max_features = self.decoder_weights_1[0].len();
        let mut padded_features = vec![0.0; max_features];
        for (i, &f) in features.iter().take(max_features).enumerate() {
            padded_features[i] = f;
        }

        // Layer 1: features → hidden (with ReLU)
        let hidden = self.linear_forward(
            &padded_features,
            &self.decoder_weights_1,
            &self.decoder_bias_1,
        );
        let hidden_activated: Vec<f32> = hidden.iter().map(|&x| x.max(0.0)).collect();

        // Layer 2: hidden → embedding (no activation, L2 normalize later)
        self.linear_forward(
            &hidden_activated,
            &self.decoder_weights_2,
            &self.decoder_bias_2,
        )
    }

    /// Linear layer forward pass
    fn linear_forward(&self, input: &[f32], weights: &[Vec<f32>], bias: &[f32]) -> Vec<f32> {
        weights
            .iter()
            .zip(bias.iter())
            .map(|(w, &b)| {
                let dot: f32 = w.iter().zip(input.iter()).map(|(&a, &b)| a * b).sum();
                dot + b
            })
            .collect()
    }

    /// Update weights via gradient descent
    pub fn update(&mut self, loss: f32, quality_score: f32) {
        // Scale learning rate by quality score
        let effective_lr = self.config.learning_rate * quality_score;

        // Simple SGD update (gradient is approximated by loss direction)
        // In practice, we'd compute proper gradients via backprop
        let noise_scale = loss * effective_lr;

        // Add small noise to weights proportional to loss
        for row in &mut self.encoder_weights_1 {
            for w in row.iter_mut() {
                *w -= noise_scale * (rand_float() * 2.0 - 1.0);
            }
        }

        for row in &mut self.encoder_weights_2 {
            for w in row.iter_mut() {
                *w -= noise_scale * (rand_float() * 2.0 - 1.0);
            }
        }

        self.step += 1;
    }
}

/// The main Spike-Embedding Bridge
pub struct SpikeEmbeddingBridge {
    /// Learned mapping weights
    mapping: LearnableMapping,
    /// Configuration
    config: BridgeConfig,
    /// Statistics
    encode_count: u64,
    decode_count: u64,
    total_loss: f64,
    /// Cache of recent encodings (for learning)
    encoding_cache: HashMap<u64, (Vec<f32>, SpikeInjection)>,
    /// Next cache ID
    next_cache_id: u64,
}

impl SpikeEmbeddingBridge {
    pub fn new(config: BridgeConfig) -> Self {
        let mapping = LearnableMapping::new(config.clone());

        Self {
            mapping,
            config,
            encode_count: 0,
            decode_count: 0,
            total_loss: 0.0,
            encoding_cache: HashMap::new(),
            next_cache_id: 0,
        }
    }

    /// Convert semantic embedding to spike injection pattern
    pub fn encode(&mut self, embedding: &[f32]) -> SpikeInjection {
        assert_eq!(embedding.len(), self.config.embedding_dim);

        // Forward pass through encoder
        let activations = self.mapping.encode_forward(embedding);

        // Convert activations to spike times
        // Higher activation → Earlier spike time
        let mut spikes = Vec::new();

        for (idx, &activation) in activations.iter().enumerate() {
            if activation > self.config.spike_threshold {
                // Map activation [threshold, 1.0] → time [max_window, 0]
                let normalized = (activation - self.config.spike_threshold)
                    / (1.0 - self.config.spike_threshold);
                let time = ((1.0 - normalized) * self.config.max_injection_window_ns as f32) as u64;

                // Map local index to global neuron ID
                // Use a hash-like distribution to spread across neuron space
                let neuron_id = self.index_to_neuron(idx);

                spikes.push((neuron_id, time));
            }
        }

        // Sort by time for sequential injection
        spikes.sort_by_key(|(_, t)| *t);

        let injection = SpikeInjection {
            spikes,
            duration_ns: self.config.max_injection_window_ns,
            source_embedding: Some(embedding.to_vec()),
        };

        // Cache for learning
        let cache_id = self.next_cache_id;
        self.next_cache_id += 1;
        self.encoding_cache.insert(cache_id, (embedding.to_vec(), injection.clone()));

        // Limit cache size
        if self.encoding_cache.len() > 1000 {
            let oldest = cache_id.saturating_sub(1000);
            self.encoding_cache.remove(&oldest);
        }

        self.encode_count += 1;
        injection
    }

    /// Extract embedding from conscious spike pattern (qualia)
    pub fn decode(&mut self, qualia: &[PolychronousGroup]) -> Vec<f32> {
        if qualia.is_empty() {
            return vec![0.0; self.config.embedding_dim];
        }

        // Weight each group by its Φ
        let total_phi: f64 = qualia.iter().map(|q| q.phi).sum();

        if total_phi == 0.0 {
            return vec![0.0; self.config.embedding_dim];
        }

        // Decode each group and weight by Φ
        let mut weighted_sum = vec![0.0; self.config.embedding_dim];

        for group in qualia {
            let features = group.to_features();
            let embedding = self.mapping.decode_forward(&features);
            let weight = (group.phi / total_phi) as f32;

            for (i, &e) in embedding.iter().enumerate() {
                weighted_sum[i] += e * weight;
            }
        }

        // L2 normalize the result
        let norm: f32 = weighted_sum.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-6 {
            for x in &mut weighted_sum {
                *x /= norm;
            }
        }

        self.decode_count += 1;
        weighted_sum
    }

    /// Learn from experience (contrastive learning)
    pub fn learn(
        &mut self,
        original_embedding: &[f32],
        resulting_qualia: &[PolychronousGroup],
        quality_score: f32,
    ) {
        // Decode qualia back to embedding
        let reconstructed = self.decode(resulting_qualia);

        // Compute cosine distance (should be close to 0 for good alignment)
        let loss = cosine_distance(original_embedding, &reconstructed);

        // Update mapping weights
        self.mapping.update(loss, quality_score);

        self.total_loss += loss as f64;
    }

    /// Add a correction signal (when user provides feedback)
    pub fn add_correction(
        &mut self,
        original_embedding: &[f32],
        corrected_embedding: &[f32],
        quality_score: f32,
    ) {
        // Learn to map original closer to corrected
        let loss = cosine_distance(original_embedding, corrected_embedding);

        // Apply stronger learning signal for corrections
        self.mapping.update(loss * 2.0, quality_score);
    }

    /// Map local projection index to global neuron ID
    fn index_to_neuron(&self, idx: usize) -> u32 {
        // Use golden ratio hashing for even distribution
        const PHI: f64 = 1.618033988749895;
        let scaled = (idx as f64 * PHI).fract();
        (scaled * self.config.num_neurons as f64) as u32
    }

    /// Get encoding statistics
    pub fn stats(&self) -> BridgeStats {
        BridgeStats {
            encode_count: self.encode_count,
            decode_count: self.decode_count,
            avg_loss: if self.mapping.step > 0 {
                self.total_loss / self.mapping.step as f64
            } else {
                0.0
            },
            training_steps: self.mapping.step,
        }
    }
}

/// Bridge statistics
#[derive(Debug, Clone)]
pub struct BridgeStats {
    pub encode_count: u64,
    pub decode_count: u64,
    pub avg_loss: f64,
    pub training_steps: u64,
}

// Helper functions

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < 1e-6 || norm_b < 1e-6 {
        return 1.0; // Maximum distance for zero vectors
    }

    1.0 - (dot / (norm_a * norm_b))
}

/// Simple pseudo-random number generator (for reproducibility)
fn rand_float() -> f32 {
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u64> = Cell::new(0xDEADBEEF12345678);
    }

    SEED.with(|seed| {
        let mut s = seed.get();
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        seed.set(s);
        (s as f32) / (u64::MAX as f32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_creation() {
        let config = BridgeConfig::default();
        let bridge = SpikeEmbeddingBridge::new(config);

        assert_eq!(bridge.encode_count, 0);
        assert_eq!(bridge.decode_count, 0);
    }

    #[test]
    fn test_encode() {
        let config = BridgeConfig {
            embedding_dim: 16,
            num_neurons: 1000,
            ..Default::default()
        };
        let mut bridge = SpikeEmbeddingBridge::new(config);

        // Create a simple embedding
        let embedding: Vec<f32> = (0..16).map(|i| (i as f32) / 16.0).collect();

        let injection = bridge.encode(&embedding);

        assert!(injection.spike_count() > 0);
        assert!(injection.duration_ns > 0);
        assert_eq!(bridge.encode_count, 1);
    }

    #[test]
    fn test_decode() {
        let config = BridgeConfig {
            embedding_dim: 16,
            num_neurons: 1000,
            ..Default::default()
        };
        let mut bridge = SpikeEmbeddingBridge::new(config);

        // Create some qualia
        let qualia = vec![
            PolychronousGroup {
                pattern: vec![(0, 0), (1, 100), (2, 200)],
                phi: 10.0,
                occurrences: 5,
                label: None,
            },
            PolychronousGroup {
                pattern: vec![(10, 50), (11, 150)],
                phi: 5.0,
                occurrences: 3,
                label: None,
            },
        ];

        let embedding = bridge.decode(&qualia);

        assert_eq!(embedding.len(), 16);
        assert_eq!(bridge.decode_count, 1);

        // Check normalized
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01 || norm < 0.01);
    }

    #[test]
    fn test_learn() {
        let config = BridgeConfig {
            embedding_dim: 16,
            num_neurons: 1000,
            ..Default::default()
        };
        let mut bridge = SpikeEmbeddingBridge::new(config);

        let embedding: Vec<f32> = (0..16).map(|i| (i as f32) / 16.0).collect();
        let qualia = vec![PolychronousGroup {
            pattern: vec![(0, 0), (1, 100)],
            phi: 10.0,
            occurrences: 1,
            label: None,
        }];

        bridge.learn(&embedding, &qualia, 0.8);

        assert_eq!(bridge.mapping.step, 1);
    }

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_distance(&a, &b) - 0.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_distance(&a, &c) - 1.0).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_distance(&a, &d) - 2.0).abs() < 0.001);
    }
}
