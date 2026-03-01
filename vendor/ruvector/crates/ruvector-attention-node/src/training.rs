//! NAPI-RS bindings for training utilities
//!
//! Provides Node.js bindings for:
//! - Loss functions (InfoNCE, LocalContrastive, SpectralRegularization)
//! - Optimizers (SGD, Adam, AdamW)
//! - Learning rate schedulers
//! - Curriculum learning
//! - Negative mining

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_attention::training::{
    Adam as RustAdam, AdamW as RustAdamW, CurriculumScheduler as RustCurriculum,
    CurriculumStage as RustStage, DecayType as RustDecayType, HardNegativeMiner as RustHardMiner,
    InfoNCELoss as RustInfoNCE, LocalContrastiveLoss as RustLocalContrastive, Loss,
    MiningStrategy as RustMiningStrategy, NegativeMiner, Optimizer,
    SpectralRegularization as RustSpectralReg, TemperatureAnnealing as RustTempAnnealing,
    SGD as RustSGD,
};

// ============================================================================
// Loss Functions
// ============================================================================

/// InfoNCE contrastive loss for representation learning
#[napi]
pub struct InfoNCELoss {
    inner: RustInfoNCE,
    temperature_value: f32,
}

#[napi]
impl InfoNCELoss {
    /// Create a new InfoNCE loss instance
    ///
    /// # Arguments
    /// * `temperature` - Temperature parameter for softmax (typically 0.07-0.1)
    #[napi(constructor)]
    pub fn new(temperature: f64) -> Self {
        Self {
            inner: RustInfoNCE::new(temperature as f32),
            temperature_value: temperature as f32,
        }
    }

    /// Compute InfoNCE loss
    ///
    /// # Arguments
    /// * `anchor` - Anchor embedding
    /// * `positive` - Positive example embedding
    /// * `negatives` - Array of negative example embeddings
    #[napi]
    pub fn compute(
        &self,
        anchor: Float32Array,
        positive: Float32Array,
        negatives: Vec<Float32Array>,
    ) -> f64 {
        let anchor_slice = anchor.as_ref();
        let positive_slice = positive.as_ref();
        let negatives_vec: Vec<Vec<f32>> = negatives.into_iter().map(|n| n.to_vec()).collect();
        let negatives_refs: Vec<&[f32]> = negatives_vec.iter().map(|n| n.as_slice()).collect();

        self.inner
            .compute(anchor_slice, positive_slice, &negatives_refs) as f64
    }

    /// Compute InfoNCE loss with gradients
    ///
    /// Returns an object with `loss` and `gradients` fields
    #[napi]
    pub fn compute_with_gradients(
        &self,
        anchor: Float32Array,
        positive: Float32Array,
        negatives: Vec<Float32Array>,
    ) -> LossWithGradients {
        let anchor_slice = anchor.as_ref();
        let positive_slice = positive.as_ref();
        let negatives_vec: Vec<Vec<f32>> = negatives.into_iter().map(|n| n.to_vec()).collect();
        let negatives_refs: Vec<&[f32]> = negatives_vec.iter().map(|n| n.as_slice()).collect();

        let (loss, gradients) =
            self.inner
                .compute_with_gradients(anchor_slice, positive_slice, &negatives_refs);

        LossWithGradients {
            loss: loss as f64,
            gradients: Float32Array::new(gradients),
        }
    }

    /// Get the temperature
    #[napi(getter)]
    pub fn temperature(&self) -> f64 {
        self.temperature_value as f64
    }
}

/// Loss computation result with gradients
#[napi(object)]
pub struct LossWithGradients {
    pub loss: f64,
    pub gradients: Float32Array,
}

/// Local contrastive loss for neighborhood preservation
#[napi]
pub struct LocalContrastiveLoss {
    inner: RustLocalContrastive,
    margin_value: f32,
}

#[napi]
impl LocalContrastiveLoss {
    /// Create a new local contrastive loss instance
    ///
    /// # Arguments
    /// * `margin` - Margin for triplet loss
    #[napi(constructor)]
    pub fn new(margin: f64) -> Self {
        Self {
            inner: RustLocalContrastive::new(margin as f32),
            margin_value: margin as f32,
        }
    }

