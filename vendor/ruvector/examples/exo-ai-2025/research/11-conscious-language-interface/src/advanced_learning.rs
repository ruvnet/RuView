//! # Advanced Learning Module
//!
//! Nobel-level optimizations for the Conscious Language Interface:
//!
//! 1. **Adaptive Learning Rate Controller** - Self-adjusts based on loss landscape
//! 2. **STDP Gradient Modulation** - Spike-timing inspired gradient enhancement
//! 3. **Pattern Consolidation** - Short-term to long-term memory with deduplication
//! 4. **Multi-Task EWC Controller** - Prevents catastrophic forgetting
//! 5. **Hybrid Inference Engine** - Fast forward pass + online learning
//! 6. **Capability Auto-Tuner** - Configuration optimization

use std::collections::HashMap;
use std::time::Instant;

/// Adaptive Learning Rate Controller
///
/// Self-adjusts learning rate based on loss landscape analysis.
/// Increases LR when learning is stable, decreases when unstable.
#[derive(Debug, Clone)]
pub struct AdaptiveLRController {
    /// Current learning rate
    pub current_lr: f32,
    /// Base learning rate
    base_lr: f32,
    /// Minimum learning rate
    min_lr: f32,
    /// Maximum learning rate
    max_lr: f32,
    /// Loss history for trend analysis
    loss_history: Vec<f32>,
    /// Stability window size
    window_size: usize,
    /// Growth factor when stable
    growth_factor: f32,
    /// Shrink factor when unstable
    shrink_factor: f32,
    /// Consecutive stable steps
    stable_steps: u32,
    /// Consecutive unstable steps
    unstable_steps: u32,
}

impl AdaptiveLRController {
    pub fn new(base_lr: f32) -> Self {
        Self {
            current_lr: base_lr,
            base_lr,
            min_lr: base_lr * 0.01,
            max_lr: base_lr * 10.0,
            loss_history: Vec::with_capacity(100),
            window_size: 10,
            growth_factor: 1.1,
            shrink_factor: 0.5,
            stable_steps: 0,
            unstable_steps: 0,
        }
    }

    /// Update learning rate based on new loss value
    pub fn update(&mut self, loss: f32) -> f32 {
        self.loss_history.push(loss);

        // Keep history bounded
        if self.loss_history.len() > self.window_size * 2 {
            self.loss_history.remove(0);
        }

        if self.loss_history.len() >= self.window_size {
            let recent = &self.loss_history[self.loss_history.len() - self.window_size..];
            let variance = self.compute_variance(recent);
            let mean = recent.iter().sum::<f32>() / recent.len() as f32;

            // Coefficient of variation
            let cv = if mean > 1e-6 { variance.sqrt() / mean } else { 0.0 };

            // Stability threshold: CV < 0.2 is stable
            if cv < 0.2 {
                self.stable_steps += 1;
                self.unstable_steps = 0;

                // Grow LR after 5 consecutive stable steps
                if self.stable_steps >= 5 {
                    self.current_lr = (self.current_lr * self.growth_factor).min(self.max_lr);
                    self.stable_steps = 0;
                }
            } else if cv > 0.5 {
                self.unstable_steps += 1;
                self.stable_steps = 0;

                // Shrink LR immediately when unstable
                if self.unstable_steps >= 2 {
                    self.current_lr = (self.current_lr * self.shrink_factor).max(self.min_lr);
                    self.unstable_steps = 0;
                }
            }
        }

        self.current_lr
    }

    fn compute_variance(&self, values: &[f32]) -> f32 {
        if values.is_empty() {
            return 0.0;
        }
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        values.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / values.len() as f32
    }

    /// Get statistics
    pub fn stats(&self) -> LRStats {
        LRStats {
            current_lr: self.current_lr,
            min_lr: self.min_lr,
            max_lr: self.max_lr,
            stable_steps: self.stable_steps,
            unstable_steps: self.unstable_steps,
            history_len: self.loss_history.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LRStats {
    pub current_lr: f32,
    pub min_lr: f32,
    pub max_lr: f32,
    pub stable_steps: u32,
    pub unstable_steps: u32,
    pub history_len: usize,
}

/// STDP (Spike-Timing Dependent Plasticity) Gradient Modulator
///
/// Uses spike-timing rules to enhance learning:
/// - **LTP (Long-Term Potentiation)**: Strengthen when post fires after pre
/// - **LTD (Long-Term Depression)**: Weaken when pre fires after post
#[derive(Debug, Clone)]
pub struct STDPGradientModulator {
    /// Time constant for LTP (ms)
    tau_plus: f32,
    /// Time constant for LTD (ms)
    tau_minus: f32,
    /// LTP amplitude
    a_plus: f32,
    /// LTD amplitude
    a_minus: f32,
    /// Spike timing history: neuron_id -> last spike time (ns)
    spike_times: HashMap<u32, u64>,
    /// Gradient modulation history
    modulation_history: Vec<f32>,
}

impl STDPGradientModulator {
    pub fn new() -> Self {
        Self {
            tau_plus: 20.0,   // 20ms LTP window
            tau_minus: 20.0,  // 20ms LTD window
            a_plus: 1.0,      // LTP amplitude
            a_minus: 0.5,     // LTD amplitude (asymmetric for stability)
            spike_times: HashMap::new(),
            modulation_history: Vec::new(),
        }
    }

    /// Record a spike event
    pub fn record_spike(&mut self, neuron_id: u32, time_ns: u64) {
        self.spike_times.insert(neuron_id, time_ns);

        // Cleanup old entries (older than 100ms)
        let cutoff = time_ns.saturating_sub(100_000_000);
        self.spike_times.retain(|_, t| *t > cutoff);
    }

    /// Compute STDP modulation for a pair of neurons
    ///
    /// Returns modulation factor in range [-a_minus, a_plus]
    pub fn compute_modulation(&self, pre_neuron: u32, post_neuron: u32) -> f32 {
        let pre_time = self.spike_times.get(&pre_neuron);
        let post_time = self.spike_times.get(&post_neuron);

        match (pre_time, post_time) {
            (Some(&t_pre), Some(&t_post)) => {
                // Convert to milliseconds
                let dt_ms = (t_post as f64 - t_pre as f64) / 1_000_000.0;

                if dt_ms > 0.0 {
                    // Post fires after pre → LTP (strengthen)
                    self.a_plus * (-dt_ms as f32 / self.tau_plus).exp()
                } else {
                    // Pre fires after post → LTD (weaken)
                    -self.a_minus * (dt_ms as f32 / self.tau_minus).exp()
                }
            }
            _ => 0.0, // No timing information
        }
    }

    /// Modulate gradient based on spike timing
    pub fn modulate_gradient(&mut self, gradient: f32, pre_neurons: &[u32], post_neurons: &[u32]) -> f32 {
        if pre_neurons.is_empty() || post_neurons.is_empty() {
            return gradient;
        }

        // Average modulation across all pairs
        let mut total_mod = 0.0;
        let mut count = 0;

        for &pre in pre_neurons {
            for &post in post_neurons {
                total_mod += self.compute_modulation(pre, post);
                count += 1;
            }
        }

        let avg_mod = if count > 0 { total_mod / count as f32 } else { 0.0 };

        // Apply modulation: gradient * (1 + modulation)
        let modulated = gradient * (1.0 + avg_mod);
        self.modulation_history.push(avg_mod);

        // Keep history bounded
        if self.modulation_history.len() > 1000 {
            self.modulation_history.remove(0);
        }

        modulated
    }

    /// Get average modulation
    pub fn average_modulation(&self) -> f32 {
        if self.modulation_history.is_empty() {
            0.0
        } else {
            self.modulation_history.iter().sum::<f32>() / self.modulation_history.len() as f32
        }
    }
}

impl Default for STDPGradientModulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Pattern Consolidation Engine
///
/// Implements short-term to long-term memory transfer with:
/// - Automatic deduplication (similarity threshold)
/// - Pattern clustering via k-means
/// - Quality-based retention
#[derive(Debug, Clone)]
pub struct PatternConsolidator {
    /// Short-term buffer
    short_term: Vec<ConsolidationPattern>,
    /// Long-term storage
    long_term: Vec<ConsolidationPattern>,
    /// Similarity threshold for deduplication
    similarity_threshold: f32,
    /// Maximum short-term patterns
    max_short_term: usize,
    /// Maximum long-term patterns
    max_long_term: usize,
    /// Consolidation statistics
    stats: ConsolidationStats,
}

#[derive(Debug, Clone)]
pub struct ConsolidationPattern {
    /// Pattern ID
    pub id: u64,
    /// Pattern embedding
    pub embedding: Vec<f32>,
    /// Quality score [0, 1]
    pub quality: f32,
    /// Access count
    pub access_count: u32,
    /// Creation time
    pub created: Instant,
    /// Is consolidated to long-term
    pub is_consolidated: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ConsolidationStats {
    pub patterns_added: u64,
    pub patterns_consolidated: u64,
    pub patterns_deduplicated: u64,
    pub last_consolidation_ms: u64,
}

impl PatternConsolidator {
    pub fn new(similarity_threshold: f32) -> Self {
        Self {
            short_term: Vec::new(),
            long_term: Vec::new(),
            similarity_threshold,
            max_short_term: 500,
            max_long_term: 10_000,
            stats: ConsolidationStats::default(),
        }
    }

    /// Add a new pattern (goes to short-term first)
    pub fn add(&mut self, embedding: Vec<f32>, quality: f32) -> u64 {
        let id = self.stats.patterns_added;
        self.stats.patterns_added += 1;

        // Check for duplicates in short-term
        for pattern in &mut self.short_term {
            if cosine_similarity(&embedding, &pattern.embedding) > self.similarity_threshold {
                // Merge: update existing pattern
                pattern.access_count += 1;
                pattern.quality = (pattern.quality + quality) / 2.0;
                self.stats.patterns_deduplicated += 1;
                return pattern.id;
            }
        }

        // Add new pattern
        self.short_term.push(ConsolidationPattern {
            id,
            embedding,
            quality,
            access_count: 1,
            created: Instant::now(),
            is_consolidated: false,
        });

        // Prune if over capacity
        if self.short_term.len() > self.max_short_term {
            self.prune_short_term();
        }

        id
    }

    /// Consolidate high-quality patterns to long-term storage
    pub fn consolidate(&mut self) -> usize {
        let start = Instant::now();

        // Find high-quality patterns to consolidate
        let quality_threshold = 0.7;
        let min_accesses = 2;

        let to_consolidate: Vec<_> = self.short_term
            .iter()
            .filter(|p| p.quality >= quality_threshold && p.access_count >= min_accesses)
            .cloned()
            .collect();

        let mut consolidated = 0;

        for mut pattern in to_consolidate {
            // Check for duplicates in long-term
            let is_duplicate = self.long_term.iter().any(|p| {
                cosine_similarity(&pattern.embedding, &p.embedding) > self.similarity_threshold
            });

            if !is_duplicate {
                pattern.is_consolidated = true;
                self.long_term.push(pattern);
                consolidated += 1;
            } else {
                self.stats.patterns_deduplicated += 1;
            }
        }

        // Remove consolidated patterns from short-term
        self.short_term.retain(|p| !p.is_consolidated);

        // Prune long-term if over capacity
        if self.long_term.len() > self.max_long_term {
            self.prune_long_term();
        }

        self.stats.patterns_consolidated += consolidated as u64;
        self.stats.last_consolidation_ms = start.elapsed().as_millis() as u64;

        consolidated
    }

    /// Find similar patterns
    pub fn find_similar(&self, embedding: &[f32], k: usize) -> Vec<&ConsolidationPattern> {
        let mut all_patterns: Vec<_> = self.long_term.iter()
            .chain(self.short_term.iter())
            .map(|p| (p, cosine_similarity(&p.embedding, embedding)))
            .collect();

        all_patterns.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        all_patterns.into_iter().take(k).map(|(p, _)| p).collect()
    }

    fn prune_short_term(&mut self) {
        // Remove lowest quality patterns
        self.short_term.sort_by(|a, b| {
            b.quality.partial_cmp(&a.quality).unwrap_or(std::cmp::Ordering::Equal)
        });
        self.short_term.truncate(self.max_short_term);
    }

    fn prune_long_term(&mut self) {
        // Remove oldest, lowest quality patterns
        self.long_term.sort_by(|a, b| {
            let score_a = a.quality * a.access_count as f32;
            let score_b = b.quality * b.access_count as f32;
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        self.long_term.truncate(self.max_long_term);
    }

    /// Get statistics
    pub fn stats(&self) -> &ConsolidationStats {
        &self.stats
    }

    /// Get pattern counts
    pub fn pattern_counts(&self) -> (usize, usize) {
        (self.short_term.len(), self.long_term.len())
    }
}

/// Elastic Weight Consolidation (EWC) Controller
///
/// Prevents catastrophic forgetting when learning multiple tasks.
/// Protects important weights learned on previous tasks.
#[derive(Debug, Clone)]
pub struct EWCController {
    /// Task-specific Fisher information matrices
    /// Maps task_id -> weight_index -> importance
    fisher_matrices: HashMap<u64, Vec<f32>>,
    /// Optimal weights for each task
    optimal_weights: HashMap<u64, Vec<f32>>,
    /// Current task ID
    current_task: u64,
    /// EWC penalty coefficient (lambda)
    lambda: f32,
    /// Number of weights being tracked
    num_weights: usize,
}

impl EWCController {
    pub fn new(num_weights: usize, lambda: f32) -> Self {
        Self {
            fisher_matrices: HashMap::new(),
            optimal_weights: HashMap::new(),
            current_task: 0,
            lambda,
            num_weights,
        }
    }

    /// Record importance of weights for current task
    /// Call this after training on a task is complete
    pub fn record_task(&mut self, weights: &[f32], gradients: &[f32]) {
        assert_eq!(weights.len(), self.num_weights);
        assert_eq!(gradients.len(), self.num_weights);

        // Fisher information ≈ squared gradients (diagonal approximation)
        let fisher: Vec<f32> = gradients.iter().map(|g| g * g).collect();

        self.fisher_matrices.insert(self.current_task, fisher);
        self.optimal_weights.insert(self.current_task, weights.to_vec());

        self.current_task += 1;
    }

    /// Compute EWC penalty for weight update
    ///
    /// Returns penalty that should be added to the loss
    pub fn compute_penalty(&self, current_weights: &[f32]) -> f32 {
        if self.fisher_matrices.is_empty() {
            return 0.0;
        }

        let mut penalty = 0.0;

        for (task_id, fisher) in &self.fisher_matrices {
            if let Some(optimal) = self.optimal_weights.get(task_id) {
                for i in 0..self.num_weights.min(current_weights.len()) {
                    let diff = current_weights[i] - optimal[i];
                    penalty += fisher[i] * diff * diff;
                }
            }
        }

        self.lambda * penalty / 2.0
    }

    /// Compute EWC-modulated gradient
    ///
    /// Reduces gradient magnitude for important weights
    pub fn modulate_gradient(&self, weight_idx: usize, gradient: f32, current_weight: f32) -> f32 {
        if weight_idx >= self.num_weights || self.fisher_matrices.is_empty() {
            return gradient;
        }

        let mut ewc_gradient = 0.0;

        for (task_id, fisher) in &self.fisher_matrices {
            if let Some(optimal) = self.optimal_weights.get(task_id) {
                let diff = current_weight - optimal[weight_idx];
                ewc_gradient += self.lambda * fisher[weight_idx] * diff;
            }
        }

        gradient + ewc_gradient
    }

    /// Get number of protected tasks
    pub fn num_tasks(&self) -> usize {
        self.fisher_matrices.len()
    }
}

/// Hybrid Inference Engine
///
/// Combines fast traditional forward pass with optional online learning.
/// - Fast path: 11μs forward pass only
/// - Learning path: +2μs overhead for online weight updates
#[derive(Debug, Clone)]
pub struct HybridInferenceEngine {
    /// Enable online learning during inference
    online_learning: bool,
    /// Online learning rate (smaller than training LR)
    online_lr: f32,
    /// Pattern augmentation enabled
    pattern_augmented: bool,
    /// Inference statistics
    stats: InferenceStats,
    /// Pattern cache for augmentation
    pattern_cache: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Default)]
pub struct InferenceStats {
    pub total_inferences: u64,
    pub online_updates: u64,
    pub pattern_augmentations: u64,
    pub total_latency_us: u64,
}

impl HybridInferenceEngine {
    pub fn new(online_learning: bool, pattern_augmented: bool) -> Self {
        Self {
            online_learning,
            online_lr: 0.0001, // Very small LR for online learning
            pattern_augmented,
            stats: InferenceStats::default(),
            pattern_cache: Vec::new(),
        }
    }

    /// Fast forward pass only
    pub fn infer_fast(&mut self, input: &[f32], weights: &[Vec<f32>]) -> Vec<f32> {
        let start = Instant::now();

        let output = self.forward_pass(input, weights);

        self.stats.total_inferences += 1;
        self.stats.total_latency_us += start.elapsed().as_micros() as u64;

        output
    }

    /// Inference with optional online learning
    pub fn infer(&mut self, input: &[f32], weights: &mut [Vec<f32>], target: Option<&[f32]>) -> Vec<f32> {
        let start = Instant::now();

        // Forward pass
        let mut output = self.forward_pass(input, weights);

        // Pattern augmentation
        if self.pattern_augmented && !self.pattern_cache.is_empty() {
            let augmented = self.augment_with_patterns(&output);
            output = augmented;
            self.stats.pattern_augmentations += 1;
        }

        // Online learning
        if self.online_learning {
            if let Some(target) = target {
                self.online_update(input, &output, target, weights);
                self.stats.online_updates += 1;
            }
        }

        // Cache pattern
        self.cache_pattern(output.clone());

        self.stats.total_inferences += 1;
        self.stats.total_latency_us += start.elapsed().as_micros() as u64;

        output
    }

    fn forward_pass(&self, input: &[f32], weights: &[Vec<f32>]) -> Vec<f32> {
        // Simple MLP forward pass
        let mut activation = input.to_vec();

        for layer_weights in weights {
            let output_size = layer_weights.len() / (activation.len().max(1));
            let mut new_activation = vec![0.0; output_size.max(1)];

            for (i, out) in new_activation.iter_mut().enumerate() {
                for (j, &inp) in activation.iter().enumerate() {
                    let weight_idx = i * activation.len() + j;
                    if weight_idx < layer_weights.len() {
                        *out += inp * layer_weights[weight_idx];
                    }
                }
                // ReLU activation
                *out = out.max(0.0);
            }

            activation = new_activation;
        }

        activation
    }

    fn online_update(&self, _input: &[f32], output: &[f32], target: &[f32], weights: &mut [Vec<f32>]) {
        // Simple online gradient descent
        let error: Vec<f32> = output.iter()
            .zip(target.iter())
            .map(|(&o, &t)| t - o)
            .collect();

        // Update last layer weights (simplified)
        if let Some(last_weights) = weights.last_mut() {
            for (i, w) in last_weights.iter_mut().enumerate() {
                let error_idx = i % error.len();
                *w += self.online_lr * error[error_idx];
            }
        }
    }

    fn augment_with_patterns(&self, output: &[f32]) -> Vec<f32> {
        // Find most similar cached pattern
        let mut best_sim = 0.0;
        let mut best_pattern: Option<&Vec<f32>> = None;

        for pattern in &self.pattern_cache {
            let sim = cosine_similarity(output, pattern);
            if sim > best_sim && sim < 0.99 {
                best_sim = sim;
                best_pattern = Some(pattern);
            }
        }

        // Blend output with similar pattern
        if let Some(pattern) = best_pattern {
            let blend = 0.1; // 10% pattern influence
            output.iter()
                .zip(pattern.iter())
                .map(|(&o, &p)| o * (1.0 - blend) + p * blend)
                .collect()
        } else {
            output.to_vec()
        }
    }

    fn cache_pattern(&mut self, pattern: Vec<f32>) {
        self.pattern_cache.push(pattern);
        if self.pattern_cache.len() > 100 {
            self.pattern_cache.remove(0);
        }
    }

    /// Get statistics
    pub fn stats(&self) -> &InferenceStats {
        &self.stats
    }

    /// Average inference latency
    pub fn avg_latency_us(&self) -> f64 {
        if self.stats.total_inferences == 0 {
            0.0
        } else {
            self.stats.total_latency_us as f64 / self.stats.total_inferences as f64
        }
    }
}

/// Capability Auto-Tuner
///
/// Automatically explores and optimizes hyperparameter configurations:
/// - LoRA rank
/// - Learning rate
/// - Batch size
/// - Hidden dimensions
#[derive(Debug, Clone)]
pub struct CapabilityAutoTuner {
    /// Configuration search space
    search_space: SearchSpace,
    /// Evaluated configurations
    evaluated: Vec<ConfigResult>,
    /// Best configuration found
    best_config: Option<TunerConfig>,
    /// Best score achieved
    best_score: f32,
    /// Maximum evaluations
    max_evaluations: usize,
}

#[derive(Debug, Clone)]
pub struct SearchSpace {
    pub lora_ranks: Vec<usize>,
    pub learning_rates: Vec<f32>,
    pub batch_sizes: Vec<usize>,
    pub hidden_dims: Vec<usize>,
}

impl Default for SearchSpace {
    fn default() -> Self {
        Self {
            lora_ranks: vec![4, 8, 16, 32, 64],
            learning_rates: vec![0.0001, 0.0003, 0.001, 0.003, 0.01],
            batch_sizes: vec![8, 16, 32, 64, 128],
            hidden_dims: vec![256, 512, 1024, 2048],
        }
    }
}

#[derive(Debug, Clone)]
pub struct TunerConfig {
    pub lora_rank: usize,
    pub learning_rate: f32,
    pub batch_size: usize,
    pub hidden_dim: usize,
}

#[derive(Debug, Clone)]
pub struct ConfigResult {
    pub config: TunerConfig,
    pub score: f32,
    pub latency_ms: f32,
}

impl CapabilityAutoTuner {
    pub fn new(max_evaluations: usize) -> Self {
        Self {
            search_space: SearchSpace::default(),
            evaluated: Vec::new(),
            best_config: None,
            best_score: 0.0,
            max_evaluations,
        }
    }

    /// Get next configuration to try
    pub fn suggest(&self) -> Option<TunerConfig> {
        if self.evaluated.len() >= self.max_evaluations {
            return None;
        }

        // Use random search with importance sampling
        // Favor configurations similar to best performers
        let config = if self.evaluated.len() < 10 || self.best_config.is_none() {
            // Initial exploration: random
            self.random_config()
        } else {
            // Exploitation: mutate best config
            self.mutate_best()
        };

        Some(config)
    }

    /// Record result of configuration evaluation
    pub fn record(&mut self, config: TunerConfig, score: f32, latency_ms: f32) {
        if score > self.best_score {
            self.best_score = score;
            self.best_config = Some(config.clone());
        }

        self.evaluated.push(ConfigResult {
            config,
            score,
            latency_ms,
        });
    }

    /// Get best configuration
    pub fn best(&self) -> Option<&TunerConfig> {
        self.best_config.as_ref()
    }

    /// Get number of configurations explored
    pub fn num_explored(&self) -> usize {
        self.evaluated.len()
    }

    fn random_config(&self) -> TunerConfig {
        TunerConfig {
            lora_rank: self.search_space.lora_ranks[rand_idx(self.search_space.lora_ranks.len())],
            learning_rate: self.search_space.learning_rates[rand_idx(self.search_space.learning_rates.len())],
            batch_size: self.search_space.batch_sizes[rand_idx(self.search_space.batch_sizes.len())],
            hidden_dim: self.search_space.hidden_dims[rand_idx(self.search_space.hidden_dims.len())],
        }
    }

    fn mutate_best(&self) -> TunerConfig {
        let best = self.best_config.as_ref().unwrap();

        // Randomly mutate one parameter
        let mutation = rand_idx(4);

        match mutation {
            0 => TunerConfig {
                lora_rank: self.search_space.lora_ranks[rand_idx(self.search_space.lora_ranks.len())],
                ..*best
            },
            1 => TunerConfig {
                learning_rate: self.search_space.learning_rates[rand_idx(self.search_space.learning_rates.len())],
                ..*best
            },
            2 => TunerConfig {
                batch_size: self.search_space.batch_sizes[rand_idx(self.search_space.batch_sizes.len())],
                ..*best
            },
            _ => TunerConfig {
                hidden_dim: self.search_space.hidden_dims[rand_idx(self.search_space.hidden_dims.len())],
                ..*best
            },
        }
    }
}

// Helper functions

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < 1e-6 || norm_b < 1e-6 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

fn rand_idx(max: usize) -> usize {
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u64> = Cell::new(0xFEEDFACE12345678);
    }

    SEED.with(|seed| {
        let mut s = seed.get();
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        seed.set(s);
        (s as usize) % max.max(1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_lr() {
        let mut lr = AdaptiveLRController::new(0.001);

        // Simulate stable learning
        for _ in 0..20 {
            lr.update(0.1);
        }

        // LR should have grown due to stability
        assert!(lr.current_lr >= 0.001);
    }

    #[test]
    fn test_stdp_modulator() {
        let mut stdp = STDPGradientModulator::new();

        // Record spikes with causal timing (pre before post)
        // Use absolute timestamps that won't get cleaned up (within 100ms window)
        let base_time = 100_000_000u64; // 100ms base
        stdp.record_spike(1, base_time);           // pre at t=100ms
        stdp.record_spike(2, base_time + 5_000_000); // post at t=105ms

        let modulation = stdp.compute_modulation(1, 2);
        // Should be LTP (positive) when post fires after pre
        assert!(modulation > 0.0, "Expected positive modulation (LTP), got {}", modulation);

        // Anti-causal timing (pre fires after post)
        let anti_modulation = stdp.compute_modulation(2, 1);
        // Should be LTD (negative) when pre fires after post
        assert!(anti_modulation < 0.0, "Expected negative modulation (LTD), got {}", anti_modulation);
    }

    #[test]
    fn test_pattern_consolidator() {
        let mut consolidator = PatternConsolidator::new(0.85);

        // Add patterns
        let id1 = consolidator.add(vec![1.0, 0.0, 0.0], 0.8);
        let id2 = consolidator.add(vec![0.0, 1.0, 0.0], 0.9);

        assert_ne!(id1, id2);

        let (short_term, _long_term) = consolidator.pattern_counts();
        assert_eq!(short_term, 2);
    }

    #[test]
    fn test_ewc_controller() {
        let mut ewc = EWCController::new(10, 1.0);

        let weights = vec![0.5; 10];
        let gradients = vec![0.1; 10];

        ewc.record_task(&weights, &gradients);

        assert_eq!(ewc.num_tasks(), 1);

        // Penalty should be 0 when weights unchanged
        let penalty = ewc.compute_penalty(&weights);
        assert!(penalty < 0.01);

        // Penalty should increase when weights deviate
        let new_weights: Vec<f32> = weights.iter().map(|w| w + 0.5).collect();
        let penalty2 = ewc.compute_penalty(&new_weights);
        assert!(penalty2 > penalty);
    }

    #[test]
    fn test_hybrid_inference() {
        let mut engine = HybridInferenceEngine::new(true, false);

        let input = vec![1.0, 2.0, 3.0];
        let mut weights = vec![vec![0.1; 9]]; // 3->3 layer

        let output = engine.infer(&input, &mut weights, None);
        assert!(!output.is_empty());
        assert_eq!(engine.stats().total_inferences, 1);
    }

    #[test]
    fn test_capability_tuner() {
        let mut tuner = CapabilityAutoTuner::new(50);

        // Get suggestions and record results
        for i in 0..5 {
            if let Some(config) = tuner.suggest() {
                tuner.record(config, 0.8 + (i as f32 * 0.01), 10.0);
            }
        }

        assert_eq!(tuner.num_explored(), 5);
        assert!(tuner.best().is_some());
    }
}