    /// Compute local contrastive loss
    #[napi]
    pub fn compute(
        &self,
        anchor: Float32Array,
        positive: Float32Array,
        negatives: Vec<Float32Array>,
    ) -> f64 {
        let anchor_slice = anchor.as_ref();
        let positive_slice = positive.as_ref();
        let negatives_vec: Vec<Vec<f32>> = negatives.into_iter().map(|n| n.to_vec()).collect();
        let negatives_refs: Vec<&[f32]> = negatives_vec.iter().map(|n| n.as_slice()).collect();

        self.inner
            .compute(anchor_slice, positive_slice, &negatives_refs) as f64
    }

    /// Compute with gradients
    #[napi]
    pub fn compute_with_gradients(
        &self,
        anchor: Float32Array,
        positive: Float32Array,
        negatives: Vec<Float32Array>,
    ) -> LossWithGradients {
        let anchor_slice = anchor.as_ref();
        let positive_slice = positive.as_ref();
        let negatives_vec: Vec<Vec<f32>> = negatives.into_iter().map(|n| n.to_vec()).collect();
        let negatives_refs: Vec<&[f32]> = negatives_vec.iter().map(|n| n.as_slice()).collect();

        let (loss, gradients) =
            self.inner
                .compute_with_gradients(anchor_slice, positive_slice, &negatives_refs);

        LossWithGradients {
            loss: loss as f64,
            gradients: Float32Array::new(gradients),
        }
    }

    /// Get the margin
    #[napi(getter)]
    pub fn margin(&self) -> f64 {
        self.margin_value as f64
    }
}

/// Spectral regularization for smooth representations
#[napi]
pub struct SpectralRegularization {
    inner: RustSpectralReg,
    weight_value: f32,
}

#[napi]
impl SpectralRegularization {
    /// Create a new spectral regularization instance
    ///
    /// # Arguments
    /// * `weight` - Regularization weight
    #[napi(constructor)]
    pub fn new(weight: f64) -> Self {
        Self {
            inner: RustSpectralReg::new(weight as f32),
            weight_value: weight as f32,
        }
    }

    /// Compute spectral regularization for a batch of embeddings
    #[napi]
    pub fn compute_batch(&self, embeddings: Vec<Float32Array>) -> f64 {
        let embeddings_vec: Vec<Vec<f32>> = embeddings.into_iter().map(|e| e.to_vec()).collect();
        let embeddings_refs: Vec<&[f32]> = embeddings_vec.iter().map(|e| e.as_slice()).collect();

        self.inner.compute_batch(&embeddings_refs) as f64
    }

    /// Get the weight
    #[napi(getter)]
    pub fn weight(&self) -> f64 {
        self.weight_value as f64
    }
}

// ============================================================================
// Optimizers
// ============================================================================

/// SGD optimizer with optional momentum and weight decay
#[napi]
pub struct SGDOptimizer {
    inner: RustSGD,
}

#[napi]
impl SGDOptimizer {
    /// Create a new SGD optimizer
    ///
    /// # Arguments
    /// * `param_count` - Number of parameters
    /// * `learning_rate` - Learning rate
    #[napi(constructor)]
    pub fn new(param_count: u32, learning_rate: f64) -> Self {
        Self {
            inner: RustSGD::new(param_count as usize, learning_rate as f32),
        }
    }

    /// Create with momentum
    #[napi(factory)]
    pub fn with_momentum(param_count: u32, learning_rate: f64, momentum: f64) -> Self {
        Self {
            inner: RustSGD::new(param_count as usize, learning_rate as f32)
                .with_momentum(momentum as f32),
        }
    }

    /// Create with momentum and weight decay
    #[napi(factory)]
    pub fn with_weight_decay(
        param_count: u32,
        learning_rate: f64,
        momentum: f64,
        weight_decay: f64,
    ) -> Self {
        Self {
            inner: RustSGD::new(param_count as usize, learning_rate as f32)
                .with_momentum(momentum as f32)
                .with_weight_decay(weight_decay as f32),
        }
    }

    /// Perform an optimization step
    ///
    /// # Arguments
    /// * `params` - Parameter array
    /// * `gradients` - Gradient array
    ///
    /// # Returns
    /// Updated parameter array
    #[napi]
    pub fn step(&mut self, params: Float32Array, gradients: Float32Array) -> Float32Array {
        let mut params_vec = params.to_vec();
        let gradients_slice = gradients.as_ref();
        self.inner.step(&mut params_vec, gradients_slice);
        Float32Array::new(params_vec)
    }

    /// Reset optimizer state
    #[napi]
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Get current learning rate
    #[napi(getter)]
    pub fn learning_rate(&self) -> f64 {
        self.inner.learning_rate() as f64
    }

    /// Set learning rate
    #[napi(setter)]
    pub fn set_learning_rate(&mut self, lr: f64) {
        self.inner.set_learning_rate(lr as f32);
    }
}

/// Adam optimizer with bias correction
#[napi]
pub struct AdamOptimizer {
    inner: RustAdam,
}

#[napi]
impl AdamOptimizer {
    /// Create a new Adam optimizer
    ///
    /// # Arguments
    /// * `param_count` - Number of parameters
    /// * `learning_rate` - Learning rate
    #[napi(constructor)]
    pub fn new(param_count: u32, learning_rate: f64) -> Self {
        Self {
            inner: RustAdam::new(param_count as usize, learning_rate as f32),
        }
    }

    /// Create with custom betas
    #[napi(factory)]
    pub fn with_betas(param_count: u32, learning_rate: f64, beta1: f64, beta2: f64) -> Self {
        Self {
            inner: RustAdam::new(param_count as usize, learning_rate as f32)
                .with_betas(beta1 as f32, beta2 as f32),
        }
    }

    /// Create with full configuration
    #[napi(factory)]
    pub fn with_config(
        param_count: u32,
        learning_rate: f64,
        beta1: f64,
        beta2: f64,
        epsilon: f64,
        weight_decay: f64,
    ) -> Self {
        Self {
            inner: RustAdam::new(param_count as usize, learning_rate as f32)
                .with_betas(beta1 as f32, beta2 as f32)
                .with_epsilon(epsilon as f32)
                .with_weight_decay(weight_decay as f32),
        }
    }

    /// Perform an optimization step
    ///
    /// # Returns
    /// Updated parameter array
    #[napi]
    pub fn step(&mut self, params: Float32Array, gradients: Float32Array) -> Float32Array {
        let mut params_vec = params.to_vec();
        let gradients_slice = gradients.as_ref();
        self.inner.step(&mut params_vec, gradients_slice);
        Float32Array::new(params_vec)
    }

    /// Reset optimizer state (momentum terms)
    #[napi]
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Get current learning rate
    #[napi(getter)]
    pub fn learning_rate(&self) -> f64 {
        self.inner.learning_rate() as f64
    }

    /// Set learning rate
    #[napi(setter)]
    pub fn set_learning_rate(&mut self, lr: f64) {
        self.inner.set_learning_rate(lr as f32);
    }
}

/// AdamW optimizer (Adam with decoupled weight decay)
#[napi]
pub struct AdamWOptimizer {
    inner: RustAdamW,
    wd: f32,
}

#[napi]
impl AdamWOptimizer {
    /// Create a new AdamW optimizer
    ///
    /// # Arguments
    /// * `param_count` - Number of parameters
    /// * `learning_rate` - Learning rate
    /// * `weight_decay` - Weight decay coefficient
    #[napi(constructor)]
    pub fn new(param_count: u32, learning_rate: f64, weight_decay: f64) -> Self {
        Self {
            inner: RustAdamW::new(param_count as usize, learning_rate as f32)
                .with_weight_decay(weight_decay as f32),
            wd: weight_decay as f32,
        }
    }

    /// Create with custom betas
    #[napi(factory)]
    pub fn with_betas(
        param_count: u32,
        learning_rate: f64,
        weight_decay: f64,
        beta1: f64,
        beta2: f64,
    ) -> Self {
        Self {
            inner: RustAdamW::new(param_count as usize, learning_rate as f32)
                .with_weight_decay(weight_decay as f32)
                .with_betas(beta1 as f32, beta2 as f32),
            wd: weight_decay as f32,
        }
    }

    /// Perform an optimization step
    ///
    /// # Returns
    /// Updated parameter array
    #[napi]
    pub fn step(&mut self, params: Float32Array, gradients: Float32Array) -> Float32Array {
        let mut params_vec = params.to_vec();
        let gradients_slice = gradients.as_ref();
        self.inner.step(&mut params_vec, gradients_slice);
        Float32Array::new(params_vec)
    }

    /// Reset optimizer state
    #[napi]
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Get current learning rate
    #[napi(getter)]
    pub fn learning_rate(&self) -> f64 {
        self.inner.learning_rate() as f64
    }

    /// Set learning rate
    #[napi(setter)]
    pub fn set_learning_rate(&mut self, lr: f64) {
        self.inner.set_learning_rate(lr as f32);
    }

    /// Get weight decay
    #[napi(getter)]
    pub fn weight_decay(&self) -> f64 {
        self.wd as f64
    }
}

// ============================================================================
// Learning Rate Scheduling
// ============================================================================

/// Learning rate scheduler with warmup and cosine decay
#[napi]
pub struct LearningRateScheduler {
    initial_lr: f32,
    current_step: usize,
    warmup_steps: usize,
    total_steps: usize,
    min_lr: f32,
}

#[napi]
impl LearningRateScheduler {
    /// Create a new learning rate scheduler
    ///
    /// # Arguments
    /// * `initial_lr` - Initial/peak learning rate
    /// * `warmup_steps` - Number of warmup steps
    /// * `total_steps` - Total training steps
    #[napi(constructor)]
    pub fn new(initial_lr: f64, warmup_steps: u32, total_steps: u32) -> Self {
        Self {
            initial_lr: initial_lr as f32,
            current_step: 0,
            warmup_steps: warmup_steps as usize,
            total_steps: total_steps as usize,
            min_lr: 1e-7,
        }
    }

    /// Create with minimum learning rate
    #[napi(factory)]
    pub fn with_min_lr(initial_lr: f64, warmup_steps: u32, total_steps: u32, min_lr: f64) -> Self {
        Self {
            initial_lr: initial_lr as f32,
            current_step: 0,
            warmup_steps: warmup_steps as usize,
            total_steps: total_steps as usize,
            min_lr: min_lr as f32,
        }
    }

    /// Get learning rate for current step
    #[napi]
    pub fn get_lr(&self) -> f64 {
        if self.current_step < self.warmup_steps {
            // Linear warmup
            (self.initial_lr * (self.current_step + 1) as f32 / self.warmup_steps as f32) as f64
        } else {
            // Cosine decay
            let progress = (self.current_step - self.warmup_steps) as f32
                / (self.total_steps - self.warmup_steps).max(1) as f32;
            let decay = 0.5 * (1.0 + (std::f32::consts::PI * progress.min(1.0)).cos());
            (self.min_lr + (self.initial_lr - self.min_lr) * decay) as f64
        }
    }

    /// Step the scheduler and return current learning rate
    #[napi]
    pub fn step(&mut self) -> f64 {
        let lr = self.get_lr();
        self.current_step += 1;
        lr
    }

    /// Reset scheduler to initial state
    #[napi]
    pub fn reset(&mut self) {
        self.current_step = 0;
    }

    /// Get current step
    #[napi(getter)]
    pub fn current_step(&self) -> u32 {
        self.current_step as u32
    }

    /// Get progress (0.0 to 1.0)
    #[napi(getter)]
    pub fn progress(&self) -> f64 {
        (self.current_step as f64 / self.total_steps.max(1) as f64).min(1.0)
    }
}

// ============================================================================
// Temperature Annealing
// ============================================================================

/// Decay type for temperature annealing
#[napi(string_enum)]
pub enum DecayType {
    Linear,
    Exponential,
    Cosine,
    Step,
}

impl From<DecayType> for RustDecayType {
    fn from(dt: DecayType) -> Self {
        match dt {
            DecayType::Linear => RustDecayType::Linear,
            DecayType::Exponential => RustDecayType::Exponential,
            DecayType::Cosine => RustDecayType::Cosine,
            DecayType::Step => RustDecayType::Step,
        }
    }
}

/// Temperature annealing scheduler
#[napi]
pub struct TemperatureAnnealing {
    inner: RustTempAnnealing,
}

#[napi]
impl TemperatureAnnealing {
    /// Create a new temperature annealing scheduler
    ///
    /// # Arguments
    /// * `initial_temp` - Starting temperature
    /// * `final_temp` - Final temperature
    /// * `steps` - Number of annealing steps
    #[napi(constructor)]
    pub fn new(initial_temp: f64, final_temp: f64, steps: u32) -> Self {
        Self {
            inner: RustTempAnnealing::new(initial_temp as f32, final_temp as f32, steps as usize),
        }
    }

    /// Create with specific decay type
    #[napi(factory)]
    pub fn with_decay(
        initial_temp: f64,
        final_temp: f64,
        steps: u32,
        decay_type: DecayType,
    ) -> Self {
        Self {
            inner: RustTempAnnealing::new(initial_temp as f32, final_temp as f32, steps as usize)
                .with_decay(decay_type.into()),
        }
    }

    /// Get current temperature
    #[napi]
    pub fn get_temp(&self) -> f64 {
        self.inner.get_temp() as f64
    }

    /// Step the scheduler and return current temperature
    #[napi]
    pub fn step(&mut self) -> f64 {
        self.inner.step() as f64
    }

    /// Reset scheduler
    #[napi]
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

// ============================================================================
// Curriculum Learning
// ============================================================================

/// Curriculum stage configuration
#[napi(object)]
pub struct CurriculumStageConfig {
    pub name: String,
    pub difficulty: f64,
    pub duration: u32,
    pub temperature: f64,
    pub negative_count: u32,
}

/// Curriculum scheduler for progressive training
#[napi]
pub struct CurriculumScheduler {
    inner: RustCurriculum,
}

#[napi]
impl CurriculumScheduler {
    /// Create an empty curriculum scheduler
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: RustCurriculum::new(),
        }
    }

    /// Create a default easy-to-hard curriculum
    #[napi(factory)]
    pub fn default_curriculum(total_steps: u32) -> Self {
        Self {
            inner: RustCurriculum::default_curriculum(total_steps as usize),
        }
    }

    /// Add a stage to the curriculum
    #[napi]
    pub fn add_stage(&mut self, config: CurriculumStageConfig) {
        let stage = RustStage::new(&config.name)
            .difficulty(config.difficulty as f32)
            .duration(config.duration as usize)
            .temperature(config.temperature as f32)
            .negative_count(config.negative_count as usize);

        // Rebuild with added stage
        let new_inner = std::mem::take(&mut self.inner).add_stage(stage);
        self.inner = new_inner;
    }

    /// Step the curriculum and return current stage info
    #[napi]
    pub fn step(&mut self) -> Option<CurriculumStageConfig> {
        self.inner.step().map(|s| CurriculumStageConfig {
            name: s.name.clone(),
            difficulty: s.difficulty as f64,
            duration: s.duration as u32,
            temperature: s.temperature as f64,
            negative_count: s.negative_count as u32,
        })
    }

    /// Get current difficulty (0.0 to 1.0)
    #[napi(getter)]
    pub fn difficulty(&self) -> f64 {
        self.inner.difficulty() as f64
    }

    /// Get current temperature
    #[napi(getter)]
    pub fn temperature(&self) -> f64 {
        self.inner.temperature() as f64
    }

    /// Get current negative count
    #[napi(getter)]
    pub fn negative_count(&self) -> u32 {
        self.inner.negative_count() as u32
    }

    /// Check if curriculum is complete
    #[napi(getter)]
    pub fn is_complete(&self) -> bool {
        self.inner.is_complete()
    }

    /// Get overall progress (0.0 to 1.0)
    #[napi(getter)]
    pub fn progress(&self) -> f64 {
        self.inner.progress() as f64
    }

    /// Reset curriculum
    #[napi]
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

// ============================================================================
// Negative Mining
// ============================================================================

/// Mining strategy for negative selection
#[napi(string_enum)]
pub enum MiningStrategy {
    Random,
    HardNegative,
    SemiHard,
    DistanceWeighted,
}

impl From<MiningStrategy> for RustMiningStrategy {
    fn from(ms: MiningStrategy) -> Self {
        match ms {
            MiningStrategy::Random => RustMiningStrategy::Random,
            MiningStrategy::HardNegative => RustMiningStrategy::HardNegative,
            MiningStrategy::SemiHard => RustMiningStrategy::SemiHard,
            MiningStrategy::DistanceWeighted => RustMiningStrategy::DistanceWeighted,
        }
    }
}

/// Hard negative miner for selecting informative negatives
#[napi]
pub struct HardNegativeMiner {
    inner: RustHardMiner,
}

#[napi]
impl HardNegativeMiner {
    /// Create a new hard negative miner
    ///
    /// # Arguments
    /// * `strategy` - Mining strategy to use
    #[napi(constructor)]
    pub fn new(strategy: MiningStrategy) -> Self {
        Self {
            inner: RustHardMiner::new(strategy.into()),
        }
    }

    /// Create with margin (for semi-hard mining)
    #[napi(factory)]
    pub fn with_margin(strategy: MiningStrategy, margin: f64) -> Self {
        Self {
            inner: RustHardMiner::new(strategy.into()).with_margin(margin as f32),
        }
    }

    /// Create with temperature (for distance-weighted mining)
    #[napi(factory)]
    pub fn with_temperature(strategy: MiningStrategy, temperature: f64) -> Self {
        Self {
            inner: RustHardMiner::new(strategy.into()).with_temperature(temperature as f32),
        }
    }

    /// Mine negative indices from candidates
    ///
    /// # Arguments
    /// * `anchor` - Anchor embedding
    /// * `positive` - Positive example embedding
    /// * `candidates` - Array of candidate embeddings
    /// * `num_negatives` - Number of negatives to select
    ///
    /// # Returns
    /// Array of indices into the candidates array
    #[napi]
    pub fn mine(
        &self,
        anchor: Float32Array,
        positive: Float32Array,
        candidates: Vec<Float32Array>,
        num_negatives: u32,
    ) -> Vec<u32> {
        let anchor_slice = anchor.as_ref();
        let positive_slice = positive.as_ref();
        let candidates_vec: Vec<Vec<f32>> = candidates.into_iter().map(|c| c.to_vec()).collect();
        let candidates_refs: Vec<&[f32]> = candidates_vec.iter().map(|c| c.as_slice()).collect();

        self.inner
            .mine(
                anchor_slice,
                positive_slice,
                &candidates_refs,
                num_negatives as usize,
            )
            .into_iter()
            .map(|i| i as u32)
            .collect()
    }
}

/// In-batch negative mining utility
#[napi]
pub struct InBatchMiner {
    exclude_positive: bool,
}

#[napi]
impl InBatchMiner {
    /// Create a new in-batch miner
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            exclude_positive: true,
        }
    }

    /// Create without excluding positive
    #[napi(factory)]
    pub fn include_positive() -> Self {
        Self {
            exclude_positive: false,
        }
    }

    /// Get negative indices for a given anchor in a batch
    ///
    /// # Arguments
    /// * `anchor_idx` - Index of the anchor in the batch
    /// * `positive_idx` - Index of the positive in the batch
    /// * `batch_size` - Total batch size
    ///
    /// # Returns
    /// Array of indices that can be used as negatives
    #[napi]
    pub fn get_negatives(&self, anchor_idx: u32, positive_idx: u32, batch_size: u32) -> Vec<u32> {
        (0..batch_size)
            .filter(|&i| i != anchor_idx && (!self.exclude_positive || i != positive_idx))
            .collect()
    }
}
